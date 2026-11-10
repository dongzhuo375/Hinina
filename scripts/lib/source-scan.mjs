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
//   - For `.vue` files the `<script>` / `<script setup>` blocks are extracted
//     with the REAL SFC parser (`@vue/compiler-sfc` `parse`) — not regexes — so
//     whitespace before `>` in a closing tag, HTML-commented script examples and
//     similar shapes parse exactly like the framework does. Everything outside
//     those blocks is masked with spaces (newlines preserved). Parse failures
//     throw: the gates fail closed instead of scanning an empty surface.
//   - The `<script …>` opener is sliced at its first `>`, so an attribute value
//     containing `>` would misplace the block start (no such form in the tree).

import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import { parse } from '@vue/compiler-sfc'

const REGEX_PRECEDING_KEYWORDS = new Set([
  'return', 'typeof', 'instanceof', 'in', 'of', 'new', 'delete', 'void',
  'throw', 'case', 'do', 'else', 'yield', 'await',
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
  const maskTo = (start, end) => {
    while (start < end) {
      out += source[start] === '\n' ? '\n' : ' '
      start += 1
    }
  }
  const emitWord = (from, to) => {
    out += source.slice(from, to)
    prevWord = source.slice(from, to)
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
  try {
    descriptor = parse(source, { filename: 'gate-input.vue' }).descriptor
  } catch (error) {
    throw new Error(`SFC 解析失败（fail closed）: ${error instanceof Error ? error.message : String(error)}`)
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
 * Vue 模板指令表达式提取（供保存门禁使用）：`@…` / `v-…` / `:` 绑定的属性值是
 * 可执行代码，必须与 `<script>` 一起进入扫描面；静态属性（class/id/…）不取。
 * 只在 `<script>` 块之外提取（script 里的同形字符串不参与）——与
 * `scriptBlocksOnly` 相反的掩码方向：模板保留、script 块掩码。
 * 已知边界：HTML 实体（&quot; 等）不展开；同引号转义不存在于合法 Vue 模板。
 */
export function templateDirectiveExpressions(source, scriptRanges) {
  // 保留 script 块**之外**的区域（模板指令在这里），script 块本身掩码——
  // 与 `scriptBlocksOnly`（保留 script、掩码其余）方向相反。
  let masked = ''
  let cursor = 0
  const keepTo = (end) => {
    masked += source.slice(cursor, end)
    cursor = Math.max(cursor, end)
  }
  const maskTo = (end) => {
    while (cursor < end) {
      masked += source[cursor] === '\n' ? '\n' : ' '
      cursor += 1
    }
  }
  for (const [start, end] of scriptRanges) {
    keepTo(start)
    maskTo(end)
  }
  keepTo(source.length)
  const expressions = []
  const re = /(?:^|\s)(?:@|v-|:)[\w.:\-[\]]+\s*=\s*(?:"([^"]*)"|'([^']*)')/g
  let match
  while ((match = re.exec(masked))) {
    const value = match[1] ?? match[2]
    if (value && value.trim() !== '') expressions.push(value)
  }
  return expressions
}

/**
 * Comment-stripped, gate-aware source for one file (offsets are stable).
 *
 * - `.vue`: script blocks (parser-extracted) are kept verbatim, everything else
 *   is masked. With `templateExpressions: true`（保存门禁），template directive
 *   expressions — executable code — are appended after the script body so the
 *   save gate can scan handlers; the layering gate keeps script-only scanning
 *   (imports cannot appear in template expressions).
 * - other extensions: comment-masked source as-is.
 */
export function prepareSource(path, content, { templateExpressions = false } = {}) {
  if (!path.endsWith('.vue')) return stripComments(content)
  const ranges = vueScriptRanges(content)
  let prepared = stripComments(keepRanges(content, ranges))
  if (templateExpressions) {
    const expressions = templateDirectiveExpressions(content, ranges)
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

/** Read a balanced bracket body starting at `start` (which points at the opener),
 * skipping string literals so braces inside strings cannot unbalance the scan. */
export function readBalanced(source, start, open, close) {
  let depth = 0
  let i = start
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
    if (ch === open) depth += 1
    else if (ch === close) {
      depth -= 1
      if (depth === 0) return source.slice(start + 1, i)
    }
    i += 1
  }
  return null
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
