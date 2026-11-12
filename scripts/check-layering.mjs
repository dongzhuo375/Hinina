// Layering static gate: pins the two red lines of the strict
// View → Store → Service rule (doc/Architecture.md「前端分层口径」).
//
//   Rule A — the whole scan surface (View / component / composable, App.vue,
//            and the utils/types **transit roots**) must not import Service
//            (`@/services/*`, the bare `@/services` barrel, or any
//            `*/services/*` specifier, including `import type` / dynamic
//            `import()` / side-effect / re-export).
//   Rule B — no scanned file may import `@/bridge` / `*/bridge/*`, and **no
//            file anywhere under src/ except bridge/ may import any
//            `@tauri-apps/*` package** (CLAUDE.md: bridge is the only place
//            that touches Tauri; a direct `invoke` — or a direct
//            `@tauri-apps/plugin-dialog` call — bypasses the bridge boundary
//            and its IpcError normalization; plugin packages added in review
//            round twenty-two). Rule B's bridge half covers the **whole scan
//            surface**: View / component (+ App.vue), **composables** (a View
//            consuming the composable reaches IPC through it — review round
//            twenty), and the **transit roots** (utils/types — re-exporting
//            bridge is the same router shape; review round eighteen).
//            The Tauri half additionally covers an **extended surface** —
//            stores/, services/, guards/, router/ and main.ts — where bridge
//            imports stay legal (Service→Bridge is the ladder; guards do
//            cross-layer assembly) but Tauri packages are not.
//            Rule A and rule B's bridge half share one scope.
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
// (identifier, concatenation, interpolated template, glob array)
// **fails closed** with a violation — the gate must not go green on a specifier
// it cannot read. The same rule covers `import.meta.glob(...)` /
// `globEager(...)`: Vite compiles them into lazy imports, so they are real
// View→Service edges (review P1). **The directory part of a glob pattern must
// be literal**: Vite resolves a non-relative pattern through the alias plugin
// and then expands it, so `@/**/config.service.ts` / `@/**/*.service.ts` are
// real lazy edges but the literal pattern does not reveal the resolved set.
// Any picomatch set syntax before the last `/` fails closed — `* ? [ ] { }`
// and the extglob parentheses (`@(a|b)`, `+(a|b)`, `!(a)` express directory
// sets without those characters; review round twenty-six), while only the
// filename segment may be wildcarded (`@/services/*.ts` still reports by its
// path segment; `@/assets/*.svg` stays clean). The check runs on the
// **decoded** specifier (Vite expands the AST literal's `element.value`), so
// escape spellings — `@/\u002a/…`, `@\u002f**\u002f…` — cannot hide the
// syntax in a directory segment (review round twenty-five).
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
// Known limits, stated: string literals are preserved by the stripper, so
// **import-shaped text inside a string literal is scanned** — a string such as
// `'x from "@/services/y"'` reports a violation (false positive, fail-closed
// direction; keep prose examples in comments). `require()` is not scanned
// (ESM-only codebase).
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
  parseSfc,
  readStringLiteral,
  scriptSurfaceOf,
  stripComments,
  templateExpressionEntries,
  walkSources,
} from './lib/source-scan.mjs'

/** Scan roots (relative to `src/`, forward slashes) + the single root component. */
export const SCAN_ROOTS = ['views', 'components', 'composables']
/// 横向模块：不在 View 链上，但可以成为**中转路由**——`utils/x.ts` 里
/// `export { configService } from '@/services/…'` 或 `export { ipc } from '@/bridge/…'`，
/// view 只 import 中转文件就绕过了红线。对它们应用规则 A 与规则 B（评审：
/// 规则 B 曾豁免中转面，`view → utils/ipc → bridge` 全绿——重导出 bridge 的
/// utils/types 文件本身就是违规点，View→Bridge 的边在路由处被斩断）。
export const TRANSIT_ROOTS = ['utils', 'types']
export const ROOT_FILES = ['App.vue']
/// Every script-ish extension under the scan roots is scanned, declarations
/// included (see header).
export const SOURCE_EXTENSIONS = /\.(?:d\.ts|ts|tsx|mts|cts|js|jsx|mjs|cjs|vue)$/

/// Rule A: specifiers that resolve into the service layer — including the bare
/// directory barrel (`from '@/services'`), symmetric with BRIDGE_SPEC below.
const SERVICE_SPEC = /(?:^|\/)services(?:\/|$)/
/// Rule B: specifiers that reach the IPC surface — the bridge layer, and any
/// `@tauri-apps/*` package. CLAUDE.md: bridge is **the only place** that
/// touches Tauri; a direct `invoke` (or a direct plugin-dialog call) from a
/// view bypasses the IpcError normalization / bridge boundary (review round
/// twenty; plugin packages added in round twenty-two — `@tauri-apps/api`
/// alone let `@tauri-apps/plugin-dialog` through).
const BRIDGE_SPEC = /(?:^|\/)bridge(?:\/|$)/
const TAURI_SPEC = /^@tauri-apps(?:\/|$)/

/// Rule B 覆盖**全部扫描面**（2026-11-12 第二十轮口径）：View/component/
/// App.vue、composables 与 utils/types 中转面。composable 被 View 消费——
/// 放行 composable 的 bridge 导入等于给 View 留一条经 composable 抵达 IPC 的
/// 路径；中转文件重导出 bridge 同理（边在路由处斩断）。规则 A 与规则 B 的
/// 作用域自此一致。
///
/// **Tauri 红线整树遍历（第二十二轮立规、第二十三轮落实）**：头注/Architecture
/// 的承诺是「src 内除 bridge/ 与 tests 外任何文件都不得 import @tauri-apps/*」——
/// 因此除主扫描面外，`src/` 的**其余全部文件**也进扫描（`main.ts`、`env.d.ts`
/// 与未来新增的顶层目录都在内；枚举 stores/services/guards/router 会漏掉它们，
/// 评审第二十三轮），这些文件只应用 **Tauri 包规则**——import `@/bridge/*` 是
/// 合法梯级（Service→Bridge；guards 是跨层装配），`@tauri-apps/*` 则任何位置
/// 都只能经 bridge（CLAUDE.md:47）。`main.ts` 缺失按扫描面不完整处理。
export const TAURI_ONLY_ROOT_FILES = ['main.ts']

/**
 * Split a dynamic-import argument — `import(...)`, `import.meta.glob(...)` and
 * `import.meta.globEager(...)` — into statically-readable specs and opaque
 * forms. Opaque forms are reported by the caller as fail-closed violations:
 * `import(name)`, `import('prefix' + rest)`, `` import(`…${x}…`) ``,
 * `import.meta.glob(['./a', './b'])`.
 */
export function dynamicImportArgs(source) {
  const literal = []
  const opaque = []
  // `import(...)` 与 Vite 的 `import.meta.glob(...)` / `globEager(...)`：后者在
  // 编译期变成懒加载 import，是真实的 View→Service 边（评审 P1）。说明符规则
  // 与动态 import 完全一致：字面量可判定，其余（数组/变量/拼接/插值）fail closed。
  for (const match of source.matchAll(/(?<![.\w$'"`])import\s*(?:\.\s*meta\s*\.\s*(glob|globEager))?\s*\(\s*/g)) {
    const isGlob = match[1] !== undefined
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
    // 字符串字面量的转义在运行时生效：`@/servi\u0063es/…` 就是 services，
    // 按原文匹配会假绿——所有判定都在**解码后**的说明符上做（评审 P2；第二十五
    // 轮补齐 glob 判定：Vite 展开用的是 AST literal 的 element.value，即解码后
    // 的模式，`@/\u002a/config.service.ts` 运行时就是 `@/*/config.service.ts`）
    const spec = decodeStringLiteral(raw)
    // **glob 的目录段通配 fail closed**（评审第二十四轮）：Vite 对非相对/
    // 非绝对模式先 resolveId（别名插件生效）再展开——`@/**/config.service.ts`、
    // `@/**/*.service.ts` 等都是真实懒加载边，但字面量模式本身看不出解析集合
    //（通配符可跨目录段命中 services/bridge）。只有**文件名段**允许通配
    //（`@/services/*.ts` 的段判据仍成立）；目录段带通配即按不可静态判定处理。
    // 判定用**解码后**的 spec：转义拼写（`\u002a` / `\x2a` / `\u002f`）不能
    // 把通配符藏进目录段（评审第二十五轮）。目录段必须是**字面量**：不再枚举
    // 通配字符，而是把 picomatch 的全部集合语法符号一并拒绝——`* ? [ ] { }`
    // 与括号（extglob 的 `@(a|b)` / `+(a|b)` / `!(a|b)` 不含 `*?[{` 也能表达
    // 目录集合，Vite 的 importGlob 用的就是 tinyglobby；评审第二十六轮）。
    if (isGlob) {
      const lastSlash = spec.lastIndexOf('/')
      const dirPart = lastSlash === -1 ? '' : spec.slice(0, lastSlash)
      if (/[?*[\]{}()]/.test(dirPart)) {
        opaque.push({
          index: match.index,
          snippet: `${quote}${raw}${quote}`,
          reason: 'glob-dir-wildcard',
        })
        continue
      }
    }
    let i = end
    while (i < source.length && /\s/.test(source[i])) i += 1
    // 说明符之后只允许 `)` 或 import options 的 `,`——任何其它 token（&& ||
    // ?? + …）都说明这不是纯字面量，fail closed（`'a' && loadB` 的第一个串恒
    // truthy，实际加载的是后面的模块，单读第一个串会假绿）。
    if (source[i] === ')' || source[i] === ',') {
      literal.push({ spec, index: match.index })
      continue
    }
    opaque.push({ index: match.index, snippet: `${quote}${raw}${quote} …` })
  }
  return { literal, opaque }
}

/**
 * Every **static** module specifier of a file, read with the shared
 * string-literal reader so quote boundaries follow JavaScript semantics: an
 * escaped delimiter does not end the literal and only the **starting**
 * delimiter closes it (`"a'b/…"` keeps its apostrophe). `from` (static /
 * re-export / `import type`) and side-effect `import '<spec>'` are covered here;
 * dynamic `import(…)` and `import.meta.glob(…)` are handled by
 * `dynamicImportArgs`, which fails closed on unreadable specifiers. A `null`
 * literal read here just means the keyword is not followed by a string
 * (`from` as an identifier, `import {` / `import type` / `import.meta`), so it
 * is skipped — there is nothing to fail closed on.
 */
export function moduleSpecifiers(source) {
  const findings = []
  for (const match of source.matchAll(/(?<![.\w$'"`])(from|import)\b\s*/g)) {
    const keyword = match[1]
    const afterKeyword = match.index + match[0].length
    if (keyword === 'import') {
      // `import(...)` / `import.meta…` handled elsewhere; `import {` / `import type`
      // carry no specifier here (the `from` keyword does).
      if (source[afterKeyword] === '(' || source[afterKeyword] === '.') continue
    }
    const literalRead = readStringLiteral(source, afterKeyword)
    if (literalRead === null) continue // 不是字符串位置（`from` 作标识符等）
    findings.push({ spec: decodeStringLiteral(literalRead.raw), index: match.index })
  }
  return { findings }
}

/**
 * Analyse one source unit (script surface or one template expression) and push
 * findings with correct file line numbers: `lineBase` is the 1-based line of
 * the unit's first line, so template expressions keep their **original** lines
 * (appending them to the script surface would misreport lines and let adjacent
 * expressions concatenate into a pseudo import).
 */
function scanUnit(violations, path, unitSource, lineBase, { tauriOnly = false } = {}) {
  const lines = buildLineIndex(unitSource)
  const lineOf = (index) => lineBase + lineOfIndex(unitSource, index, lines) - 1
  const findings = moduleSpecifiers(unitSource).findings
  const dynamic = dynamicImportArgs(unitSource)
  findings.push(...dynamic.literal)
  // 扩展面（stores/services/guards/router/main.ts）只认 Tauri 包：不报动态
  // import 的不可读形态（懒加载拼接在这些层是正常写法，与红灯无关），也不
  // 应用规则 A / bridge 路径规则（Service→Bridge 是合法梯级、guards 是跨层装配）
  if (!tauriOnly) {
    for (const { index, snippet, reason } of dynamic.opaque) {
      const why =
        reason === 'glob-dir-wildcard'
          ? '说明符无法静态分析——glob 通配/集合语法出现在**目录段**（展开集合可跨目录命中 services/bridge，实际解析集合不可判定；目录段必须是字面量）'
          : '说明符无法静态分析'
      violations.push(
        `${path}:${lineOf(index)}: 动态 import / import.meta.glob 的${why}（${snippet}）；` +
          '分层门禁 fail closed——请改用字符串字面量（不含 ${} / 拼接 / 变量 / 数组；glob 只有文件名段可带通配）' +
          '或静态 import。规则 A/B 无法确认目标模块时按违规处理（否则门禁会假绿）',
      )
    }
  }
  for (const { spec, index } of findings) {
    const line = lineOf(index)
    if (!tauriOnly && SERVICE_SPEC.test(spec)) {
      violations.push(
        `${path}:${line}: 规则A —— 扫描面（View/component/composable 与 utils/types 中转面）不得 import Service（${spec}）；` +
          '一律经对应 store 消费（Architecture.md「前端分层口径」）',
      )
    }
    if (!tauriOnly && BRIDGE_SPEC.test(spec)) {
      violations.push(
        `${path}:${line}: 规则B —— View/component/composable（含 utils/types 中转面）不得 import bridge（${spec}）；` +
          'IPC 细节收敛在 service 层，bridge 是唯一接触点（IpcError 归一化在 bridge；Architecture.md「前端分层口径」）',
      )
    }
    if (TAURI_SPEC.test(spec)) {
      violations.push(
        `${path}:${line}: 规则B —— Tauri 包只能由 bridge 接触（${spec}）；` +
          'bridge 是唯一 import @tauri-apps/* 的地方（CLAUDE.md:47）——直接调用绕过 bridge 的 IpcError 归一化' +
          '（本文件位于扩展扫描面：stores/services/guards/router/main 同样适用）',
      )
    }
  }
}

/**
 * One finding per (occurrence, rule) — an import hitting both layers reports
 * both. See the header for the scanned shapes and the dynamic-import
 * fail-closed rule. For `.vue` files the script surface and each template
 * executable expression (directive binding / dynamic argument / interpolation)
 * are scanned as separate units so every finding keeps its real line.
 */
export function findViolations(files) {
  const violations = []
  for (const { path, content, tauriOnly = false } of files) {
    if (path.endsWith('.vue')) {
      const descriptor = parseSfc(content)
      scanUnit(violations, path, stripComments(scriptSurfaceOf(content, descriptor)), 1)
      // `<script src="…">`：外链脚本在编译期被内联进该组件，是真实的静态依赖
      //（评审：src 指向 services/ 时零违规）。src 值本身就是模块说明符，按
      // 规则 A/B 判定；行号取 block 起点。相对说明符的匹配口径与 import 一致
      //（`../services/x`、`@/bridge/x` 命中；同目录裸名不指向层目录，不命中）。
      for (const block of [descriptor.script, descriptor.scriptSetup]) {
        if (!block?.src) continue
        const line = block.loc.start.line
        if (SERVICE_SPEC.test(block.src)) {
          violations.push(
            `${path}:${line}: 规则A —— <script src="${block.src}"> 指向 Service；` +
              '扫描面（含 utils/types 中转面）不得 import Service，一律经对应 store 消费' +
              '（Architecture.md「前端分层口径」）',
          )
        }
        if (BRIDGE_SPEC.test(block.src) || TAURI_SPEC.test(block.src)) {
          violations.push(
            `${path}:${line}: 规则B —— <script src="${block.src}"> 指向 bridge / Tauri 包；` +
              'IPC 细节收敛在 service 层，bridge 是唯一接触点（Architecture.md「前端分层口径」）',
          )
        }
      }
      for (const { text, line } of templateExpressionEntries(content, descriptor)) {
        scanUnit(violations, path, text, line)
      }
      continue
    }
    scanUnit(violations, path, stripComments(content), 1, { tauriOnly })
  }
  return violations
}

/// Walk the scan roots for production sources, plus the root-level files.
/// Paths relative to `src/`, forward slashes.
export function loadSources(root) {
  // 路径段匹配（`utils/__tests__` 这类子串过宽：`utils__tests__helpers` 之类
  // 会被误排除）
  const skip = (rel) => /(^|\/)__tests__(\/|$)/.test(rel) || rel.startsWith('test-support/')
  const files = walkSources(root, {
    dirs: ['views', 'components', 'composables', 'utils', 'types'],
    files: ROOT_FILES,
    extensions: SOURCE_EXTENSIONS,
    skip,
  })
  if (files.length === 0) {
    // 防御假绿：扫描面为空说明根目录结构变了，门禁必须红而不是静默通过
    throw new Error('layering-gate: 扫描面为空（views/components/composables/App.vue 均未找到）')
  }
  // Tauri 红线整树遍历：`src/` 下除 `bridge/` 与 tests 外的**全部文件**都扫
  //（承诺见头注；枚举目录会漏掉 env.d.ts 与未来顶层目录，评审第二十三轮）。
  // 已在主扫描面里的文件**不重复**收集（它们走规则 A/B 全查），其余文件只应用
  // Tauri 包规则。main.ts 缺失按扫描面不完整处理（fail-closed）。
  const seen = new Set(files.map((file) => file.path))
  const tauriOnlyFiles = walkSources(root, {
    dirs: ['.'],
    required: TAURI_ONLY_ROOT_FILES,
    extensions: SOURCE_EXTENSIONS,
    skip: (rel) => skip(rel) || rel.startsWith('bridge/') || seen.has(rel),
  })
  for (const file of tauriOnlyFiles) file.tauriOnly = true
  files.push(...tauriOnlyFiles)
  return files
}

export function runCli(root) {
  let files
  let violations
  try {
    files = loadSources(root)
    // findViolations 也必须收进 try：SFC 解析错误（parseSfc throw）同样是红，
    // runCli 的契约是**返回码**而非异常（评审：坏 .vue 曾把 CLI 变成未捕获异常，
    // 与 save-gate / 空扫描面的口径不一致）
    violations = findViolations(files)
  } catch (error) {
    console.error(`layering-gate: ${error instanceof Error ? error.message : String(error)}`)
    return 1
  }
  if (violations.length === 0) {
    console.log(
      `layering-gate: OK（${files.length} 个文件；扫描面（含 utils/types 中转面）不 import Service / bridge；src 内除 bridge/ 与 tests 外均不 import @tauri-apps/*）`,
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
