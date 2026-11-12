// Shared source-scanning primitives for the repository's static source gates
// (`scripts/check-save-gate.mjs` and `scripts/check-layering.mjs`).
//
// Why one module: the comment stripper, the `.vue` script extraction and the
// directory walk are load-bearing for both gates. A fidelity fix applied to one
// copy would silently drift from the other — that class of divergence has
// bitten before (a Vue template apostrophe deciding how a later `/* … */`
// reads, an unquoted regex literal truncating the rest of the file).
// Both gates now prepare sources through `prepareSource`.
//
// Fidelity notes, stated rather than implied:
//   - `stripComments` MASKS comments with spaces (newlines preserved), so
//     offsets and line numbers of the prepared source are identical to the raw
//     input. String literals are kept verbatim (`https://` must not read as a
//     line comment). **Unquoted regex literals are consumed verbatim** using the
//     standard division-vs-regex heuristic (previous meaningful token; keywords
//     like `return`/`typeof` precede a regex, values like `)`/`]`/identifiers/
//     quotes/numbers/strings/regexes precede a division, `}` is treated as an
//     ASI boundary and therefore regex), so `/[/*]/` no longer truncates the
//     file — an improvement over the pre-2026-11-10 stripper, which deleted the
//     rest of the source in that shape. Char classes `[...]` are respected
//     (a `/` inside them does not end the regex). The heuristic is imperfect by
//     nature; keep exotic shapes out of scanned sources or disclose them here.
//     Control-statement closing parens (`if`/`for`/`while`/`switch`/`catch`)
//     are regex context: `/` after them starts a regex, not a division —
//     `if (ok) /[/*]/.test(v)` scans intact. Member calls are excluded
//     (`promise.catch(h)` — the word is preceded by `.`, so its closing paren
//     stays a value boundary; without the guard the `/` would be consumed as a
//     regex, which is fail-closed but noisy). Why the direction is safe: a
//     mis-detected regex is consumed VRBATIM (offsets/content unchanged), so the
//     failure mode is comment residue → a false positive, never a silent miss.
//   - Template expressions (directive bindings / interpolations / dynamic
//     arguments) are taken from the AST node's **decoded `content`** — Vue
//     decodes the full HTML5 entity table when parsing the template, so the
//     node text IS the runtime code (`import&lpar;('@/services/x')` compiles to
//     a real dynamic import; a raw source slice plus a six-name entity table
//     missed `&lpar;` / `&sol;` / … and stayed silent). The decoded text is
//     then comment-masked with the same
//     stripper: expressions are **code fragments**, and a block comment can
//     split a token pair the matchers key on (`import/*c*/('@/services/x')` was
//     a real dynamic import the raw text read as neither `import(` nor an
//     unreadable argument — silent green, review). String literals stay
//     verbatim, so `'https://…'` and `//` inside a string are still data.
//   - `readBalanced` skips strings, comments and regex literals with the same
//     heuristic, so `/[}]/` inside a transaction callback no longer truncates
//     the body it is scanning.
//   - For `.vue` files the `<script>` / `<script setup>` blocks are extracted
//     with the REAL SFC parser (`@vue/compiler-sfc` `parse`) — not regexes — so
//     whitespace before `>` in a closing tag, HTML-commented script examples and
//     similar shapes parse exactly like the framework does. Everything outside
//     those blocks is masked with spaces (newlines preserved). Parse failures
//     throw: the gates fail closed instead of scanning an empty surface.
//   - Template executable expressions (directive bindings + interpolations) are
//     extracted from the template AST by `templateExpressions` for the save
//     gate; HTML comments are COMMENT nodes and never yield expressions.

import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
// 与应用同一解析器：'vue/compiler-sfc' 是 vue 包挂出的子路径（Vite 的
// @vitejs/plugin-vue 也走它）——直接钉 @vue/compiler-sfc 的版本会与 vue 的
// 内部版本各自漂移。
import { parse } from 'vue/compiler-sfc'

const CONTROL_KEYWORDS = new Set(['if', 'for', 'while', 'switch', 'catch', 'with'])
const REGEX_PRECEDING_KEYWORDS = new Set([
  'return', 'typeof', 'instanceof', 'in', 'of', 'new', 'delete', 'void',
  'throw', 'case', 'do', 'else', 'yield', 'await', 'default',
])

/**
 * `(` 是否控制语句条件括号（其闭括号后是语句起点、随后的 `/` 是正则）。
 * 含 `for await (` 头：`await` 单独出现不算（`await (x) / 2` 是除法），
 * 只有紧跟 `for` 时才是控制语境（`for await (const x of y) /re/` 曾
 * 被当除法而截断后续扫描）。
 */
function isControlParenHead(prevWord, prevPrevWord, prevWordIsMember) {
  if (prevWordIsMember) return false
  if (CONTROL_KEYWORDS.has(prevWord)) return true
  return prevWord === 'await' && prevPrevWord === 'for'
}

/**
 * Skip a regex literal starting at `start` (the opening `/`). Returns the index
 * just past the literal (flags included) or `-1` when unterminated. `/` inside a
 * character class does not end the literal.
 */
export function skipRegexLiteral(source, start) {
  if (source[start] !== '/') return -1
  let i = start + 1
  let inClass = false
  while (i < source.length) {
    const ch = source[i]
    if (ch === '\\') {
      i += 2
      continue
    }
    if (ch === '\n') return -1
    if (ch === '[') inClass = true
    else if (ch === ']') inClass = false
    else if (ch === '/' && !inClass) {
      i += 1
      while (i < source.length && /[a-z]/i.test(source[i])) i += 1
      return i
    }
    i += 1
  }
  return -1
}

/**
 * Skip a string literal starting at `start` (the quote). Returns the index just
 * past the closing quote or `-1` when unterminated.
 *
 * **Template literals are nested-aware**: `${ … }` is scanned as JavaScript
 * (strings, comments, regex heuristics, nested templates, braces) so
 * `` `a${`b`}c` `` does not desync the tokenizer — the previous shallow scan
 * treated everything up to the next backtick as literal text, so a `/*` or `//`
 * inside a real string could flip comment states and either hide code or flag a
 * commented-out call (review item 6). CRLF line continuations are consumed as a
 * unit so a bare `\n` is never left dangling.
 */
export function skipStringLiteral(source, start) {
  const quote = source[start]
  if (quote !== "'" && quote !== '"' && quote !== '`') return -1
  let i = start + 1
  while (i < source.length) {
    const ch = source[i]
    if (ch === '\\') {
      if (source[i + 1] === '\r' && source[i + 2] === '\n') {
        i += 3
        continue
      }
      i += 2
      continue
    }
    if (ch === quote) return i + 1
    if (quote === '`' && ch === '$' && source[i + 1] === '{') {
      const end = skipTemplateExpression(source, i + 2)
      if (end === -1) return -1
      i = end
      continue
    }
    if (ch === '\n' && quote !== '`') return -1
    i += 1
  }
  return -1
}

/// `${ … }` 内部：按 JS 词法扫到配对 `}`，返回其后的下标。
function skipTemplateExpression(source, start) {
  let depth = 1
  let i = start
  let prevChar = ''
  let prevWord = ''
  let prevPreviousWord = ''
  let prevWordIsMember = false
  const parenStack = []
  while (i < source.length) {
    const ch = source[i]
    const next = source[i + 1]
    if (ch === '/' && next === '/') {
      while (i < source.length && source[i] !== '\n') i += 1
      continue
    }
    if (ch === '/' && next === '*') {
      const end = source.indexOf('*/', i + 2)
      if (end === -1) return -1
      i = end + 2
      continue
    }
    if (ch === '/' && isRegexStart(prevChar, prevWord)) {
      const end = skipRegexLiteral(source, i)
      if (end === -1) return -1
      i = end
      prevChar = ')'
      prevWord = ''
      continue
    }
    if (ch === '"' || ch === "'" || ch === '`') {
      const end = skipStringLiteral(source, i)
      if (end === -1) return -1
      i = end
      prevChar = ch
      prevWord = ''
      continue
    }
    if (/\s/.test(ch)) {
      i += 1
      continue
    }
    if (/[A-Za-z0-9_$]/.test(ch)) {
      const s = i
      while (i < source.length && /[A-Za-z0-9_$]/.test(source[i])) i += 1
      prevPreviousWord = prevWord
      prevWord = source.slice(s, i)
      prevWordIsMember = source[s - 1] === '.'
      prevChar = source[i - 1]
      continue
    }
    if (ch === '(') {
      parenStack.push(isControlParenHead(prevWord, prevPreviousWord, prevWordIsMember))
      prevChar = ch
      prevWord = ''
      i += 1
      continue
    }
    if (ch === ')') {
      prevChar = (parenStack.pop() ?? false) ? ';' : ')'
      prevWord = ''
      i += 1
      continue
    }
    if (ch === '{') {
      depth += 1
      prevChar = ch
      prevWord = ''
      i += 1
      continue
    }
    if (ch === '}') {
      depth -= 1
      if (depth === 0) return i + 1
      prevChar = ch
      prevWord = ''
      i += 1
      continue
    }
    prevChar = ch
    prevWord = ''
    i += 1
  }
  return -1
}

/**
 * Drop comments but keep string literals, masking comment characters with
 * spaces (newlines preserved — offsets stay identical to the raw input).
 *
 * Unquoted regex literals are consumed verbatim using the standard
 * division-vs-regex heuristic; see the module header for the disclosed limits.
 */
export function stripComments(source) {
  let out = ''
  let i = 0
  let prevChar = '' // last meaningful (non-whitespace) code char seen
  let prevWord = '' // last identifier/keyword token, '' if prevChar isn't a word char
  let prevPreviousWord = ''
  let prevWordIsMember = false // 上一个词前紧邻 `.`（成员访问，不是关键词语境）
  // 括号栈：记录每个 `(` 是否为控制语句条件括号（if/for/while/switch/catch 后）。
  // 控制括号的闭括号之后是语句起点，随后的 `/` 是正则；分组括号的闭括号是值，
  // 随后的 `/` 是除法（`if (ok) /[/*]/.test(value)` 曾被截断）。
  const parenStack = []
  const maskTo = (start, end) => {
    while (start < end) {
      out += source[start] === '\n' ? '\n' : ' '
      start += 1
    }
  }
  const emitWord = (from, to) => {
    out += source.slice(from, to)
    prevPreviousWord = prevWord
    prevWord = source.slice(from, to)
    // 成员访问的 `catch`（`promise.catch(h)`）不是控制语句：前面那个 `.` 让
    // 括号不被压成「条件语境」，否则 `p.catch(h)` 的 `/` 会被当正则消费
    //（方向是 fail closed（注释残留→假阳），但仍应消除）。
    prevWordIsMember = source[from - 1] === '.'
  }
  while (i < source.length) {
    const ch = source[i]
    const next = source[i + 1]
    if (ch === '/' && next === '/') {
      const start = i
      while (i < source.length && source[i] !== '\n') i += 1
      maskTo(start, i)
      continue
    }
    if (ch === '/' && next === '*') {
      const start = i
      i += 2
      while (i < source.length && !(source[i] === '*' && source[i + 1] === '/')) i += 1
      const end = Math.min(i + 2, source.length)
      maskTo(start, end)
      i = end
      continue
    }
    if (ch === '/' && isRegexStart(prevChar, prevWord)) {
      const end = skipRegexLiteral(source, i)
      if (end !== -1) {
        out += source.slice(i, end)
        i = end
        prevChar = ')' // a regex is a value: a following `/` is division
        prevWord = ''
        continue
      }
      // 未闭合的正则按除法处理（继续走标点分支）
    }
    if (ch === '"' || ch === "'" || ch === '`') {
      const quote = ch
      const end = skipStringLiteral(source, i)
      if (end !== -1) {
        // 模板字面量整段原样保留（`${}` 里的表达式文本也保留——扫描器只在
        // 其中找 import 形状，保留只会多扫不会漏扫）
        out += source.slice(i, end)
        i = end
        prevChar = quote // a string is a value: a following `/` is division
        prevWord = ''
        continue
      }
      // 未闭合：原样输出该字符，继续扫描（fail closed 方向）
    }
    if (ch === '\n') {
      out += '\n'
      i += 1
      // 行边界不清空 prevChar：`return\n/re/` 仍是正则先行词
      continue
    }
    if (/\s/.test(ch)) {
      out += ch
      i += 1
      continue
    }
    if (/[A-Za-z0-9_$]/.test(ch)) {
      const start = i
      while (i < source.length && /[A-Za-z0-9_$]/.test(source[i])) i += 1
      emitWord(start, i)
      prevChar = source[i - 1]
      continue
    }
    if (ch === '(') {
      out += ch
      parenStack.push(isControlParenHead(prevWord, prevPreviousWord, prevWordIsMember))
      prevChar = ch
      prevWord = ''
      i += 1
      continue
    }
    if (ch === ')') {
      out += ch
      // 控制括号闭括号之后是语句起点：视同 `;`（随后 `/` 是正则）
      prevChar = (parenStack.pop() ?? false) ? ';' : ')'
      prevWord = ''
      i += 1
      continue
    }
    out += ch
    i += 1
    prevChar = ch
    prevWord = ''
  }
  return out
}

function isRegexStart(prevChar, prevWord) {
  if (prevChar === '') return true
  // 值之后（括号/中括号闭合、字符串、上一个正则）是除法
  if (prevChar === ')' || prevChar === ']' || prevChar === '"' || prevChar === "'" || prevChar === '`' || prevChar === '/') {
    return false
  }
  if (/[A-Za-z0-9_$]/.test(prevChar)) {
    // 数字/标识符后是除法；关键词（return/typeof/…）后是正则
    return REGEX_PRECEDING_KEYWORDS.has(prevWord)
  }
  // 运算符/标点（= ( , : ! & | ? ; { } < > 等）后是正则；`}` 按 ASI 边界归正则
  return true
}

/**
 * Vue SFC: extract the exact `<script>` / `<script setup>` content ranges with
 * the real framework parser (`@vue/compiler-sfc`), so whitespace before `>` in a
 * closing tag, HTML-commented script examples and related shapes parse exactly
 * like the framework does. Returns `[start, end)` offsets in source order.
 * Throws when the SFC cannot be parsed — callers must fail closed.
 */
/**
 * Parse an SFC once, failing closed on BOTH throw and returned errors.
 * `parse()` normally reports problems in `errors` instead of throwing; the
 * previous implementation only caught throws, so a file with an unclosed
 * `<script setup>` still produced a (partial) AST and the gate scanned a
 * surface it had not really read.
 */
export function parseSfc(source) {
  let parsed
  try {
    parsed = parse(source, { filename: 'gate-input.vue' })
  } catch (error) {
    throw new Error(`SFC 解析失败（fail closed）: ${error instanceof Error ? error.message : String(error)}`)
  }
  const errors = parsed.errors ?? []
  if (errors.length > 0) {
    const first = errors[0]
    const message = first instanceof Error ? first.message : String(first?.message ?? first)
    throw new Error(`SFC 解析错误（fail closed，共 ${errors.length} 条）: ${message}`)
  }
  return parsed.descriptor
}

function scriptRangesOf(descriptor) {
  const ranges = []
  for (const block of [descriptor.script, descriptor.scriptSetup]) {
    if (block) ranges.push([block.loc.start.offset, block.loc.end.offset])
  }
  ranges.sort((a, b) => a[0] - b[0])
  return ranges
}

export function vueScriptRanges(source) {
  return scriptRangesOf(parseSfc(source))
}

/**
 * Vue SFC: keep only the script block contents; mask every other character with
 * a space (newlines preserved, so reported line/column numbers stay exact).
 * Extraction uses the real SFC parser — see `vueScriptRanges`.
 */
export function scriptBlocksOnly(source) {
  return keepRanges(source, vueScriptRanges(source))
}

/** 用已解析的 descriptor 生成 script 扫描面（避免同文件重复 parse）。 */
export function scriptSurfaceOf(source, descriptor) {
  return keepRanges(source, scriptRangesOf(descriptor))
}

function keepRanges(source, ranges) {
  let out = ''
  let cursor = 0
  const maskTo = (end) => {
    while (cursor < end) {
      out += source[cursor] === '\n' ? '\n' : ' '
      cursor += 1
    }
  }
  for (const [start, end] of ranges) {
    maskTo(start)
    out += source.slice(start, end)
    cursor = end
  }
  maskTo(source.length)
  return out
}

/**
 * Vue 模板可执行表达式提取（两个门禁共用）：**指令表达式**（`@…` / `v-…` /
 * `:…` 绑定的 `exp`）与**插值**（`{{ … }}` 的 content）都是可执行代码，必须与
 * `<script>` 一起进入扫描面。提取走 `vue/compiler-sfc` 的模板 AST（与应用同一
 * 解析器）：HTML 注释（COMMENT 节点）与静态属性自然不在集合内，散文里的伪指令
 * 不再误报。
 *
 * 表达式文本取 AST 节点**已解码的 `content`**（Vue 解析模板时按完整 HTML5
 * 实体表解码——`node.exp.content` / 插值 `node.content.content` /
 * 动态参数 `node.arg.content` 都是运行时代码）；**不再**用原文切片配自带
 * 实体表：表只含 6 个命名实体，`&lpar;` / `&sol;` / `&period;` 等可让
 * `import&lpar;('@/services/x')` 对门禁不可见而编译产物是真实动态 import
 *。注释仍在解码后经 `stripComments` 掩码——
 * 表达式是**代码片段**，`import` 与调用括号之间夹一段块注释仍是真实动态
 * import（曾静默假绿）；字符串字面量原样保留。
 *
 * 动态参数（`@[expr]` / `v-slot:[expr]` 的 arg）与指令 exp、插值一并提取——
 * arg 是**可执行表达式**（`@[runSaveTxn(h)]="go"` 在运行时是一次真实调用）。
 *
 * 单一**遍历器**：本函数是唯一实现（收集 `{ text, line }`，按行序——同文件的
 * 非交叠表达式行序即源码序）；`templateExpressions` 从它派生。两份 walker 会
 * 漂移（一个修了动态参数、另一个漏掉）。
 */
export function templateExpressionEntries(source, descriptor = parseSfc(source)) {
  const ast = descriptor.template?.ast
  if (!ast) return []
  const entries = []
  const push = (loc, content) => {
    if (!loc || typeof content !== 'string') return
    // content 已是解码后的代码；注释掩码保持（`import/*c*/(…)` 的块注释会把
    // matcher 依赖的 token 对拆开，原样扫描会静默假绿）
    const text = stripComments(content)
    if (text.trim() === '') return
    entries.push({ text, line: loc.start.line })
  }
  const walk = (node) => {
    if (!node) return
    // 7 = DIRECTIVE：exp 是绑定表达式（v-for 的 exp 覆盖整段 `item in items`，
    // 原样喂给保存门禁无害）
    if (node.type === 7 && node.exp) push(node.exp.loc, node.exp.content)
    // 5 = INTERPOLATION：content 是插值表达式
    else if (node.type === 5 && node.content) push(node.content.loc, node.content.content)
    // 动态指令参数 `@[expr]="handler"` / `v-slot:[expr]`：arg 是**可执行表达式**
    //（`isStatic === false`）。只收 exp 会漏掉它——`@[runSaveTxn(h)]="go"` 在
    // 运行时是一次真实调用
    if (node.type === 7 && node.arg && node.arg.isStatic === false) {
      push(node.arg.loc, node.arg.content)
    }
    for (const child of node.children ?? []) walk(child)
    // 指令住在元素的 props 里（children 之外），必须单独遍历
    for (const prop of node.props ?? []) walk(prop)
  }
  walk(ast)
  entries.sort((a, b) => a.line - b.line)
  return entries
}

/** 与 `templateExpressionEntries` 相同集合的表达式文本（保存门禁拼接扫描面用）。 */
export function templateExpressions(source, descriptor = parseSfc(source)) {
  return templateExpressionEntries(source, descriptor).map((entry) => entry.text)
}

/**
 * Comment-stripped, gate-aware source for one file (offsets are stable).
 *
 * - `.vue`: script blocks (parser-extracted) are kept verbatim, everything else
 *   is masked. With `templateExpressions: true`（两个门禁都开），template
 *   executable expressions — directive bindings, dynamic directive arguments
 *   and interpolations (`{{ … }}`), extracted from the template AST — are
 *   appended after the script body. The layering gate needs them too: a
 *   dynamic `import()` is a legal template expression
 *   (`@click="import('@/services/x')"` compiles to a real call).
 * - other extensions: comment-masked source as-is.
 */
export function prepareSource(path, content, { templateExpressions: includeTemplateExpressions = false } = {}) {
  if (!path.endsWith('.vue')) return stripComments(content)
  // **单次解析**：descriptor 同时喂给 script 面与模板表达式提取（每个
  // SFC 被 parseSfc 两次，解析失败时第二次还会重复抛错）
  const descriptor = parseSfc(content)
  let prepared = stripComments(scriptSurfaceOf(content, descriptor))
  if (includeTemplateExpressions) {
    const expressions = templateExpressions(content, descriptor)
    if (expressions.length > 0) {
      prepared += `\n${expressions.join('\n')}`
    }
  }
  return prepared
}

/** 1-based line number of `index` in `source`. `lineStarts`（可选，来自
 * `buildLineIndex`）避免每个违规定位都从头重扫——文件大、违规多时是 O(n·m)。 */
export function lineOfIndex(source, index, lineStarts) {
  if (lineStarts !== undefined) {
    let lo = 0
    let hi = lineStarts.length - 1
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1
      if (lineStarts[mid] <= index) lo = mid
      else hi = mid - 1
    }
    return lo + 1
  }
  let line = 1
  for (let i = 0; i < index && i < source.length; i += 1) {
    if (source[i] === '\n') line += 1
  }
  return line
}

/** 每行起始偏移（0-based 行号 → 偏移），配 `lineOfIndex` 做二分定位。 */
export function buildLineIndex(source) {
  const starts = [0]
  for (let i = 0; i < source.length; i += 1) {
    if (source[i] === '\n') starts.push(i + 1)
  }
  return starts
}

/**
 * Read a JavaScript string literal starting at `quoteIndex` (the opening quote).
 *
 * Honors backslash escapes — so `'@/foo\'bar/services/x'` does not terminate at
 * the escaped quote, and only the **starting** delimiter closes the literal, so
 * `"@/foo'bar/services/x"` keeps its apostrophe. Preserves escape sequences
 * verbatim in `raw` (decode them with `decodeStringLiteral`); line continuations
 * survive here and are removed by the decoder. Returns `{ raw, end }` or `null`
 * when the literal is unterminated or a single/double-quoted literal contains a
 * raw newline — callers must fail closed on `null` (review: regex `[^'"]+`
 * capture stopped at the *other* quote type and at escaped quotes, so three
 * specifier shapes went green).
 */
export function readStringLiteral(source, quoteIndex) {
  const quote = source[quoteIndex]
  if (quote !== "'" && quote !== '"' && quote !== '`') return null
  let i = quoteIndex + 1
  let raw = ''
  while (i < source.length) {
    const ch = source[i]
    if (ch === '\\') {
      // 行续接的 CRLF 要整段吞掉：只吃 `\` + `\r` 会把裸 `\n` 留在字面量里，
      // 被下面的「单/双引号不允许裸换行」判成未闭合（CRLF 夹具曾因此漏报）
      if (source[i + 1] === '\r' && source[i + 2] === '\n') {
        raw += '\\\r\n'
        i += 3
        continue
      }
      raw += ch + (source[i + 1] ?? '')
      i += 2
      continue
    }
    if (ch === quote) return { raw, end: i + 1 }
    if (ch === '\n' && quote !== '`') return null
    raw += ch
    i += 1
  }
  return null
}

/**
 * Read a balanced `<...>` generic argument/parameter list starting at `start`
 * (the opening `<`). Nested generics (`Array<Map<string, T>>`) and string
 * literals inside the list are handled. **`=>` inside the list is skipped**
 * (function-type annotations like `(e: Event) => void` — the `>` of `=>` must
 * not decrease the depth, review P3: the shallow `</>` balance truncated at the
 * arrow and dropped the whole call name). **Single-line only**: any newline
 * inside the list returns `null` — multi-line generic arguments are outside
 * the tracked shapes, and a cross-line scan used to run past the newline (the
 * old `depth === 0` guard was dead: depth is ≥ 1 from the first character) to
 * an unrelated `>` further down (review). Returns the index just past the
 * closing `>` or `null` when unbalanced / newline-crossing.
 */
export function readAngles(source, start) {
  if (source[start] !== '<') return null
  let depth = 0
  let i = start
  while (i < source.length) {
    const ch = source[i]
    if (ch === '"' || ch === "'" || ch === '`') {
      const literal = readStringLiteral(source, i)
      if (literal === null) return null
      i = literal.end
      continue
    }
    if (ch === '=' && source[i + 1] === '>') {
      i += 2
      continue
    }
    if (ch === '<') depth += 1
    else if (ch === '>') {
      depth -= 1
      if (depth === 0) return i + 1
    } else if (ch === '\n') {
      return null
    }
    i += 1
  }
  return null
}

/**
 * Read a balanced bracket body starting at `start` (which points at the opener).
 *
 * Skips string literals, comments and regex literals so braces inside them
 * cannot unbalance the scan. The regex/division decision reuses the same
 * heuristic as `stripComments` (control-statement closing parens are regex
 * context; member `.catch(` is not) — review: `/[}]/` inside a transaction
 * callback used to truncate the body and hide the queued save behind it.
 */
export function readBalanced(source, start, open, close) {
  let depth = 0
  let i = start
  let prevChar = ''
  let prevWord = ''
  let prevPreviousWord = ''
  let prevWordIsMember = false
  const parenStack = []
  while (i < source.length) {
    const ch = source[i]
    const next = source[i + 1]
    if (ch === '/' && next === '/') {
      while (i < source.length && source[i] !== '\n') i += 1
      continue
    }
    if (ch === '/' && next === '*') {
      i += 2
      while (i < source.length && !(source[i] === '*' && source[i + 1] === '/')) i += 1
      i += 2
      continue
    }
    if (ch === '/' && isRegexStart(prevChar, prevWord)) {
      const end = skipRegexLiteral(source, i)
      if (end === -1) return null
      i = end
      prevChar = ')'
      prevWord = ''
      continue
    }
    if (ch === '"' || ch === "'" || ch === '`') {
      const quote = ch
      const end = skipStringLiteral(source, i)
      if (end === -1) return null
      i = end
      prevChar = quote
      prevWord = ''
      continue
    }
    if (/\s/.test(ch)) {
      i += 1
      continue
    }
    if (/[A-Za-z0-9_$]/.test(ch)) {
      const wordStart = i
      while (i < source.length && /[A-Za-z0-9_$]/.test(source[i])) i += 1
      prevPreviousWord = prevWord
      prevWord = source.slice(wordStart, i)
      prevWordIsMember = source[wordStart - 1] === '.'
      prevChar = source[i - 1]
      continue
    }
    if (ch === '(') {
      // 深度计数与括号栈各管一段：计数服务 readBalanced 的配平，栈服务
      // 「闭括号后是否正则语境」
      if (ch === open) depth += 1
      parenStack.push(isControlParenHead(prevWord, prevPreviousWord, prevWordIsMember))
      prevChar = ch
      prevWord = ''
      i += 1
      continue
    }
    if (ch === ')') {
      if (ch === close) {
        depth -= 1
        if (depth === 0) return source.slice(start + 1, i)
      }
      prevChar = (parenStack.pop() ?? false) ? ';' : ')'
      prevWord = ''
      i += 1
      continue
    }
    if (ch === open) depth += 1
    else if (ch === close) {
      depth -= 1
      if (depth === 0) return source.slice(start + 1, i)
    }
    prevChar = ch
    prevWord = ''
    i += 1
  }
  return null
}

/**
 * Decode string-literal escapes in a module specifier before the layer match.
 * `'@/servi\u0063es/config.service'` is `@/services/...` at runtime — the gate
 * must not go green on the raw spelling (review).
 *
 * Covers `\uXXXX`, `\u{…}`, `\xXX`, **identity escapes** (`\c` → `c`,
 * `\e` → `e`): JavaScript drops the backslash for any non-special character, so
 * `'@/servi\ces/config.service'` is the service module too, and **line
 * continuations** (`\` + LF / CRLF / LS / PS — a backslash-newline inside a
 * string literal produces nothing, so `"@/servi\<newline>ces/…"` is the
 * service module — round-twelve review). Plus `\\`, quotes, `\b`, `\f`,
 * `\v`, `\0`, `\n`, `\r`, `\t`. An unmatched `\u`/`\x` (invalid escape, file
 * would not compile) stays verbatim — conservative.
 */
export function decodeStringLiteral(raw) {
  // 行续接先消除：`\` + 行终止符在字符串里不产生任何字符（ES LineContinuation）。
  // 放在转义总替换之前——`.` 不匹配换行，续接会落到「未识别」而原样保留。
  const joined = raw.replace(/\\(?:\r\n|[\n\r\u2028\u2029])/g, '')
  // 前导 `\\` 已消耗掉那个反斜杠，末段只能是 `(.)`——写成 `\\(.)` 需要两个
  // 反斜杠，身份转义（`\c`）永远不会命中（简单转义仍漏报）。
  return joined.replace(/\\(?:u\{([0-9a-fA-F]+)\}|u([0-9a-fA-F]{4})|x([0-9a-fA-F]{2})|(.))/g, (m, cp, u4, x2, simple) => {
    if (cp !== undefined) {
      const code = Number.parseInt(cp, 16)
      return Number.isFinite(code) && code <= 0x10ffff ? String.fromCodePoint(code) : m
    }
    if (u4 !== undefined) return String.fromCharCode(Number.parseInt(u4, 16))
    if (x2 !== undefined) return String.fromCharCode(Number.parseInt(x2, 16))
    const map = { '\\': '\\', "'": "'", '"': '"', n: '\n', r: '\r', t: '\t', b: '\b', f: '\f', v: '\v', 0: '\0' }
    if (simple in map) return map[simple]
    if (simple === 'u' || simple === 'x') return m // 非法转义（源文件本就编译不过）
    return simple // identity escape：反斜杠被 JS 丢弃
  })
}

// 模板表达式的 HTML 实体解码已不再需要：表达式文本直接取 AST 节点的
// `content`（Vue 解析时按**完整 HTML5 实体表**解码，见
// `templateExpressionEntries`）。自带六实体表配原文切片漏掉 `&lpar;` /
// `&sol;` 等命名实体，曾让模板动态 import 静默假绿。

/**
 * Walk sources for a gate.
 *
 * @param {string} root repository root
 * @param {object} options
 * @param {string[]} options.dirs directories relative to `src/`
 * @param {string[]} [options.files] individual files relative to `src/`
 * @param {RegExp} options.extensions file-name test (e.g. `/\.(ts|vue)$/`)
 * @param {(relPath: string) => boolean} [options.skip] exclusion predicate
 * @returns {Array<{path: string, content: string}>} paths relative to `src/`, forward slashes
 */
export function walkSources(root, { dirs, files: rootFiles = [], required = [], extensions, skip = () => false }) {
  const srcDir = join(root, 'src')
  const files = []
  const walk = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const full = join(dir, entry.name)
      if (entry.isDirectory()) {
        walk(full)
        continue
      }
      if (!extensions.test(entry.name)) continue
      const rel = relative(srcDir, full).split('\\').join('/')
      if (skip(rel)) continue
      files.push({ path: rel, content: readFileSync(full, 'utf8') })
    }
  }
  for (const dir of dirs) walk(join(srcDir, dir))
  for (const rootFile of rootFiles) {
    const full = join(srcDir, rootFile)
    if (!statSync(full).isFile()) {
      throw new Error(`扫描面不完整：缺少 ${rootFile}（fail-closed）`)
    }
    files.push({ path: rootFile, content: readFileSync(full, 'utf8') })
  }
  // 关键文件必须出现在扫描结果里：文件被改名/挪目录时，按 path 匹配的专项检查
  // （如 save-gate 的包装规则）会整体蒸发而门禁仍绿
  const present = new Set(files.map((file) => file.path))
  const missing = required.filter((rel) => !present.has(rel))
  if (missing.length > 0) {
    throw new Error(`扫描面不完整：缺少关键文件 ${missing.join('、')}（fail-closed）`)
  }
  return files
}
