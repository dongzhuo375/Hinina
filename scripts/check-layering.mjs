// Layering static gate: pins the two red lines of the strict
// View → Store → Service rule (doc/Architecture.md「前端分层口径」).
//
//   Rule A — View / component / composable (+ App.vue) must not import Service
//            (`@/services/*`, the bare `@/services` barrel, or any
//            `*/services/*` specifier, including `import type` / dynamic
//            `import()` / side-effect / re-export).
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
// real src/ tree runs in `npm test` too. Comment stripping / `.vue` handling /
// the directory walk are shared with check-save-gate.mjs via
// scripts/lib/source-scan.mjs (single fidelity surface for both gates).
//
// Detection scans the WHOLE prepared source (not line-by-line), so cross-line
// import shapes cannot slip through: `from` split from its specifier, dynamic
// `import(...)` wrapped across lines, and side-effect `import` split across
// lines all report; match offsets map back to line numbers. Import-keyword
// matches must not be preceded by `.` / word chars / a quote, so property
// access (`x.from`, `x.import('y')`) does not read as an import and prose inside
// a string literal does not read as one either.
//
// Dynamic imports are only checkable when the specifier is statically visible:
// a plain string literal or a backtick literal without `${}`. Every other form
// (identifier, concatenation, interpolated template) **fails closed** with a
// violation — the gate must not go green on a specifier it cannot read.
//
// Scan surface: `views/`, `components/`, `composables/` (recursively) plus
// `src/App.vue`, over `.ts` / `.tsx` / `.js` / `.jsx` / `.mjs` / `.cjs` /
// `.vue` — `.d.ts` included (a declaration file importing the service layer is
// still a view-side edge). `__tests__/` and `test-support/` are excluded. `.vue`
// files are scanned through their `<script>` blocks only (template prose never
// reaches the import matcher). A misplaced service file under those directories
// is therefore scanned — fail-closed on the surface, not just on the matcher.
//
// Known limits, stated: string literals are preserved by the stripper, so a
// *string-literal text* such as `const s = "from '@/services/x'"` is guarded
// only by the quote-lookbehind (no such form in the current tree — keep prose
// examples in comments); `require()` is not scanned (ESM-only codebase).
//
// Stores/services/guards/router/main.ts are deliberately out of scope: they
// sit on other rungs of the ladder (Store → Service → Bridge is legal; the
// composition root and guards keep their registered service access).
//
// Usage:
//   node scripts/check-layering.mjs      (also: npm run check:layering)

import { pathToFileURL } from 'node:url'
import {
  lineOfIndex,
  prepareSource,
  walkSources,
} from './lib/source-scan.mjs'

/** Scan roots (relative to `src/`, forward slashes) + the single root component. */
export const SCAN_ROOTS = ['views', 'components', 'composables']
export const ROOT_FILES = ['App.vue']
/// Every script-ish extension under the scan roots is scanned, declarations
/// included (see header).
export const SOURCE_EXTENSIONS = /\.(?:d\.ts|ts|tsx|js|jsx|mjs|cjs|vue)$/

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
 * Split a `import(...)` argument into statically-readable specs and opaque
 * forms. Opaque forms are reported by the caller as fail-closed violations:
 * `import(name)`, `import('prefix' + rest)`, `` import(`…${x}…`) ``.
 */
export function dynamicImportArgs(source) {
  const literal = []
  const opaque = []
  for (const match of source.matchAll(/(?<![.\w$'"`])import\s*\(\s*/g)) {
    const argStart = match.index + match[0].length
    const quote = source[argStart]
    if (quote === "'" || quote === '"' || quote === '`') {
      const end = source.indexOf(quote, argStart + 1)
      if (end === -1) {
        opaque.push({ index: match.index, snippet: `${quote}…（未闭合）` })
        continue
      }
      const raw = source.slice(argStart + 1, end)
      if (quote === '`' && raw.includes('${')) {
        opaque.push({ index: match.index, snippet: '`…${…}…`' })
        continue
      }
      let i = end + 1
      while (i < source.length && /\s/.test(source[i])) i += 1
      if (source[i] === '+') {
        opaque.push({ index: match.index, snippet: `${quote}${raw}${quote} + …` })
        continue
      }
      literal.push({ spec: raw, index: match.index })
      continue
    }
    const snippet = source.slice(match.index, Math.min(match.index + 48, source.length))
    opaque.push({ index: match.index, snippet: `${snippet.replace(/\s+/g, ' ')}…` })
  }
  return { literal, opaque }
}

/**
 * One finding per (occurrence, rule) — an import hitting both layers reports
 * both. See the header for the scanned shapes and the dynamic-import
 * fail-closed rule.
 */
export function findViolations(files) {
  const violations = []
  for (const { path, content } of files) {
    const source = prepareSource(path, content)
    const findings = []
    // Keywords must not be preceded by a property dot / word char / quote: the
    // dot blocks `x.from`, the word char blocks `myimport`, the quote keeps
    // string-literal prose (`"from '@/services/x'"`) out of the matcher.
    for (const match of source.matchAll(/(?<![.\w$'"`])from\s*['"]([^'"]+)['"]/g)) {
      findings.push({ spec: match[1], index: match.index })
    }
    for (const match of source.matchAll(/(?<![.\w$'"`])import\s*['"]([^'"]+)['"]/g)) {
      // 副作用 import（`import '<spec>'`）；`x.import(...)` 被 lookbehind 挡掉，
      // `import(...)` 走下面的动态分支
      findings.push({ spec: match[1], index: match.index })
    }
    const dynamic = dynamicImportArgs(source)
    findings.push(...dynamic.literal)
    for (const { index, snippet } of dynamic.opaque) {
      violations.push(
        `${path}:${lineOfIndex(source, index)}: 动态 import 的说明符无法静态分析（${snippet}）；` +
          '分层门禁 fail closed——请改用字符串字面量（不含 ${} / 拼接 / 变量）或静态 import。' +
          '规则 A/B 无法确认目标模块时按违规处理（否则门禁会假绿）',
      )
    }
    for (const { spec, index } of findings) {
      const line = lineOfIndex(source, index)
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

/// Walk the scan roots for production sources, plus the root-level files.
/// Paths relative to `src/`, forward slashes.
export function loadSources(root) {
  const files = walkSources(root, {
    dirs: SCAN_ROOTS,
    files: ROOT_FILES,
    extensions: SOURCE_EXTENSIONS,
    skip: (rel) => rel.includes('__tests__') || rel.startsWith('test-support'),
  })
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
