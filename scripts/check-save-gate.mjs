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
//   2. A `runSaveTxn` callback body must not **call** ANY queued save entry
//      — `runSaveTxn`, `updateEditorPrefs`, `saveSplitRatio` — and the entries
//      must not be **referenced bare anywhere in a scanned file**. The two
//      wrappers queue too, so calling them inside a callback deadlocks exactly
//      like a nested transaction. Call detection is reference-level
//      (`QUEUED_ENTRY`: `(obj.m)(x)`, `obj.m.bind(obj)(x)`, `obj?.['m']?.(x)`,
//      `(0, obj.m)(x)`, tagged templates are all recognized or reported at the
//      bare-reference site). The bare-reference rule is the round-twenty fix:
//      `const up = configStore.updateEditorPrefs` at module scope followed by
//      `up({})` inside a callback used to be green — the callback body only
//      names the alias, and the runtime call queues behind the save gate the
//      callback already holds (settings stuck at "saving…" until restart).
//      Any non-call occurrence (alias binding, argument, property extraction,
//      even a string that merely spells the name — fail-closed direction) is
//      reported at its own site; the call-shaped forms are left to the
//      callback / observer / helper scans.
//      **Fail-closed**: a callback form the scanner cannot extract (a referenced
//      callback from another module, an alias, `handlers.save`, …) is reported
//      as a violation instead of being skipped — otherwise the gate would show
//      green while the queued save inside it goes unscanned.
//   3. The two public wrappers must write through `runSaveTxn` and must not
//      reach the private save body directly (that would bypass the queue). The
//      wrapper **definition** is located by definition shape at every
//      occurrence (`name(` + params + same-line body `{`) — `indexOf(name +
//      '(')` used to lock onto the first textual hit, so an earlier same-name
//      method or call site made the check read the wrong block (both directions:
//      a bypassing wrapper passed, an unrelated block was judged).
//   4. `switchObservers.add(listener)` listeners run inside the transaction
//      callback (performSwitch's finally awaits them). A listener that calls a
//      queued save deadlocks: listener waits the queue, the queue waits the
//      callback, the callback waits the listener. Registration is judged **per
//      occurrence** of `switchObservers`: only four benign shapes exist (`=
//      new Set` declaration, `.add(…)` with optional chaining, `.delete(…)`,
//      spread read `[...switchObservers]`) — aliases (`const reg =
//      switchObservers.add`), `add.bind`, bracket access, wholesale reassignment
//      and bare references all fail closed (an alias defeats any registration
//      matcher, so its listener would never be scanned — review). Every scanned
//      listener must be resolvable and clean.
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
// saveSplitRatio), and the transfer walk keys on **mentions**, not just calls —
// a helper passed by reference (`list.map(persistOne)`) is followed too (a
// reference can be invoked later). Tracked declaration shapes: `function`
// declarations (annotations may contain parentheses, generics such as
// `Array<{...}>` and `=>`; an object-type annotation immediately followed by
// the body, `(): { a: 1 } {`, is disambiguated from the body), function-
// expression constants (`const f = function (...) { … }`, annotated returns
// included), assignment arrows keyed by the assigned name (`h = () => …` class
// fields, `this.x = …`, params read with balanced brackets so nested-paren
// defaults survive), block- and expression-body arrows, and class/object
// methods **with a single-line return annotation**, generics included
// (`save<T>(v: Array<Map<string, T>>): void {`, getters; the body must sit on
// the params' line — a newline after the call parens means the next statement
// is not a definition). Object-literal properties are keyed by the property
// name (`{ run: async () => … }`, `{ run: function () { … } }` — review round
// twenty-two; plain `key: value` entries fast-fail, a ternary can key a
// pseudo-name — over-collection only widens the scan). What is *not*
// tracked: cross-file helpers, aliases, dynamic lookup, multi-line
// expression bodies (truncated at the first newline), method annotations that
// span a newline or contain a conditional `?` (the body walk stops there),
// object-type annotations with a postfix (`function f(): { a: 1 }[] {` — the
// type literal is read as the body), generic arguments spanning a newline
// (readAngles is single-line), and same-named helpers resolved at **depth >
// 6** (the transfer walk has a hard depth cap). Unknown *callback* forms and any queued-entry
// **reference** fail closed instead; the untracked helper shapes above are
// skipped silently — keep queued calls inside a tracked shape.
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
  skipStringLiteral,
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
/// `QUEUED_CALL_TAIL` recognizes the common **call** forms (optional calls
/// `saveSplitRatio?.(…)`, `.call(...)` / `.apply(...)`, bracket-literal access
/// `configService['saveSplitRatio'](0.5)`); `QUEUED_ENTRY` = name + tail. The
/// anchor for bracket access is **name + closing quote + `]` + `(`**, because
/// the `\b` of the name sits inside the brackets.
const QUEUED_CALL_TAIL = /(?:['"`]\s*\]\s*\(|\s*(?:\?\.)?\s*(?:\.\s*(?:call|apply)\s*)?\s*[<(])/
const QUEUED_ENTRY = new RegExp(`\\b(runSaveTxn|updateEditorPrefs|saveSplitRatio)${QUEUED_CALL_TAIL.source}`)
/// 同一 tail 的**锚定**版：判断某个出现点本身是否调用形态（全局裸引用扫描用——
/// 不锚定会在 tail 里搜到后续另一个调用点，把本处的裸引用误判成调用）。
const QUEUED_CALL_AT = new RegExp(`^${QUEUED_CALL_TAIL.source}`)
const QUEUED_NAME = /\b(runSaveTxn|updateEditorPrefs|saveSplitRatio)\b/g

/**
 * 全文件**裸引用**扫描：排队入口以非调用形态出现的位置（取别名
 * `const up = configStore.updateEditorPrefs`、传参
 * `register(configStore.saveSplitRatio)`、属性分离
 * `configStore['saveSplitRatio']`……）。这些引用可以在事务回调里经别名调用——
 * 回调正文里只有别名、看不到排队入口的名字，回调级扫描必然漏掉（评审：模块
 * 顶层取别名 + 回调内 `up({})` 曾零违规；运行时 `up()` 排在回调已持有的保存门
 * 后，设置页永久停在「保存中…」直到重启）。方向 fail closed：字符串里仅拼写
 * 名字同样报（与调用级口径一致）。调用形态不在此列（`name(` / `?.(` /
 * `.call(` / 方括号调用交给回调/监听器/辅助函数的调用扫描）。
 */
export function bareQueuedReferences(source) {
  const hits = []
  let match
  QUEUED_NAME.lastIndex = 0
  while ((match = QUEUED_NAME.exec(source))) {
    if (QUEUED_CALL_AT.test(source.slice(match.index + match[0].length))) continue
    hits.push({ name: match[1], index: match.index })
  }
  return hits
}

/**
 * Extract a callback body from a call's argument text.
 *
 * Handles inline arrows (block & expression body) and inline
 * `function` / `async function` expressions — including a return annotation
 * whose type contains braces (`function (tx): Promise<{ ok: boolean }> { … }`);
 * the body is located by `bodyBraceAfterParams`, never by the first `{`.
 * A **bare identifier** is returned as `ref` so the caller can resolve it
 * against the same-file local function table. Anything else yields
 * `{ body: null, ref: null }` — the caller must treat that as a fail-closed
 * violation, never as "nothing to scan".
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
      // `indexOf('{')` 会把返回注解里的类型字面量当正文：`function (tx):
      // Promise<{ ok: boolean }> { … }` 的正文曾被截成 ` ok: boolean `，
      // 真实回调体（含排队入口）整条漏扫。注解感知定位见 bodyBraceAfterParams。
      const bodyOpen = bodyBraceAfterParams(args, open + params.length + 2, { sameLine: false })
      if (bodyOpen === -1) return { body: null, ref: null }
      return { body: readBalanced(args, bodyOpen, '{', '}'), ref: null }
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
  i += 1
  while (i < source.length && /\s/.test(source[i])) i += 1
  // 冒号后**首个 token 就是 `{`**：三元后件的对象字面量（`ok ? runSaveTxn(cb) :
  // { ok: false }`），不是返回类型注解——注解里深度 0 的 `{` 只会是对象**类型**
  // 字面量（`): { a: 1 } {`），那类声明本就在不追踪清单里；按非定义处理的方向是
  // 把调用点扫出来（fail closed，评审：三元对象尾曾让整个调用点被跳过）
  if (source[i] === '{') return false
  let depth = 0
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

/// 提取正文里被**调用或引用**的标识符，排除关键字/保留字。调用形态
/// （`name(` / `name?.(` / `name<T>(`）之外，**裸引用同样收集**——`list.map(persistOne)`
/// 的 `persistOne`、别名赋值、作为实参传递的函数，都可能随后被调用；引用式
/// 传参曾让传递路径漏追（评审）。集合是「正文里出现过的非关键字标识符」，
/// 交由 localFunctions 表解析，方向保守（多一条边只多扫一份正文）。
export function calledNames(body) {
  const names = new Set()
  const idRe = /\b([A-Za-z_$][\w$]*)/g
  let match
  while ((match = idRe.exec(body))) {
    const name = match[1]
    if (!NOT_CALLS.has(name)) names.add(name)
  }
  return names
}

/// 从 `afterName` 起解析一个 **函数声明** 的正文（无正文返回 null）。
///
/// 评审抓到的漏报形态：`function pick(): () => void {` 的返回类型注解里也有
/// `=>`，旧实现把它当成箭头函数的箭头、正文截成 "void"。修正：注解里的
/// 括号、方括号、泛型（`Array<{...}>`）与 `=>` 逐 token 跳过，对象类型注解
/// （`(): { a: 1 } {`）按「类型字面量后紧跟正文」消歧（见
/// `bodyBraceAfterParams`）；裸 `=` 或 `;` 意味着不是函数声明形状，返回 null。
function declBody(source, afterName) {
  const openParen = source.indexOf('(', afterName)
  if (openParen === -1) return null
  const params = readBalanced(source, openParen, '(', ')')
  if (params === null) return null
  const bodyOpen = bodyBraceAfterParams(source, openParen + params.length + 2, {
    sameLine: false,
    bailOnEquals: true,
  })
  if (bodyOpen === -1) return null
  return readBalanced(source, bodyOpen, '{', '}')
}

/// 从 `=>` 之后解析箭头函数正文：块体取配对花括号；表达式体截到 `;` 或换行
///（跳过字符串字面量，多行表达式会被截断——见文件头局限）。字符串跳过用共享
/// `skipStringLiteral`；未闭合字面量按「吞到文件尾」处理（方向：多扫不误放）。
function arrowBody(source, afterArrow) {
  let i = afterArrow
  while (i < source.length && /\s/.test(source[i])) i += 1
  if (source[i] === '{') return readBalanced(source, i, '{', '}')
  let j = i
  while (j < source.length) {
    const ch = source[j]
    if (ch === '"' || ch === "'" || ch === '`') {
      const end = skipStringLiteral(source, j)
      if (end === -1) {
        j = source.length
        break
      }
      j = end
      continue
    }
    if (ch === ';' || ch === '\n') break
    j += 1
  }
  return source.slice(i, j)
}

/// 从赋值 `=` 之后解析「赋值箭头」正文：可选 `async` → 参数（**配平括号**或裸
/// 标识符）→ 可选同行返回注解 → `=>` → arrowBody。形状不符返回 null——大量
/// 普通赋值（`count = 0`、`this.x = obj`）会走到这里快速失败，方向保守。
///（评审：旧实现 `\([^)]*\)` 在嵌套括号默认值 `(a = (1, 2))` 处截断，类字段
/// 箭头 `h = () => …` 与函数表达式常量则完全没有分支。）
function assignArrowBody(source, afterEq) {
  let i = afterEq
  while (i < source.length && /\s/.test(source[i])) i += 1
  if (source.startsWith('async', i) && /\s/.test(source[i + 5] ?? '')) {
    i += 5
    while (i < source.length && /\s/.test(source[i])) i += 1
  }
  if (source[i] === '(') {
    const params = readBalanced(source, i, '(', ')')
    if (params === null) return null
    i += params.length + 2
  } else {
    const id = /^[A-Za-z_$][\w$]*/.exec(source.slice(i))
    if (id === null) return null
    i += id[0].length
  }
  while (i < source.length && (source[i] === ' ' || source[i] === '\t')) i += 1
  if (source[i] === ':') {
    // 同行返回注解：逐 token 跳到深度 0 的 `=>`（泛型走 readAngles、括号走
    // readBalanced；换行即放弃——注解跨行不在追踪面）
    i += 1
    let depth = 0
    while (i < source.length) {
      const ch = source[i]
      if (ch === '\n' || ch === ';') return null
      if (ch === '=' && source[i + 1] === '>' && depth === 0) return arrowBody(source, i + 2)
      if (ch === '(') {
        const inner = readBalanced(source, i, '(', ')')
        if (inner === null) return null
        i += inner.length + 2
        continue
      }
      if (ch === '<') {
        const end = readAngles(source, i)
        if (end === null) return null
        i = end
        continue
      }
      if (ch === '(' || ch === '[' || ch === '{' || ch === '<') depth += 1
      else if (ch === ')' || ch === ']' || ch === '}' || ch === '>') depth = Math.max(0, depth - 1)
      i += 1
    }
    return null
  }
  if (source[i] === '=' && source[i + 1] === '>') return arrowBody(source, i + 2)
  return null
}

/// 参数闭括号之后定位正文 `{`（方法/包装器定义与内联 `function` 回调共用）。
/// 允许一段返回类型注解：注解里的 `=>`、括号、方括号、泛型逐 token 跳过。
/// **对象类型注解与正文的消歧**：`): { a: 1 } {` 的第一个 `{` 是类型字面量——
/// 读平衡后若（同一行空白之后）紧跟另一个 `{`，后者才是正文；否则第一个就是
/// 正文（`): Promise<void> {`）。`sameLine` 模式遇换行即放弃（方法定义只认
/// 同一行）；非 sameLine 模式（内联 function 表达式）允许注解/正文换行。
/// 遇到 `;` / `)` / `,` / `}` / `?`（条件类型）即放弃——`call()` 后接三元、
/// 逗号或下一条语句不是定义；`bailOnEquals` 供 `function` 声明保留旧行为
///（顶层裸 `=` 不是声明形态）。返回 `{` 的**绝对下标**，找不到返回 -1。
///
/// **两种配平读取的返回值形态不同**：`readBalanced` 返回括号内正文（推进
/// 长度 + 2），`readAngles` 返回闭括号后的**绝对下标**。混用（旧实现对两者
/// 都写 `closer - i`）会得到两种错误：正文为空（注解里的 `()`）时游标归零、
/// 从文件头重扫 → 单行文件死循环；正文非空时变 NaN → 静默放弃整条注解。
///（评审：注解/方法体只允许出现在参数括号的**同一行**——跳过前导换行会让
/// `log(1)` 之后下一行的无关 if 块被当成方法体，产生误报。）
function bodyBraceAfterParams(source, from, { sameLine, bailOnEquals = false }) {
  let i = from
  while (i < source.length) {
    const ch = source[i]
    if (ch === '\n' || ch === '\r') {
      if (sameLine) return -1
      i += 1
      continue
    }
    if (ch === ';' || ch === ')' || ch === ',' || ch === '}' || ch === '?') return -1
    if (ch === '"' || ch === "'" || ch === '`') {
      // 字符串字面量类型（`'a{b'`）里的标点不得参与正文定位；注解里的模板
      // 字面量类型可跨行，sameLine 模式仍按旧口径放弃
      const end = skipStringLiteral(source, i)
      if (end === -1) return -1
      if (sameLine && source.slice(i, end).includes('\n')) return -1
      i = end
      continue
    }
    if (ch === '(') {
      const inner = readBalanced(source, i, '(', ')')
      if (inner === null) return -1
      i += inner.length + 2
      continue
    }
    if (ch === '[') {
      const inner = readBalanced(source, i, '[', ']')
      if (inner === null) return -1
      i += inner.length + 2
      continue
    }
    if (ch === '<') {
      const end = readAngles(source, i)
      if (end === null) return -1
      i = end
      continue
    }
    if (ch === '{') {
      const inner = readBalanced(source, i, '{', '}')
      if (inner !== null) {
        let j = i + inner.length + 2
        while (j < source.length && (source[j] === ' ' || source[j] === '\t')) j += 1
        // 紧跟第二个 `{`：前一个 `{` 是对象类型注解（`): { a: 1 } {`）
        if (source[j] === '{') return j
      }
      return i
    }
    if (ch === '=') {
      if (source[i + 1] === '>') {
        i += 2
        continue
      }
      if (bailOnEquals) return -1
    }
    i += 1
  }
  return -1
}

/// 方法定义（含包装器 / 转发入口）：注解与正文只允许在同一行。
function sameLineBodyOpen(source, from) {
  return bodyBraceAfterParams(source, from, { sameLine: true })
}

/**
 * 同一文件内的局部函数表，覆盖本仓实际会写的形态：
 * - `function name(...)` 声明：返回类型注解里的括号、泛型（含 `Array<{...}>`
 *   这类泛型内对象类型）与 `=>` 逐 token 跳过；注解与正文之间可以换行
 * - 函数表达式常量：`const name = function (...) { … }`（匿名/具名、可 async）
 * - 赋值箭头：`name = (...) => { … }` / `name = x => expr`——含**类字段箭头**
 *   （`h = () => …`）、`this.onSave = (v) => …`、解构后的重绑定；参数括号配平
 *   读取，嵌套括号默认值 `(a = (1, 2))` 不再截断（评审：`\[^)]*\` 单层括号）
 * - 类/对象方法 `name(...) {`、带泛型/返回注解的 `name<T>(...): R {`（含 getter）
 *   与对象方法形态；注解与方法体只允许出现在参数括号的**同一行**
 *   （`sameLineBodyOpen`：换行/`;`/`)`/`,`/`}`/`?` 即放弃——`call()` 后接三元/
 *   下一条语句不是定义，见文件头局限）。对象类型注解（`(): { a: 1 } {`）通过
 *   「类型字面量后紧跟正文 `{`」消歧
 * - 对象字面量属性：`{ run: async () => { … } }` / `{ run: function () { … } }`
 *   ——键名即入口名（评审第二十二轮补；方向保守，普通 `key: value` 快速失败）
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
  let match
  // `function name(...)` 声明（declBody 里逐 token 跳注解，泛型/对象类型均可）
  const fnDeclRe = /\bfunction\s+([A-Za-z_$][\w$]*)/g
  while ((match = fnDeclRe.exec(source))) {
    add(match[1], declBody(source, fnDeclRe.lastIndex))
  }
  // 函数表达式常量：`const name = function (...) { … }`（`function` 关键字之后
  // 交给 declBody——indexOf('(') 自然跳过可选的具名/泛型段）
  const fnExprRe = /\b(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*(?::[^=;\n]*)?=\s*(?:async\s+)?function\b/g
  while ((match = fnExprRe.exec(source))) {
    add(match[1], declBody(source, fnExprRe.lastIndex))
  }
  // 赋值箭头：正则只定位 `name =`（排除 `==`/`=>`/`<=`/`>=`/`!=`），参数、可选
  // 同行注解与 `=>` 在 assignArrowBody 里逐 token 走。普通赋值快速失败返回
  // null，方向保守（多收条目只多扫正文）。
  const assignRe = /\b([A-Za-z_$][\w$]*)\s*(?::[^=;\n]*)?=(?![=>])\s*(?:async\b)?/g
  while ((match = assignRe.exec(source))) {
    add(match[1], assignArrowBody(source, assignRe.lastIndex))
  }
  // **对象字面量属性形态**（评审第二十二轮）：`{ run: async () => { … } }` /
  // `{ run: function () { … } }`——键名即调用入口名。过去冒号形态完全不进表，
  // 回调里 `svc.run()` 只带成员名、间接死锁路径漏追（门禁绿而运行时会排队等
  // 自己）。方向保守：`{ key: value }` 普通值快速失败；三元 `a : b => c` 之类
  // 伪键会多收正文，只加扫描面不加假绿。
  const propFnRe = /\b([A-Za-z_$][\w$]*)\s*:\s*(?:async\s+)?function\b/g
  while ((match = propFnRe.exec(source))) {
    add(match[1], declBody(source, propFnRe.lastIndex))
  }
  const propArrowRe = /\b([A-Za-z_$][\w$]*)\s*:\s*(?:async\s+)?/g
  while ((match = propArrowRe.exec(source))) {
    add(match[1], assignArrowBody(source, propArrowRe.lastIndex))
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
    const bodyOpen = sameLineBodyOpen(source, cursor + params.length + 2)
    if (bodyOpen === -1) continue
    add(name, readBalanced(source, bodyOpen, '{', '}'))
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
    // 与 localFunctions 的方法分支共用 sameLineBodyOpen（同一套跳法）
    const bodyOpen = sameLineBodyOpen(source, open + params.length + 2)
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
 * 判定按**调用形态**（`QUEUED_ENTRY`）——辅助函数里的裸引用（取别名/传参）由
 * 全文件 `bareQueuedReferences` 在出现处直接报，不依赖调用链可达性。返回命中
 * 路径，例如 `performSwitch → saveSplitRatio`。
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
/// 定位**定义**不能靠 `indexOf(name + '(')`——文件里更早出现的同名文本（对象
/// 方法、`this.saveSplitRatio(…)` 调用点）会抢先命中，取到的正文是别的块：
/// 真正绕过队列的包装器没被检查（错块放行），或无关块被当成包装器（错块
/// 误报），两个方向都会发生（评审）。逐个出现位置按「参数 → 同行注解 → 块」
/// 判定定义形态（与 localFunctions 方法分支共用 sameLineBodyOpen），同名定义
/// 全部收齐、逐份检查。
function wrapperViolations(file, source) {
  const violations = []
  for (const name of ['updateEditorPrefs', 'saveSplitRatio']) {
    const bodies = []
    const re = new RegExp(`\\b${name}\\s*\\(`, 'g')
    let match
    while ((match = re.exec(source))) {
      const open = match.index + match[0].length - 1
      const params = readBalanced(source, open, '(', ')')
      if (params === null) continue
      const bodyOpen = sameLineBodyOpen(source, open + params.length + 2)
      if (bodyOpen === -1) continue
      const body = readBalanced(source, bodyOpen, '{', '}')
      if (body !== null) bodies.push(body)
    }
    if (bodies.length === 0) {
      violations.push(
        `${file}: ${name} 的方法定义无法定位（公开保存入口被删除/改名，或出现文本均非定义形态）——` +
          '请同步本门禁与文档',
      )
      continue
    }
    for (const body of bodies) {
      if (!body.includes('runSaveTxn')) {
        violations.push(`${file}: ${name} 未经营业队列（应经 runSaveTxn，否则并发保存会丢更新）`)
      }
      if (body.includes('updateConfigNow')) {
        violations.push(`${file}: ${name} 直连 updateConfigNow（绕过保存队列）`)
      }
    }
  }
  return violations
}

/**
 * 规则 4：`switchObservers.add(listener)` 的监听器会在事务回调内被 await
 * （`performSwitch` 的 finally）。监听器若（直接/经同文件辅助函数）调用排队
 * 保存入口，就是「监听器等队列、队列等回调、回调等监听器」的死锁。监听器形态
 * 无法解析时同样 fail closed。
 *
 * 注册形态按**逐出现点**判定（评审：`const reg = switchObservers.add` 别名与
 * `switchObservers?.add(…)` 可选链都能让 `switchObservers\.add\s*\(` 匹配落空，
 * 监听器不被扫描）。每个 `switchObservers` 出现只允许四种良性形态——
 * `= new Set` 声明、`.add(…)`（含 `?.` 可选链）、`.delete(…)`（移除监听器）、
 * 展开读取 `[...switchObservers]`；其余一切（方括号取值、`add.bind`、整体赋值
 * 为现成 Set、把 Set 传出去、裸引用）一律 fail closed——「add 之后不是 `(`」
 * 的形态无法穷举，与规则 2 同一哲学。
 */
export function observerViolations(path, source, functions) {
  const violations = []
  const registrationOpens = []
  const re = /\bswitchObservers\b/g
  let match
  while ((match = re.exec(source))) {
    let i = match.index + match[0].length
    while (i < source.length && /\s/.test(source[i])) i += 1
    const failClosed = (why) => {
      const snippet = source
        .slice(match.index, Math.min(match.index + 60, source.length))
        .replace(/\s+/g, ' ')
      violations.push(
        `${path}: switchObservers 的${why}形态无法静态分析（${snippet}…）——` +
          '监听器在事务回调内被 await，若其调用排队保存会死锁；' +
          '请用 switchObservers.add(命名函数或内联箭头)，注册面的别名/改写会让门禁 fail closed',
      )
    }
    if (source[i] === '=') {
      // 只放行 `new Set` 声明；整体赋值为现成 Set 可能把带监听器的集合换进来
      let j = i + 1
      while (j < source.length && /\s/.test(source[j])) j += 1
      if (source.startsWith('new Set', j)) continue
      failClosed('整体赋值')
      continue
    }
    if (source[i] === ']') continue // 展开读取 `[...switchObservers]`：只读
    if (source[i] === '?') i += 1 // 可选链 `?.add(`
    if (source[i] === '.') {
      let j = i + 1
      while (j < source.length && /\s/.test(source[j])) j += 1
      const member = /^([A-Za-z_$][\w$]*)/.exec(source.slice(j))
      if (member?.[1] === 'add') {
        let k = j + 3
        while (k < source.length && /\s/.test(source[k])) k += 1
        if (source[k] === '(') {
          registrationOpens.push(k)
          continue
        }
        failClosed('注册')
        continue
      }
      if (member?.[1] === 'delete') continue // 移除监听器：良性
      failClosed('注册')
      continue
    }
    failClosed('使用')
  }
  for (const open of registrationOpens) {
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
      // 调用形态（裸引用由全文件 bareQueuedReferences 在出现处直接报）
      const mention = body.match(QUEUED_ENTRY)
      if (mention) {
        violations.push(
          `${path}: switchObservers 监听器内调用了排队保存入口 \`${mention[1]}\`——` +
            '观察者在事务回调内被 await，会排队等自己（死锁）。监听器只能做读取/对齐',
        )
        continue
      }
      for (const hit of queuedViaLocals(body, functions)) {
        violations.push(
          `${path}: switchObservers 监听器经 ${hit.trail.join(' → ')} 间接引用排队入口 \`${hit.entry}\`——` +
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
    // 全文件裸引用：取别名/传参/属性分离的排队入口——别名可在事务回调里被调用，
    // 回调正文只见别名不见入口名，回调级扫描必然漏（评审：模块顶层
    // `const up = configStore.updateEditorPrefs` + 回调内 `up({})` 曾零违规）。
    // 在**出现处**直接报，fail closed。
    for (const bare of bareQueuedReferences(source)) {
      violations.push(
        `${path}: 排队保存入口 \`${bare.name}\` 以引用形态出现（取别名 / 传参 / 属性分离）——` +
          '调用点无法追踪：在事务回调里经别名调用会排在回调已持有的保存门后，永久挂起；' +
          `请直接以 \`${bare.name}(...)\` 形态调用，事务内改用 tx.updateConfig。片段：` +
          source.slice(Math.max(0, bare.index - 30), bare.index + 60).replace(/\s+/g, ' '),
      )
    }
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
          // 调用形态即违规；裸引用（取别名/传参）由全文件 bareQueuedReferences
          // 在出现处直接报——即使别名在回调外创建，门禁也在原地红
          const mention = body.match(QUEUED_ENTRY)
          if (mention) {
            violations.push(
              `${path}: runSaveTxn 回调内调用了排队保存入口 \`${mention[1]}\`——` +
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
              `${path}: runSaveTxn 回调经 ${hit.trail.join(' → ')} 间接引用排队入口 \`${hit.entry}\`——` +
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
