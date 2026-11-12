// Save-gate static gate: pins the rules that keep `runSaveTxn`'s callback from
// deadlocking on the save queue.
//
// Why a source scan and not a runtime check: the browser has no async-context
// API (`AsyncLocalStorage` is Node-only), so a reentrant call cannot be told
// apart from a legitimate concurrent caller — and a timeout heuristic would
// reject writes that still land afterwards (rejected in review). The deadlock
// only happens when a callback body calls a *queued* save entry, so pinning
// that shape in the source is the honest guarantee.
//
// Rules (each maps to one of the three exported findings):
//   1. `runSaveTxn` may only appear in allowlisted files (definition + the one
//      transactional caller). A new caller must pass review once.
//   2. A `runSaveTxn` callback body must not reference ANY queued save entry —
//      `runSaveTxn`, `updateEditorPrefs`, `saveSplitRatio` (including optional
//      calls `saveSplitRatio?.(…)`). The two wrappers queue too, so calling
//      them inside a callback deadlocks exactly like a nested transaction.
//      **Fail-closed**: a callback form the scanner cannot extract (a referenced
//      callback from another module, an alias, `handlers.save`, …) is reported
//      as a violation instead of being skipped — otherwise the gate would show
//      green while the queued save inside it goes unscanned.
//   3. The two public wrappers must write through `runSaveTxn` and must not
//      reach the private save body directly (that would bypass the queue).
//   4. `switchObservers.add(listener)` listeners run inside the transaction
//      callback (performSwitch's finally awaits them). A listener that calls a
//      queued save deadlocks: listener waits the queue, the queue waits the
//      callback, the callback waits the listener. Every registered listener must
//      be resolvable and clean (fail-closed, same as rule 2).
//
// Rule 2 treats a **bare parameter name** as a pure forward — scoped to the
// registered forward entry: the call site must sit inside a method named
// `runSaveTxn` in stores/configStore.ts and the forwarded callback must be one
// of that method's own parameters (annotated returns included). The callback
// body is then provided at the callers' call sites, which are scanned; every
// file mentioning runSaveTxn is allowlisted and scanned per call site. Same-named
// external callbacks, arbitrary forward aliases and non-parameter bare
// identifiers all fail closed. Anything else it cannot resolve still fails
// closed.
//
// Vue SFCs are scanned through their `<script>` blocks PLUS the template's
// executable expressions — directive bindings (`@` / `v-` / `:`) and
// interpolations (`{{ … }}`), extracted from the real template AST
// (`templateExpressions` in scripts/lib/source-scan.mjs) and appended after the
// script body. HTML comments and static attributes are not expressions; an
// apostrophe in `Don't` cannot decide how a later `/* … */` reads.
//
// Rule 2 also follows **same-file local helpers** (callback → performSwitch →
// saveSplitRatio), so a helper gaining a queued call is caught — for the
// shapes this repo actually writes: `function` declarations (return-type
// annotations without braces, e.g. `(): () => void`), block- and
// expression-body arrows (`=> expr` captured to `;` / newline), and
// class/object methods **without annotations** (`name(...) {`, getters).
// What is *not* tracked: cross-file helpers, aliases, dynamic lookup,
// multi-line expression bodies (truncated at the first newline), methods with
// a return annotation (`helper(): Promise<void> {`), declarations whose
// return type contains braces (`function f(): Array<{...}> {`), arrows stored as
// **object properties** (`const o = { h: () => saveSplitRatio(0.5) }` — only
// same-file function declarations / arrows / methods are keyed by name), and
// same-named helpers resolved at **depth > 6** (the transfer walk has a hard
// depth cap). Unknown *callback* forms fail closed instead, but the helper
// shapes above are skipped silently; keep queued calls inside a tracked shape.
// Scanning-fidelity note: unquoted regex literals are consumed verbatim by the
// regex-aware stripper (division-vs-regex heuristic; control-statement closing
// parens are regex context — see scripts/lib/source-scan.mjs), so `/[/*]/` and
// `if (ok) /[/*]/.test(v)` no longer truncate the scan.
//
// Why not "structural" enforcement (only `SaveTxn` reachable): JS has no async
// context (`AsyncLocalStorage` is Node-only), so a callback can always call a
// public queued wrapper — no module boundary can distinguish it from a
// legitimate concurrent caller. The structural half is already in place (the
// only non-queuing write channel is the private `updateConfigNow`, reachable
// solely through the `SaveTxn` handle); this script is the honest complement
// that pins the *call-site shape*. Limits (stated, not implied): cross-file
// helpers, aliases, and dynamic lookup are invisible — which is exactly why
// unknown forms fail closed above.
//
// Usage:
//   node scripts/check-save-gate.mjs      (also: npm run check:save-gate)

import { pathToFileURL } from 'node:url'
import {
  prepareSource,
  readAngles,
  readBalanced,
  stripComments,
  walkSources,
} from './lib/source-scan.mjs'

// stripComments / readBalanced / the directory walk live in
// scripts/lib/source-scan.mjs, shared with check-layering.mjs: a fidelity fix
// (Vue template prose feeding the tokenizer, a comment-stripping edge) must not
// drift between the two gates. Re-exported below for existing callers.
export { stripComments, readBalanced }

/// Files allowed to mention `runSaveTxn` (paths relative to `src/`, forward slashes).
export const TXN_ALLOWLIST = [
  'services/config.service.ts', // definition + the two semantic wrappers
  'composables/useSettingsForm.ts', // the only compound transaction (save → auto-switch)
  // 2026-11-09 严格分层：View/composable 不再直连 configService，保存事务经
  // stores/configStore 转发。纯转发（非新事务调用方）：回调以**参数直传**形态出现，
  // 由 findViolations 的参数直传规则免扫——回调正文在调用点，而任何出现
  // runSaveTxn 的文件都必须白名单化并被逐调用点扫描，转发体本身不可能引入
  // 排队入口。
  'stores/configStore.ts',
]

/// Queued save entries: appearing inside a transaction callback body deadlocks.
/// Optional calls (`saveSplitRatio?.(…)`) count too — `?.` is still a call;
/// `.call(...)` / `.apply(...)` also queue; bracket-literal access
/// (`configService['saveSplitRatio'](0.5)`, template brackets included) is the
/// same queued entry — the quoted name inside the brackets is matched with a
/// backreference so other members never match (review: literal bracket access
/// was a false green). The anchor is **name + closing quote + `]` + `(`**,
/// because the `\b` of the name sits inside the brackets.
const QUEUED_ENTRY = /\b(runSaveTxn|updateEditorPrefs|saveSplitRatio)(?:['"`]\s*\]\s*\(|\s*(?:\?\.)?\s*(?:\.\s*(?:call|apply)\s*)?\s*[<(])/

/**
 * Extract a callback body from a call's argument text.
 *
 * Handles inline arrows (block & expression body) and inline
 * `function` / `async function` expressions. A **bare identifier** is returned
 * as `ref` so the caller can resolve it against the same-file local function
 * table. Anything else yields `{ body: null, ref: null }` — the caller must
 * treat that as a fail-closed violation, never as "nothing to scan".
 *
 * @param {string} args raw text between the call's parentheses
 * @returns {{ body: string | null, ref: string | null }}
 */
export function callbackFromArgs(args) {
  // `function` 头必须先判定：先找任意 `=>` 会被回调体内部的嵌套箭头劫持
  //（`async function doSave(tx) { … const nested = () => null }` 会截出错误正文，
  // 排队入口因此漏扫——评审 P3）
  const fnHead = args.match(/^\s*(?:async\s+)?function\b/)
  if (fnHead) {
    const open = args.indexOf('(', fnHead[0].length)
    const params = open === -1 ? null : readBalanced(args, open, '(', ')')
    if (params !== null) {
      const brace = args.indexOf('{', open + params.length + 2)
      const body = brace === -1 ? null : readBalanced(args, brace, '{', '}')
      return { body, ref: null }
    }
    return { body: null, ref: null }
  }
  const arrow = args.indexOf('=>')
  if (arrow !== -1) {
    const rest = args.slice(arrow + 2)
    const lead = rest.search(/\S/)
    if (lead !== -1) {
      const tail = rest.slice(lead)
      if (tail.startsWith('{')) {
        const body = readBalanced(tail, 0, '{', '}')
        return { body, ref: null }
      }
      // 表达式体：可能连第二个参数（如 `, { settingsPage: true }`）一起截进来——
      // 偏保守（多扫不误放），无需精确切分。
      return { body: tail, ref: null }
    }
  }
  // 裸标识符（可带第二参数）：由调用方在同文件函数表里解析
  const refMatch = args.match(/^\s*([A-Za-z_$][\w$]*)\s*(?:,|$)/)
  if (refMatch) return { body: null, ref: refMatch[1] }
  return { body: null, ref: null }
}

/// 定义尾形态判定：参数闭括号之后（同行）是方法体 `{` 或返回注解 `: … {`。
/// 调用表达式之后不可能出现这两种形态（`foo(x) {` 不是合法调用），因此用它
/// 区分「定义」与「显式泛型调用」——旧实现按字面 `<T>` 跳定义，会把
/// `runSaveTxn<T>(cb)` 调用一并静默跳过（白名单文件内的真实残留面）。
function isDefinitionTail(source, closeParenIndex) {
  let i = closeParenIndex + 1
  while (i < source.length && (source[i] === ' ' || source[i] === '\t')) i += 1
  if (source[i] === '\n' || source[i] === '\r') return false
  if (source[i] === '{') return true
  if (source[i] !== ':') return false
  let depth = 0
  i += 1
  while (i < source.length) {
    const ch = source[i]
    if (ch === '\n') return false
    if (ch === '(' || ch === '[' || ch === '<') depth += 1
    else if (ch === ')' || ch === ']' || ch === '>') depth = Math.max(0, depth - 1)
    else if (ch === '{' && depth === 0) return true
    else if ((ch === '=' || ch === ';') && depth === 0) return false
    i += 1
  }
  return false
}

/**
 * Every `runSaveTxn(...)` **call site** with its argument text and the extracted
 * callback. Method **definitions** are skipped by shape (params followed by a
 * body or a return annotation — see `isDefinitionTail`), so explicit generic
 * calls (`runSaveTxn<T>(cb)`) are scanned like any other call. `body === null`
 * is meaningful: `ref` is a same-file function name, or both are null for a
 * form the gate cannot analyze (fail closed in `findViolations`).
 */
export function txnCallSites(source) {
  const sites = []
  const needle = 'runSaveTxn'
  let from = 0
  for (;;) {
    const hit = source.indexOf(needle, from)
    if (hit === -1) break
    from = hit + needle.length
    // 就地前扫（不 slice 文件尾部）：跳过空白与可选泛型实参，找调用括号
    let cursor = from
    while (cursor < source.length && /\s/.test(source[cursor])) cursor += 1
    if (source[cursor] === '<') {
      const anglesEnd = readAngles(source, cursor)
      if (anglesEnd === null) {
        sites.push({ index: hit, args: '', body: null, ref: null })
        continue
      }
      cursor = anglesEnd
      while (cursor < source.length && /\s/.test(source[cursor])) cursor += 1
    }
    if (source[cursor] !== '(') {
      // 名字出现了但不是可解析的调用（别名/赋值/文本……）——fail closed
      sites.push({ index: hit, args: '', body: null, ref: null })
      continue
    }
    const openParen = cursor
    const args = readBalanced(source, openParen, '(', ')')
    if (args === null) {
      sites.push({ index: hit, args: '', body: null, ref: null })
      continue
    }
    // 成员调用（`configService.runSaveTxn(…)`）不可能是方法定义：三元表达式
    // 里调用后可以跟 `: { … }` 对象字面量，按尾形态会把它误判成「返回注解 +
    // 方法体」而跳过整个调用点（评审 P2）。定义只可能是裸名（`async runSaveTxn`）
    // 或对象/类成员（行首裸名）。**点号与属性名之间允许空白**（`svc . runSaveTxn(…)`
    // 是合法 JS），因此向前跳过空白再判 `.`。
    let dotScan = hit - 1
    while (dotScan >= 0 && /\s/.test(source[dotScan])) dotScan -= 1
    const isMemberCall = dotScan >= 0 && source[dotScan] === '.'
    if (!isMemberCall && isDefinitionTail(source, openParen + args.length + 1)) continue
    sites.push({ index: hit, args, ...callbackFromArgs(args) })
  }
  return sites
}

/// 兼容旧用例：只返回能直接提取到的回调正文（无法分析的形态由
/// `txnCallSites` + `findViolations` 负责 fail closed）。
export function txnCallbackBodies(source) {
  return txnCallSites(source)
    .map((site) => site.body)
    .filter((body) => body !== null)
}

/// 关键字/保留字：调用点提取时排除（`if (`、`return (` 之类不是函数调用）
const NOT_CALLS = new Set([
  'if', 'for', 'while', 'switch', 'catch', 'function', 'return', 'typeof', 'await',
  'new', 'void', 'delete', 'in', 'of', 'instanceof', 'do', 'else', 'try', 'finally',
  'throw', 'class', 'const', 'let', 'var', 'async', 'yield', 'super', 'this',
])

/// 提取正文里被调用的标识符（`name(` / `name?.(` / `name<T>(`），排除关键字。
/// 泛型实参用平衡读取（`readAngles`）——旧正则 `<[^<>()]*>` 在内层 `<` 处停下，
/// `persist<Array<string>>(…)` 不进集合、传递路径漏追（评审 P3）。
export function calledNames(body) {
  const names = new Set()
  const idRe = /\b([A-Za-z_$][\w$]*)/g
  let match
  while ((match = idRe.exec(body))) {
    const name = match[1]
    let i = match.index + name.length
    while (i < body.length && /\s/.test(body[i])) i += 1
    // 可选调用 `?.`
    if (body[i] === '?' && body[i + 1] === '.') {
      i += 2
      while (i < body.length && /\s/.test(body[i])) i += 1
    }
    if (body[i] === '<') {
      const end = readAngles(body, i)
      if (end === null) continue // 无法解析的泛型形态：不作为调用名（方向保守）
      i = end
      while (i < body.length && /\s/.test(body[i])) i += 1
    }
    if (body[i] === '(' && !NOT_CALLS.has(name)) names.add(name)
  }
  return names
}

/// 从 `afterName` 起解析一个 **函数声明** 的正文（无正文返回 null）。
///
/// 评审抓到的漏报形态：`function pick(): () => void {` 的返回类型注解里也有
/// `=>`，旧实现把它当成箭头函数的箭头、正文截成 "void"。修正：跳过配对的
/// 类型注解括号与注解内的 `=>`，正文从首个深度 0 的 `{` 取配对块；裸 `=` 或
/// `;` 意味着这不是函数声明的形状，返回 null。
function declBody(source, afterName) {
  const openParen = source.indexOf('(', afterName)
  if (openParen === -1) return null
  const params = readBalanced(source, openParen, '(', ')')
  if (params === null) return null
  let i = openParen + params.length + 2
  while (i < source.length) {
    const ch = source[i]
    if (ch === '"' || ch === "'" || ch === '`') {
      const quote = ch
      i += 1
      while (i < source.length) {
        if (source[i] === '\\') {
          i += 2
          continue
        }
        if (source[i] === quote) {
          i += 1
          break
        }
        i += 1
      }
      continue
    }
    if (ch === '(') {
      const inner = readBalanced(source, i, '(', ')')
      if (inner === null) return null
      i += inner.length + 2
      continue
    }
    if (ch === '{') return readBalanced(source, i, '{', '}')
    if (ch === '=') {
      // 函数声明分支里出现的 `=>` 只可能是返回类型注解的一部分
      if (source[i + 1] === '>') {
        i += 2
        continue
      }
      return null
    }
    if (ch === ';') return null
    i += 1
  }
  return null
}

/// 从 `=>` 之后解析箭头函数正文：块体取配对花括号；表达式体截到 `;` 或换行
///（跳过字符串字面量，多行表达式会被截断——见文件头局限）。
function arrowBody(source, afterArrow) {
  let i = afterArrow
  while (i < source.length && /\s/.test(source[i])) i += 1
  if (source[i] === '{') return readBalanced(source, i, '{', '}')
  let j = i
  while (j < source.length) {
    const ch = source[j]
    if (ch === '"' || ch === "'" || ch === '`') {
      const quote = ch
      j += 1
      while (j < source.length) {
        if (source[j] === '\\') {
          j += 2
          continue
        }
        if (source[j] === quote) {
          j += 1
          break
        }
        j += 1
      }
      continue
    }
    if (ch === ';' || ch === '\n') break
    j += 1
  }
  return source.slice(i, j)
}

/**
 * 同一文件内的局部函数表，覆盖本仓实际会写的形态：
 * - `function name(...)` 声明（返回类型注解**不含 `{`** 时：`(): () => void` 可、
 *   `(): Array<{...}> {` 不可——泛型里的 `{` 会被当成正文起点）
 * - `const name = (...) => {...}` 块体箭头 与 `const name = (...) => expr` 表达式体箭头
 * - 类/对象方法 `name(...) {`（**仅无注解形态**，含 getter；`helper(): Promise<void> {`
 *   与 `push(tx): void {` 不追踪——带上限的注解扫描成本高于收益，见文件头局限）
 *
 * 只为「回调 → 局部辅助函数 → 排队入口」这条**传递路径**服务：直接调用由
 * 事务回调的正文检查覆盖，跨文件/别名/动态调用不在能力范围内（见文件头）。
 */
export function localFunctions(source) {
  // 同名可有多份定义（不同作用域）。**保留全部正文**：旧实现首个定义获胜，
  // 先出现的无关同名函数会遮蔽后面真正调用排队入口的那份（评审 P2）。
  const table = new Map()
  const add = (name, body) => {
    if (body === null) return
    const bodies = table.get(name)
    if (bodies === undefined) {
      table.set(name, [body])
      return
    }
    // 同一正文可能被声明分支与方法分支各收一次；按内容去重，但**不同作用域的
    // 同名定义**（正文不同）全部保留
    if (!bodies.includes(body)) bodies.push(body)
  }
  const declRe = /\bfunction\s+([A-Za-z_$][\w$]*)|\b(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*(?::[^=]+)?=\s*(?:async\s*)?(?:\([^)]*\)|[A-Za-z_$][\w$]*)\s*(?::[^=]+)?=>/g
  let match
  while ((match = declRe.exec(source))) {
    const name = match[1] ?? match[2]
    if (name === undefined) continue
    const isFunction = match[1] !== undefined
    add(
      name,
      isFunction ? declBody(source, declRe.lastIndex) : arrowBody(source, declRe.lastIndex),
    )
  }
  // 方法形态：`name(...) {` / `name<T>(...) {`（排除关键字）。**泛型实参用
  // readAngles 平衡读取**——单层 `<[^<>()]*>` 在嵌套泛型（`Array<Map<…>>`）或
  // 函数类型（`(e: Event) => void`）处截断，嵌套泛型方法不进函数表、间接路径
  // 漏追（评审 P3）。合法 TS 里「调用后跟块」只会出现在类/对象体中；误收的
  // 条目只会多扫一些正文，方向是保守的。
  const methodRe = /\b([A-Za-z_$][\w$]*)\b/g
  while ((match = methodRe.exec(source))) {
    const name = match[1]
    if (NOT_CALLS.has(name)) continue
    let cursor = match.index + name.length
    while (cursor < source.length && /\s/.test(source[cursor])) cursor += 1
    // 可选泛型实参：`<...>`（readAngles 平衡读取，支持嵌套与函数类型）；无泛型
    // 则直接找参数括号——**两条路径都必须**，只认泛型形态会漏掉无泛型方法
    if (source[cursor] === '<') {
      const anglesEnd = readAngles(source, cursor)
      if (anglesEnd === null) continue
      cursor = anglesEnd
      while (cursor < source.length && /\s/.test(source[cursor])) cursor += 1
    }
    if (source[cursor] !== '(') continue
    const params = readBalanced(source, cursor, '(', ')')
    if (params === null) continue
    let i = cursor + params.length + 2
    while (i < source.length && /\s/.test(source[i])) i += 1
    // 参数闭括号后允许**同一行**的返回类型注解（`): Promise<void> {`——注解里
    // 的 `=>`、括号、泛型逐 token 跳过），方法体 `{` 可能在注解之后
    let sawOpen = false
    while (i < source.length) {
      const ch = source[i]
      if (ch === '\n' || ch === ';') break
      if (ch === '{') {
        add(name, readBalanced(source, i, '{', '}'))
        sawOpen = true
        break
      }
      if (ch === '(' || ch === '<') {
        const closer = ch === '(' ? readBalanced(source, i, '(', ')') : readAngles(source, i)
        if (closer === null) break
        i += closer === null ? 1 : closer - i
        continue
      }
      i += 1
    }
    if (!sawOpen) continue
  }
  return table
}

/// 参数列表按深度 0 的逗号切分（嵌套括号/泛型/对象字面量里的逗号不切）。
function splitTopLevel(params) {
  const segments = []
  let depth = 0
  let current = ''
  for (const ch of params) {
    if (ch === '(' || ch === '[' || ch === '{' || ch === '<') depth += 1
    else if (ch === ')' || ch === ']' || ch === '}' || ch === '>') depth = Math.max(0, depth - 1)
    if (ch === ',' && depth === 0) {
      segments.push(current)
      current = ''
      continue
    }
    current += ch
  }
  segments.push(current)
  return segments
}

/// 顶层默认值赋值的 `=`（排除箭头 `=>` 与比较 `==`/`===`/`>=`/`<=`）。
function defaultAssignmentIndex(segment) {
  let depth = 0
  for (let i = 0; i < segment.length; i += 1) {
    const ch = segment[i]
    if (ch === '(' || ch === '[' || ch === '{' || ch === '<') depth += 1
    else if (ch === ')' || ch === ']' || ch === '}' || ch === '>') depth = Math.max(0, depth - 1)
    else if (ch === '=' && depth === 0) {
      const prev = segment[i - 1]
      const next = segment[i + 1]
      if (next === '>' || next === '=' || prev === '=' || prev === '!' || prev === '<' || prev === '>') continue
      return i
    }
  }
  return -1
}

function paramNames(params) {
  const names = new Set()
  for (const segment of splitTopLevel(params)) {
    const trimmed = segment.trim()
    const m = trimmed.match(/^(?:\.\.\.)?([A-Za-z_$][\w$]*)/)
    if (!m) continue
    // 带默认值的形参：**惰性字面量**（`{}` / 布尔 / 数字 / 字符串 / null）无害；
    // 任何表达式默认值（箭头、调用、标识符）都可能把回调内联在转发体之外执行，
    // 而那段正文永不被扫（评审：默认参数走私）——整条入口放弃豁免，fail closed。
    const eq = defaultAssignmentIndex(trimmed)
    if (eq !== -1 && !/^\s*(?:\{\s*\}|true|false|null|-?\d+(?:\.\d+)?|'[^']*'|"[^"]*")\s*$/.test(trimmed.slice(eq + 1))) {
      return null
    }
    names.add(m[1])
  }
  return names
}

/**
 * 识别「同名转发入口」：名为 `runSaveTxn` 的方法定义（其**形参**被原样转发给
 * service）。调用点会被 `;`/换行/`)` 过滤掉——定义之后才跟方法体；带返回注解
 * 的 `runSaveTxn<T>(…): Promise<T> {` 同样识别（注解只允许出现在参数括号的
 * 同一行，调用表达式的实参不会混进来）。
 */
export function runSaveTxnForwardEntries(source) {
  const entries = []
  // 泛型实参用 readAngles 平衡读取——嵌套泛型转发入口
  // （`runSaveTxn<Array<T>>(fn)`）用单层 `<[^<>()]*>` 会找不到调用括号、整条
  // 入口消失（方向安全但「无法静态分析」的报错对合法形态有误导，评审 P4）。
  // 调用点（`configService.runSaveTxn(fn)`）不会产生入口：其参数闭括号后没有
  // 方法体 `{`。
  for (const match of source.matchAll(/\brunSaveTxn\b/g)) {
    let cursor = match.index + 'runSaveTxn'.length
    while (cursor < source.length && /\s/.test(source[cursor])) cursor += 1
    if (source[cursor] === '<') {
      const anglesEnd = readAngles(source, cursor)
      if (anglesEnd === null) continue
      cursor = anglesEnd
      while (cursor < source.length && /\s/.test(source[cursor])) cursor += 1
    }
    if (source[cursor] !== '(') continue
    const open = cursor
    const params = readBalanced(source, open, '(', ')')
    if (params === null) continue
    // 方法体 `{` 可能跟在**同一行**的返回注解之后（`): Promise<void> {`）——
    // 逐 token 跳过注解内的括号/泛型/箭头，与 localFunctions 的方法分支同口径
    let i = open + params.length + 2
    let bodyOpen = -1
    while (i < source.length) {
      const ch = source[i]
      if (ch === '\n' || ch === ';' || ch === ')' || ch === ',' || ch === '}') break
      if (ch === '{') {
        bodyOpen = i
        break
      }
      if (ch === '(') {
        const inner = readBalanced(source, i, '(', ')')
        if (inner === null) break
        i += inner.length + 2
        continue
      }
      if (ch === '<') {
        const anglesEnd = readAngles(source, i)
        if (anglesEnd === null) break
        i = anglesEnd
        continue
      }
      i += 1
    }
    if (bodyOpen === -1) continue
    const body = readBalanced(source, bodyOpen, '{', '}')
    if (body === null) continue
    const names = paramNames(params)
    // 默认值形参（null）整条入口不作豁免：回调可能内联在缺省值里
    if (names === null) continue
    entries.push({ bodyStart: bodyOpen + 1, bodyEnd: bodyOpen + 1 + body.length, paramNames: names })
  }
  return entries
}

/** 调用点落在转发入口方法体内、且被转发回调是该方法的形参——纯转发。 */
function isPureForward(entries, callIndex, ref) {
  return entries.some(
    (entry) => callIndex >= entry.bodyStart && callIndex <= entry.bodyEnd && entry.paramNames.has(ref),
  )
}

/**
 * 从回调正文出发，沿**同文件局部函数**追踪到排队入口（深度有上限、带环检测）。
 * 返回命中路径，例如 `performSwitch → saveSplitRatio`。
 */
export function queuedViaLocals(callbackBody, functions, maxDepth = 6) {
  const hits = []
  const walk = (body, trail, depth, seen) => {
    for (const name of calledNames(body)) {
      if (seen.has(name)) continue
      const bodies = functions.get(name)
      if (bodies === undefined) continue
      const nextTrail = [...trail, name]
      // 同名可能有多份正文（不同作用域）：逐份检查，任一份命中即报
      for (const fnBody of bodies) {
        const match = fnBody.match(QUEUED_ENTRY)
        if (match) {
          hits.push({ trail: nextTrail, entry: match[1] })
          continue
        }
        if (depth < maxDepth) walk(fnBody, nextTrail, depth + 1, new Set([...seen, name]))
      }
    }
  }
  walk(callbackBody, [], 0, new Set())
  return hits
}

/// Rule 3: the wrapper method bodies must queue and must not bypass it.
function wrapperViolations(file, source) {
  const violations = []
  for (const name of ['updateEditorPrefs', 'saveSplitRatio']) {
    const signature = source.indexOf(`${name}(`)
    if (signature === -1) {
      violations.push(`${file}: ${name} 不存在（公开保存入口被删除或改名，请同步本门禁与文档）`)
      continue
    }
    const braceStart = source.indexOf('{', signature)
    const body = braceStart === -1 ? null : readBalanced(source, braceStart, '{', '}')
    if (body === null) {
      violations.push(`${file}: ${name} 方法体无法解析（门禁的配对扫描失败）`)
      continue
    }
    if (!body.includes('runSaveTxn')) {
      violations.push(`${file}: ${name} 未经营业队列（应经 runSaveTxn，否则并发保存会丢更新）`)
    }
    if (body.includes('updateConfigNow')) {
      violations.push(`${file}: ${name} 直连 updateConfigNow（绕过保存队列）`)
    }
  }
  return violations
}

/**
 * 规则 4：`switchObservers.add(listener)` 的监听器会在事务回调内被 await
 * （`performSwitch` 的 finally）。监听器若（直接/经同文件辅助函数）调用排队
 * 保存入口，就是「监听器等队列、队列等回调、回调等监听器」的死锁。监听器形态
 * 无法解析时同样 fail closed。
 */
export function observerViolations(path, source, functions) {
  const violations = []
  // 混淆注册形态（`switchObservers['add'](...)`、`switchObservers.add.bind(...)`）
  // 扫描不到监听器 → 门禁会假绿；出现即 fail closed（评审：bind/['add'] 变体）
  if (/\bswitchObservers\s*\[/.test(source) || /\bswitchObservers\s*\.\s*add\s*\./.test(source)) {
    violations.push(
      `${path}: switchObservers 的注册形态无法静态分析（方括号取值 / add.bind）——` +
        '监听器在事务回调内被 await，若其调用排队保存会死锁；请用 switchObservers.add(命名函数或内联箭头)',
    )
  }
  const re = /\bswitchObservers\.add\s*\(/g
  let match
  while ((match = re.exec(source))) {
    const open = match.index + match[0].length - 1
    const args = readBalanced(source, open, '(', ')')
    if (args === null) {
      violations.push(`${path}: switchObservers.add 的参数无法解析（门禁 fail closed）`)
      continue
    }
    const cb = callbackFromArgs(args)
    const bodies = cb.body !== null ? [cb.body] : cb.ref ? functions.get(cb.ref) ?? null : null
    if (bodies === null || bodies.length === 0) {
      violations.push(
        `${path}: switchObservers.add 的监听器形态无法静态分析（${args.trim().slice(0, 80)}）——` +
          '监听器在 runSaveTxn 回调内被 await，若它调用排队保存会死锁；请用同文件命名函数或内联箭头',
      )
      continue
    }
    for (const body of bodies) {
      const direct = body.match(QUEUED_ENTRY)
      if (direct) {
        violations.push(
          `${path}: switchObservers 监听器内含排队保存入口 \`${direct[1]}\`——` +
            '观察者在事务回调内被 await，会排队等自己（死锁）。监听器只能做读取/对齐',
        )
        continue
      }
      for (const hit of queuedViaLocals(body, functions)) {
        violations.push(
          `${path}: switchObservers 监听器经 ${hit.trail.join(' → ')} 间接调用排队入口 \`${hit.entry}\`——` +
            '同一死锁形状（监听器等队列，队列等回调，回调等监听器）',
        )
      }
    }
  }
  return violations
}

/**
 * Check a set of sources.
 * @param {Array<{path: string, content: string}>} files paths relative to `src/`
 * @returns {string[]} human-readable violations (empty = clean)
 */
export function findViolations(files) {
  const violations = []
  for (const { path, content } of files) {
    const source = prepareSource(path, content, { templateExpressions: true })
    const functions = localFunctions(source)
    // 参数直传豁免**只认 stores/configStore.ts 的同名转发入口**（评审 P2：
    // 文件级参数名豁免会把同名外部回调假绿；任意 forward 别名的调用点不会
    // 出现 runSaveTxn，回调正文根本没有被扫描）。
    const forwardEntries = path === 'stores/configStore.ts' ? runSaveTxnForwardEntries(source) : []
    if (source.includes('runSaveTxn')) {
      if (!TXN_ALLOWLIST.includes(path)) {
        violations.push(
          `${path}: 出现 runSaveTxn，但不在白名单。确认它不在其它事务回调内（或改用 tx 句柄），` +
            '再把文件加进 TXN_ALLOWLIST —— 白名单是有意设置的复核摩擦',
        )
      }
      for (const site of txnCallSites(source)) {
        // 引用式回调：同文件命名函数可解析（同名多份正文时逐份扫）；
        // 跨文件/别名解析不到 → 违规（fail closed）
        const bodies = site.body !== null ? [site.body] : site.ref ? functions.get(site.ref) ?? null : null
        if (bodies === null && site.ref !== null && isPureForward(forwardEntries, site.index, site.ref)) {
          continue
        }
        if (bodies === null || bodies.length === 0) {
          violations.push(
            `${path}: runSaveTxn 回调形态无法静态分析（${site.args.trim().slice(0, 80) || '参数解析失败'}）——` +
              '请改为内联箭头/同文件命名函数；无法确认回调内容时按违规处理（否则门禁会假绿）',
          )
          continue
        }
        for (const body of bodies) {
          if (QUEUED_ENTRY.test(body)) {
            violations.push(
              `${path}: runSaveTxn 回调内引用了排队保存入口（runSaveTxn/updateEditorPrefs/saveSplitRatio，含 ?.()）——` +
                '它会排在回调自己持有的门禁之后，永久挂起。改用 tx.updateConfig(...)。片段：' +
                body.trim().slice(0, 120),
            )
            continue
          }
          // 传递路径：回调 → 局部辅助函数 →（…）→ 排队入口。只扫同文件局部函数，
          // 覆盖「回调里调 performSwitch 之类辅助函数」的场景（评审：单层文本匹配
          // 抓不到这种间接调用）。
          for (const hit of queuedViaLocals(body, functions)) {
            violations.push(
              `${path}: runSaveTxn 回调经 ${hit.trail.join(' → ')} 间接调用排队入口 \`${hit.entry}\`——` +
                '同一队列，回调会等自己，永久挂起。该辅助函数内应改用 tx 句柄，或改调独立队列（如 switchOj）',
            )
          }
        }
      }
    }
    if (path === 'services/config.service.ts') {
      violations.push(...wrapperViolations(path, source))
    }
    violations.push(...observerViolations(path, source, functions))
  }
  return violations
}

/// Walk `src/` for production sources (tests/test-support/declarations excluded).
export function loadSources(root) {
  const files = walkSources(root, {
    dirs: ['.'],
    // 与 layering 门禁同一扩展名面（含 .d.ts）：.js/.jsx/.tsx/.mjs/.cjs 里
    // 同样可能出现保存事务调用，扫描面不能比文件名清单窄（评审 P2）
    extensions: /\.(?:d\.ts|ts|tsx|mts|cts|js|jsx|mjs|cjs|vue)$/,
    // 关键文件钉死：config.service.ts 改名/挪目录时按 path 匹配的包装规则会
    // 整体蒸发，必须 fail closed 而不是静默放行
    required: ['services/config.service.ts'],
    // 路径段匹配（子串会把 `x__tests__y` 这类正常文件误排除）
    skip: (rel) => /(^|\/)__tests__(\/|$)/.test(rel) || rel.startsWith('test-support/'),
  })
  if (files.length === 0) {
    // 空扫描面 = 门禁静默死亡（评审 P1：对照 layering 的 throw）
    throw new Error('save-gate: 扫描面为空（src/ 下未找到任何源文件）')
  }
  return files
}

export function runCli(root) {
  let violations
  try {
    violations = findViolations(loadSources(root))
  } catch (error) {
    // 扫描面不完整（空面 / 关键文件缺失）必须红：此前 runCli 直接抛，
    // 主模块守卫会退成非零——但 spec 调用 runCli 时看到的是异常而非返回码，
    // 且对照 layering 的口径应统一为「返回码」（评审 P1）
    console.error(`save-gate: ${error instanceof Error ? error.message : String(error)}`)
    return 1
  }
  if (violations.length === 0) {
    console.log('save-gate: OK（事务回调内无排队入口，公开包装经队列写入）')
    return 0
  }
  console.error('save-gate: 发现以下违规：')
  for (const violation of violations) console.error(`  - ${violation}`)
  return 1
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  try {
    process.exitCode = runCli(process.cwd())
  } catch (error) {
    console.error(`save-gate: ${error instanceof Error ? error.message : String(error)}`)
    process.exitCode = 1
  }
}
