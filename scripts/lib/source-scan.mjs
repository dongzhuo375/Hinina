// Shared source-scanning primitives for the repository's static source gates
// (`scripts/check-save-gate.mjs` and `scripts/check-layering.mjs`).
//
// Why one module: the comment stripper, the `.vue` script extraction and the
// directory walk are load-bearing for both gates. A fidelity fix applied to one
// copy would silently drift from the other — review round five found exactly
// that class of divergence (a Vue template apostrophe deciding how a later
// `/* … */` reads, an unquoted regex literal truncating the rest of the file).
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
//   - Template expressions (directive bindings / interpolations) are appended
//     WITHOUT comment stripping: they are expression fragments, and any `//`
//     inside them can only add scan text (conservative direction). They are
//     HTML-entity decoded first (`&#114;unSaveTxn` is a real runSaveTxn at
//     runtime); numeric references and the common named set only.
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
// 内部版本各自漂移（评审 P3）。
import { parse } from 'vue/compiler-sfc'

const CONTROL_KEYWORDS = new Set(['if', 'for', 'while', 'switch', 'catch', 'with'])
const REGEX_PRECEDING_KEYWORDS = new Set([
  'return', 'typeof', 'instanceof', 'in', 'of', 'new', 'delete', 'void',
  'throw', 'case', 'do', 'else', 'yield', 'await', 'default',
])

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
  let prevWordIsMember = false // 上一个词前紧邻 `.`（成员访问，不是关键词语境）
  // 括号栈：记录每个 `(` 是否为控制语句条件括号（if/for/while/switch/catch 后）。
  // 控制括号的闭括号之后是语句起点，随后的 `/` 是正则；分组括号的闭括号是值，
  // 随后的 `/` 是除法（评审第七轮 P2：`if (ok) /[/*]/.test(value)` 曾被截断）。
  const parenStack = []
  const maskTo = (start, end) => {
    while (start < end) {
      out += source[start] === '\n' ? '\n' : ' '
      start += 1
    }
  }
  const emitWord = (from, to) => {
    out += source.slice(from, to)
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
      // Regex literal: consume verbatim, `/` inside `[...]` does not end it.
      out += ch
      i += 1
      let inClass = false
      while (i < source.length) {
        const rch = source[i]
        if (rch === '\\') {
          out += rch + (source[i + 1] ?? '')
          i += 2
          continue
        }
        if (rch === '[') inClass = true
        else if (rch === ']') inClass = false
        else if (rch === '/' && !inClass) {
          out += rch
          i += 1
          break
        }
        out += rch
        i += 1
      }
      // flags
      while (i < source.length && /[a-z]/i.test(source[i])) {
        out += source[i]
        i += 1
      }
      prevChar = ')' // a regex is a value: a following `/` is division
      prevWord = ''
      continue
    }
    if (ch === '"' || ch === "'" || ch === '`') {
      const quote = ch
      const start = i
      out += ch
      i += 1
      while (i < source.length) {
        out += source[i]
        if (source[i] === '\\') {
          out += source[i + 1] ?? ''
          i += 2
          continue
        }
        if (source[i] === quote) {
          i += 1
          break
        }
        i += 1
      }
      prevChar = quote // a string is a value: a following `/` is division
      prevWord = ''
      continue
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
      parenStack.push(CONTROL_KEYWORDS.has(prevWord) && !prevWordIsMember)
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
export function vueScriptRanges(source) {
  let descriptor
  let errors
  try {
    const parsed = parse(source, { filename: 'gate-input.vue' })
    descriptor = parsed.descriptor
    errors = parsed.errors ?? []
  } catch (error) {
    throw new Error(`SFC 解析失败（fail closed）: ${error instanceof Error ? error.message : String(error)}`)
  }
  // `parse()` 通常把错误放进返回值而不是 throw——只 catch throw 会让门禁在
  // 部分 AST 上继续扫（评审 P4：注释声称的 fail closed 不成立）
  if (errors.length > 0) {
    const first = errors[0]
    const message = first instanceof Error ? first.message : String(first?.message ?? first)
    throw new Error(`SFC 解析错误（fail closed，共 ${errors.length} 条）: ${message}`)
  }
  const ranges = []
  for (const block of [descriptor.script, descriptor.scriptSetup]) {
    if (block) ranges.push([block.loc.start.offset, block.loc.end.offset])
  }
  ranges.sort((a, b) => a[0] - b[0])
  return ranges
}

/**
 * Vue SFC: keep only the script block contents; mask every other character with
 * a space (newlines preserved, so reported line/column numbers stay exact).
 * Extraction uses the real SFC parser — see `vueScriptRanges`.
 */
export function scriptBlocksOnly(source) {
  return keepRanges(source, vueScriptRanges(source))
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
 * Vue 模板可执行表达式提取（供保存门禁使用）：**指令表达式**（`@…` / `v-…` /
 * `:…` 绑定的 `exp`）与**插值**（`{{ … }}` 的 content）都是可执行代码，必须与
 * `<script>` 一起进入扫描面。提取走 `vue/compiler-sfc` 的模板 AST（与应用同一
 * 解析器）：HTML 注释（COMMENT 节点）与静态属性自然不在集合内，散文里的伪指令
 * 不再误报；偏移为文件绝对，直接切片。表达式切片随后经 `decodeHtmlEntities`
 * 解码（`&#114;unSaveTxn` 运行时就是 runSaveTxn）。
 * 已知边界：指令的动态参数（`v-on:[event]`）不提取（Vue 编译器限制其形态，
 * 不含函数调用）；命名实体的表只含常见集合（表外命名实体保持原样）。
 */
export function templateExpressions(source) {
  const { descriptor } = parse(source, { filename: 'gate-input.vue' })
  const ast = descriptor.template?.ast
  if (!ast) return []
  const locs = []
  const walk = (node) => {
    if (!node) return
    // 7 = DIRECTIVE：exp 是绑定表达式（v-for 的 exp 覆盖整段 `item in items`，
    // 原样切片对保存门禁无害）
    if (node.type === 7 && node.exp?.loc) locs.push([node.exp.loc.start.offset, node.exp.loc.end.offset])
    // 5 = INTERPOLATION：content 是插值表达式
    else if (node.type === 5 && node.content?.loc) locs.push([node.content.loc.start.offset, node.content.loc.end.offset])
    // 动态指令参数 `@[expr]="handler"` / `v-slot:[expr]`：arg 是**可执行表达式**
    //（`isStatic === false`）。只收 exp 会漏掉它——`@[runSaveTxn(h)]="go"` 在
    // 运行时是一次真实调用（评审 P2）
    if (node.type === 7 && node.arg && node.arg.isStatic === false && node.arg.loc) {
      locs.push([node.arg.loc.start.offset, node.arg.loc.end.offset])
    }
    for (const child of node.children ?? []) walk(child)
    // 指令住在元素的 props 里（children 之外），必须单独遍历
    for (const prop of node.props ?? []) walk(prop)
  }
  walk(ast)
  locs.sort((a, b) => a[0] - b[0])
  // Vue 编译前会解码 HTML 实体：`&#114;unSaveTxn(…)` 在运行时就是
  // runSaveTxn——原文切片必须先解码再进入扫描面（评审：实体曾可藏调用）
  return locs
    .map(([start, end]) => decodeHtmlEntities(source.slice(start, end)))
    .filter((text) => text.trim() !== '')
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
  const ranges = vueScriptRanges(content)
  let prepared = stripComments(keepRanges(content, ranges))
  if (includeTemplateExpressions) {
    const expressions = templateExpressions(content)
    if (expressions.length > 0) {
      prepared += `\n${expressions.join('\n')}`
    }
  }
  return prepared
}

/** 1-based line number of `index` in `source`. */
export function lineOfIndex(source, index) {
  let line = 1
  for (let i = 0; i < index && i < source.length; i += 1) {
    if (source[i] === '\n') line += 1
  }
  return line
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
 * literals inside the list are handled; returns the index just past the closing
 * `>` or `null` when unbalanced. Used for call-name extraction — the old
 * `<[^<>()]*>` pattern stopped at the inner `<` and silently dropped
 * `persist<Array<string>>(…)` from the called-name set (review P3).
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
    if (ch === '<') depth += 1
    else if (ch === '>') {
      depth -= 1
      if (depth === 0) return i + 1
    } else if (ch === '\n' && depth === 0) {
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
      i += 1
      let inClass = false
      while (i < source.length) {
        const rch = source[i]
        if (rch === '\\') {
          i += 2
          continue
        }
        if (rch === '[') inClass = true
        else if (rch === ']') inClass = false
        else if (rch === '/' && !inClass) {
          i += 1
          break
        }
        i += 1
      }
      while (i < source.length && /[a-z]/i.test(source[i])) i += 1
      prevChar = ')'
      prevWord = ''
      continue
    }
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
      prevWord = source.slice(wordStart, i)
      prevWordIsMember = source[wordStart - 1] === '.'
      prevChar = source[i - 1]
      continue
    }
    if (ch === '(') {
      // 深度计数与括号栈各管一段：计数服务 readBalanced 的配平，栈服务
      // 「闭括号后是否正则语境」
      if (ch === open) depth += 1
      parenStack.push(CONTROL_KEYWORDS.has(prevWord) && !prevWordIsMember)
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
  // 反斜杠，身份转义（`\c`）永远不会命中（评审第十一轮：简单转义仍漏报）。
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

const NAMED_ENTITIES = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'", nbsp: '\u00a0' }

/**
 * Decode HTML character references in template expression text. Vue compiles
 * after entity decoding, so `&#114;unSaveTxn(…)` inside `@click` is a real
 * `runSaveTxn` call while the raw source spells something else (review: the
 * save gate must not go green on that).
 *
 * Mirrors **attribute-value** decoding (directives and interpolations are
 * attribute / text content): numeric references decode with or without the
 * semicolon (`&#114` and `&#x72` and `&#X72`), named references decode with a
 * semicolon and — for the small legacy set here — also without one unless the
 * next character is alphanumeric or `=` (`&amp` → `&`, `&ampx` → verbatim),
 * exactly as the browser parser rules jsdom confirmed. Unknown names stay
 * verbatim (conservative: a false miss only for named entities outside the
 * table).
 */
export function decodeHtmlEntities(text) {
  return text.replace(/&(?:#([0-9]+)|#[xX]([0-9a-fA-F]+)|([a-zA-Z][a-zA-Z0-9]*))(;?)/g, (m, dec, hex, name, semi, offset, whole) => {
    if (dec !== undefined) return String.fromCodePoint(Number.parseInt(dec, 10))
    if (hex !== undefined) return String.fromCodePoint(Number.parseInt(hex, 16))
    if (!(name in NAMED_ENTITIES)) return m
    if (semi === ';') return NAMED_ENTITIES[name]
    const next = whole[offset + m.length] ?? ''
    // 属性语境：无分号的 legacy 实体后接字母数字/等号时浏览器不解码
    if (/[A-Za-z0-9=]/.test(next)) return m
    return NAMED_ENTITIES[name]
  })
}

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
export function walkSources(root, { dirs, files: rootFiles = [], extensions, skip = () => false }) {
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
  return files
}
