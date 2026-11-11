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
// a string literal does not read as one either. Specifiers are decoded
// (`decodeStringLiteral`: Unicode, hex and simple escapes) before the layer
// match - an escaped spelling of the service module is still the service module
// at runtime (review: raw-text matching was a silent bypass).
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
// files are scanned through their `<script>` blocks **plus the template's
// executable expressions** (directive bindings, dynamic directive arguments,
// interpolations): template prose never reaches the import matcher, but a
// dynamic `import()` IS a legal template expression and compiles to a real call
// (`@click="import('@/services/x')"`). Specifiers are read with the shared
// string-literal reader (escape- and delimiter-aware), so `"a'b/services/x"` and
// `'a\'b/services/x'` keep their full path. A misplaced service file under
// those directories is therefore scanned — fail-closed on the surface, not just
// on the matcher.
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
  buildLineIndex,
  decodeStringLiteral,
  lineOfIndex,
  prepareSource,
  readStringLiteral,
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
    // 字符串字面量用共享读取器（尊重转义与起始定界符）——`indexOf(quote)`
    // 会在转义引号处提前收尾，把恶意路径的 `/services/` 段留在字面量之外
    //（评审 P2：`import('@/safe\')/services/…')` 曾假绿）
    const literalRead = readStringLiteral(source, argStart)
    if (literalRead === null) {
      const snippet = source.slice(match.index, Math.min(match.index + 48, source.length))
      opaque.push({ index: match.index, snippet: `${snippet.replace(/\s+/g, ' ')}…` })
      continue
    }
    const { raw, end } = literalRead
    const quote = source[argStart]
    if (quote === '`' && raw.includes('${')) {
      opaque.push({ index: match.index, snippet: '`…${…}…`' })
      continue
    }
    let i = end
    while (i < source.length && /\s/.test(source[i])) i += 1
    // 说明符之后只允许 `)` 或 import options 的 `,`——任何其它 token（&& ||
    // ?? + …）都说明这不是纯字面量，fail closed（`'a' && loadB` 的第一个串恒
    // truthy，实际加载的是后面的模块，单读第一个串会假绿）。
    if (source[i] === ')' || source[i] === ',') {
      // 字符串字面量的转义在运行时生效：`@/servi\u0063es/…` 就是 services，
      // 按原文匹配会假绿——解码后再进入层判定（评审 P2）
      literal.push({ spec: decodeStringLiteral(raw), index: match.index })
      continue
    }
    opaque.push({ index: match.index, snippet: `${quote}${raw}${quote} …` })
  }
  return { literal, opaque }
}

/**
 * Every module specifier of a file, read with the shared string-literal reader
 * so quote boundaries follow JavaScript semantics: an escaped delimiter does not
 * end the literal and only the **starting** delimiter closes it (`"a'b/…"` keeps
 * its apostrophe). Static `from`, side-effect `import` and dynamic `import(…)`
 * share the reader; an unreadable literal is reported as opaque (fail closed).
 */
export function moduleSpecifiers(source) {
  const findings = []
  const opaque = []
  for (const match of source.matchAll(/(?<![.\w$'"`])(from|import)\b\s*/g)) {
    const keyword = match[1]
    const afterKeyword = match.index + match[0].length
    if (keyword === 'import') {
      // `import(...)` is handled by dynamicImportArgs; `import.meta` / `import {`
      // / `import type` carry no specifier here (the `from` keyword does).
      if (source[afterKeyword] === '(') continue
    }
    const literalRead = readStringLiteral(source, afterKeyword)
    if (literalRead === null) continue // 不是字符串位置（`from` 作标识符等）
    findings.push({ spec: decodeStringLiteral(literalRead.raw), index: match.index })
  }
  return { findings, opaque }
}

/**
 * One finding per (occurrence, rule) — an import hitting both layers reports
 * both. See the header for the scanned shapes and the dynamic-import
 * fail-closed rule.
 */
export function findViolations(files) {
  const violations = []
  for (const { path, content } of files) {
    // 模板可执行表达式也要扫：动态 `import()` 是合法模板表达式
    //（`@click="import('@/services/x')"` 编译成真实调用——评审 P2）
    const source = prepareSource(path, content, { templateExpressions: true })
    const lines = buildLineIndex(source)
    const findings = moduleSpecifiers(source).findings
    const dynamic = dynamicImportArgs(source)
    findings.push(...dynamic.literal)
    for (const { index, snippet } of dynamic.opaque) {
      violations.push(
        `${path}:${lineOfIndex(source, index, lines)}: 动态 import 的说明符无法静态分析（${snippet}）；` +
          '分层门禁 fail closed——请改用字符串字面量（不含 ${} / 拼接 / 变量）或静态 import。' +
          '规则 A/B 无法确认目标模块时按违规处理（否则门禁会假绿）',
      )
    }
    for (const { spec, index } of findings) {
      const line = lineOfIndex(source, index, lines)
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
