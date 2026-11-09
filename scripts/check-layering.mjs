// Layering static gate: pins the two red lines of the strict
// View → Store → Service rule (doc/Architecture.md「前端分层口径」).
//
//   Rule A — View / component / composable (+ App.vue) must not import Service
//            (`@/services/*` or any `*/services/*` specifier, including
//            `import type` / dynamic `import()` / side-effect / re-export).
//   Rule B — View / component (+ App.vue) must not import `@/bridge` or any
//            `*/bridge/*` specifier. **Composables are outside rule B**: the
//            registered red line names View / component only (Architecture.md
//            「前端分层口径」first bullet); widen it only with a documented
//            reason.
//
// Both red lines were previously enforced by manual grep ("可 grep 断言"); this
// script makes them a CI check (`npm run check:layering`, ci.yml frontend job),
// same mechanism as check-save-gate.mjs / check-csp-policy.mjs: synthetic
// fixtures pin the verdicts in scripts/__tests__/check-layering.spec.ts and the
// real src/ tree runs in `npm test` too.
//
// Detection scans the WHOLE comment-stripped source (not line-by-line), so
// cross-line import shapes cannot slip through: `from` split from its
// specifier, dynamic `import(...)` wrapped across lines, and side-effect
// `import` split across lines all report; match offsets map back to line
// numbers. Import-keyword matches require a statement boundary (start / `;` /
// `{` / `}` / newline) and must not be preceded by `.` or word chars, so
// property access / tagged templates (`x.import('y')`, `x.from`, …) do not
// read as imports. Comments are stripped first (same string-preserving
// scanner shape as check-save-gate). Known limits, stated:
//   - String literals are preserved, so string content containing
//     `from '@/services/x'` would read as an import (no such form in the
//     current tree — keep prose examples in comments); `require()` is not
//     scanned (ESM-only codebase).
// Path scopes are enforced by construction (the walk only visits views/
// components/ composables/ + App.vue), so a misplaced service file under
// those dirs is still scanned — fail-closed.
//
// Stores/services/guards/router/main.ts are deliberately out of scope: they
// sit on other rungs of the ladder (Store → Service → Bridge is legal; the
// composition root and guards keep their registered service access).
//
// Usage:
//   node scripts/check-layering.mjs      (also: npm run check:layering)

import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import { pathToFileURL } from 'node:url'

/** Scan roots (relative to `src/`, forward slashes) + the single root component. */
export const SCAN_ROOTS = ['views', 'components', 'composables']
export const ROOT_FILES = ['App.vue']

/// Rule A: specifiers that resolve into the service layer — including the bare
/// directory barrel (`from '@/services'`), symmetric with BRIDGE_SPEC below.
const SERVICE_SPEC = /(?:^|\/)services(?:\/|$)/
/// Rule B: specifiers that resolve into the bridge layer.
const BRIDGE_SPEC = /(?:^|\/)bridge(?:\/|$)/

/// Rule A applies to every scanned scope; rule B excludes composables
/// (the registered red line names View / component only).
function ruleBEnabled(relPath) {
  return !relPath.startsWith('composables/')
}

/**
 * Drop comments but keep string literals (a URL like `https://…` inside a
 * string must not read as a line comment). Same shape as check-save-gate's
 * scanner; known fidelity note: an *unquoted* regex literal containing `//`
 * is cut as a line comment (keep regexes quoted or on their own line).
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
 * One finding per (occurrence, rule) — an import hitting both layers reports
 * both. Scans the WHOLE comment-stripped source (not line-by-line) so
 * cross-line import shapes cannot slip through: `from` split from its
 * specifier, dynamic `import(...)` wrapped across lines, and side-effect
 * `import` split across lines all report; match offsets map back to line
 * numbers. Import-keyword matches require a statement boundary (start / `;` /
 * `{` / `}` / newline) and must not be preceded by `.` or word chars, so
 * property access / tagged templates (`x.import('y')`, `x.from`, …) do not
 * read as imports.
 */
export function findViolations(files) {
  const violations = []
  for (const { path, content } of files) {
    const source = stripComments(content)
    const findings = []
    for (const match of source.matchAll(/(?<![.\w$])from\s*['"]([^'"]+)['"]/g)) {
      findings.push({ spec: match[1], index: match.index })
    }
    for (const match of source.matchAll(/(?:^|[;{}\n])\s*import\s*['"]([^'"]+)['"]/g)) {
      findings.push({ spec: match[1], index: match.index + match[0].indexOf('import') })
    }
    for (const match of source.matchAll(/(?<![.\w$])import\s*\(\s*['"]([^'"]+)['"]\s*\)/g)) {
      findings.push({ spec: match[1], index: match.index })
    }
    const lineOf = (index) => {
      let line = 1
      for (let i = 0; i < index; i += 1) if (source[i] === '\n') line += 1
      return line
    }
    for (const { spec, index } of findings) {
      const line = lineOf(index)
      if (SERVICE_SPEC.test(spec)) {
        violations.push(
          `${path}:${line}: 规则A —— View/component/composable 不得 import Service（${spec}）；` +
            '一律经对应 store 消费（Architecture.md「前端分层口径」）',
        )
      }
      if (ruleBEnabled(path) && BRIDGE_SPEC.test(spec)) {
        violations.push(
          `${path}:${line}: 规则B —— View/component 不得 import bridge（${spec}）；` +
            'IPC 细节收敛在 service 层（Architecture.md「前端分层口径」）',
        )
      }
    }
  }
  return violations
}

/// Walk the scan roots for production sources (tests / declarations excluded),
/// plus the root-level files. Paths relative to `src/`, forward slashes.
export function loadSources(root) {
  const srcDir = join(root, 'src')
  const files = []
  const walk = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const full = join(dir, entry.name)
      if (entry.isDirectory()) {
        walk(full)
        continue
      }
      if (!/\.(ts|vue)$/.test(entry.name)) continue
      if (entry.name.endsWith('.d.ts')) continue
      const rel = relative(srcDir, full).split('\\').join('/')
      if (rel.includes('__tests__') || rel.startsWith('test-support')) continue
      files.push({ path: rel, content: readFileSync(full, 'utf8') })
    }
  }
  for (const rootDir of SCAN_ROOTS) walk(join(srcDir, rootDir))
  for (const rootFile of ROOT_FILES) {
    const full = join(srcDir, rootFile)
    if (!statSync(full).isFile()) {
      throw new Error(`layering-gate: 根组件缺失：${rootFile}（fail-closed，扫描面不完整）`)
    }
    files.push({ path: rootFile, content: readFileSync(full, 'utf8') })
  }
  if (files.length === 0) {
    // 防御假绿：扫描面为空说明根目录结构变了，门禁必须红而不是静默通过
    throw new Error('layering-gate: 扫描面为空（views/components/composables/App.vue 均未找到）')
  }
  return files
}

export function runCli(root) {
  let files
  try {
    files = loadSources(root)
  } catch (error) {
    console.error(`layering-gate: ${error instanceof Error ? error.message : String(error)}`)
    return 1
  }
  const violations = findViolations(files)
  if (violations.length === 0) {
    console.log(
      `layering-gate: OK（${files.length} 个文件；View/component/composable 不 import Service，View/component 不 import bridge）`,
    )
    return 0
  }
  console.error('layering-gate: 发现以下分层违规：')
  for (const violation of violations) console.error(`  - ${violation}`)
  return 1
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  try {
    process.exitCode = runCli(process.cwd())
  } catch (error) {
    console.error(`layering-gate: ${error instanceof Error ? error.message : String(error)}`)
    process.exitCode = 1
  }
}
