// Shared source-scanning primitives for the repository's static source gates
// (`scripts/check-save-gate.mjs` and `scripts/check-layering.mjs`).
//
// Why one module: the comment stripper and the directory walk are load-bearing
// for both gates. A fidelity fix applied to one copy would silently drift from
// the other — and two were needed at once when review found Vue template prose
// feeding the JS tokenizer (an apostrophe in `Don't` flips the string state, so
// whether a later `/* … */` reads as a comment is decided by template text).
// Both gates now prepare sources through `prepareSource`, which keeps only
// `<script>` block contents for `.vue` files.
//
// Fidelity notes, stated rather than implied:
//   - `stripComments` keeps string literals (`https://` must not read as a
//     line comment). An *unquoted* regex literal containing `//` is cut as a
//     line comment — keep regexes quoted or on their own line.
//   - For `.vue` files only `<script>` blocks are scanned; everything else is
//     masked with spaces (newlines preserved so line numbers stay exact).
//     Text content, templates and `<style>` are therefore invisible to the
//     gates — including prose that merely looks like an import.
//   - The `<script …>` opener is sliced at its first `>`, so an attribute value
//     containing `>` would misplace the block start (no such form in the tree).

import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'

/**
 * Drop comments but keep string literals (a URL like `https://…` inside a
 * string must not read as a line comment). Known fidelity note: an *unquoted*
 * regex literal containing `//` is cut as a line comment.
 */
export function stripComments(source) {
  let out = ''
  let i = 0
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
    if (ch === '"' || ch === "'" || ch === '`') {
      const quote = ch
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
      continue
    }
    out += ch
    i += 1
  }
  return out
}

/**
 * Vue SFC: keep only `<script>` block contents; mask every other character with
 * a space (newlines preserved, so reported line/column numbers stay exact).
 *
 * Without this the JS tokenizer would consume template text: prose punctuation
 * (`<p>Don't</p>`) flips the string state, and comment recognition for the real
 * script block is then decided by template characters. A `.vue` file with no
 * script block yields an all-blank scan surface (nothing to report).
 */
export function scriptBlocksOnly(source) {
  const inside = new Uint8Array(source.length)
  const re = /<script\b[^>]*>([\s\S]*?)<\/script>/gi
  let match
  while ((match = re.exec(source))) {
    const start = match.index + match[0].indexOf('>') + 1
    const end = start + match[1].length
    inside.fill(1, start, end)
  }
  let out = ''
  for (let i = 0; i < source.length; i += 1) {
    if (inside[i] === 1) out += source[i]
    else out += source[i] === '\n' ? '\n' : ' '
  }
  return out
}

/** Comment-stripped, `.vue`-aware source for a gate scan (offsets are stable). */
export function prepareSource(path, content) {
  return stripComments(path.endsWith('.vue') ? scriptBlocksOnly(content) : content)
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
