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
//      `runSaveTxn`, `updateEditorPrefs`, `saveSplitRatio`. The two wrappers
//      queue too, so calling them inside a callback deadlocks exactly like a
//      nested transaction.
//   3. The two public wrappers must write through `runSaveTxn` and must not
//      reach the private save body directly (that would bypass the queue).
//
// Rule 2 also follows **same-file local helpers** (callback → performSwitch →
// saveSplitRatio), so a helper gaining a queued call is caught. Limits (stated,
// not implied): cross-file helpers, aliases, and dynamic lookup are invisible.
// It guards this repo's conventions, not adversarial code.
//
// Usage:
//   node scripts/check-save-gate.mjs      (also: npm run check:save-gate)

import { readFileSync, readdirSync } from 'node:fs'
import { join, relative } from 'node:path'
import { pathToFileURL } from 'node:url'

/// Files allowed to mention `runSaveTxn` (paths relative to `src/`, forward slashes).
export const TXN_ALLOWLIST = [
  'services/config.service.ts', // definition + the two semantic wrappers
  'composables/useSettingsForm.ts', // the only compound transaction (save → auto-switch)
]

/// Queued save entries: appearing inside a transaction callback body deadlocks.
const QUEUED_ENTRY = /\b(runSaveTxn|updateEditorPrefs|saveSplitRatio)\s*[<(]/

/// Drop comments but keep string literals (`https://` must not read as a comment).
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

/// Read a balanced bracket body starting at `start` (which points at the opener),
/// skipping string literals so braces inside strings cannot unbalance the scan.
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

/// Collect the callback bodies of every `runSaveTxn(...)` *call site*.
/// The definition (`async runSaveTxn<T>(`) is skipped; both arrow forms (block
/// body and expression body) are handled.
export function txnCallbackBodies(source) {
  const bodies = []
  const needle = 'runSaveTxn'
  let from = 0
  for (;;) {
    const hit = source.indexOf(needle, from)
    if (hit === -1) break
    from = hit + needle.length
    if (source.slice(from, from + 3) === '<T>') continue // definition
    const afterName = source.slice(from).match(/^\s*\(/)
    if (!afterName) continue
    const openParen = from + afterName[0].length - 1
    const args = readBalanced(source, openParen, '(', ')')
    if (!args) continue
    const arrow = args.indexOf('=>')
    if (arrow === -1) continue // not a callback call; skip defensively
    const rest = args.slice(arrow + 2)
    const lead = rest.search(/\S/)
    if (lead === -1) continue
    const tail = rest.slice(lead)
    if (tail.startsWith('{')) {
      const braceStart = openParen + 1 + arrow + 2 + lead
      const body = readBalanced(source, braceStart, '{', '}')
      if (body !== null) bodies.push(body)
    } else {
      bodies.push(tail)
    }
  }
  return bodies
}

/// 关键字/保留字：调用点提取时排除（`if (`、`return (` 之类不是函数调用）
const NOT_CALLS = new Set([
  'if', 'for', 'while', 'switch', 'catch', 'function', 'return', 'typeof', 'await',
  'new', 'void', 'delete', 'in', 'of', 'instanceof', 'do', 'else', 'try', 'finally',
  'throw', 'class', 'const', 'let', 'var', 'async', 'yield', 'super', 'this',
])

/// 提取正文里被调用的标识符（`name(` 或 `name<T>(`），排除关键字。
export function calledNames(body) {
  const names = new Set()
  const re = /\b([A-Za-z_$][\w$]*)\s*(?:<[^<>()]*>)?\s*\(/g
  let match
  while ((match = re.exec(body))) {
    if (!NOT_CALLS.has(match[1])) names.add(match[1])
  }
  return names
}

/// 从 `afterName` 起解析一个函数声明/箭头函数的正文（无正文返回 null）。
function declBody(source, afterName) {
  const openParen = source.indexOf('(', afterName)
  if (openParen === -1) return null
  const params = readBalanced(source, openParen, '(', ')')
  if (params === null) return null
  let i = openParen + params.length + 2
  while (i < source.length && source[i] !== '{' && source[i] !== '=' && source[i] !== ';') i += 1
  if (source[i] === '=' && source[i + 1] === '>') {
    i += 2
    while (i < source.length && /\s/.test(source[i])) i += 1
    if (source[i] === '{') return readBalanced(source, i, '{', '}')
    let j = i
    while (j < source.length && source[j] !== ';' && source[j] !== '\n') j += 1
    return source.slice(i, j)
  }
  if (source[i] === '{') return readBalanced(source, i, '{', '}')
  return null
}

/**
 * 同一文件内的局部函数表（`function name(...)` 与 `const name = (...) => {...}`）。
 *
 * 只为「回调 → 局部辅助函数 → 排队入口」这条**传递路径**服务：直接调用由
 * `txnCallbackBodies` 的正文检查覆盖，跨文件/别名/动态调用不在能力范围内
 *（见文件头的局限声明）。
 */
export function localFunctions(source) {
  const table = new Map()
  const declRe = /\bfunction\s+([A-Za-z_$][\w$]*)|\b(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*(?::[^=]+)?=\s*(?:async\s*)?(?:\([^)]*\)|[A-Za-z_$][\w$]*)\s*(?::[^=]+)?=>\s*\{/g
  let match
  while ((match = declRe.exec(source))) {
    const name = match[1] ?? match[2]
    if (name === undefined || table.has(name)) continue
    const isFunction = match[1] !== undefined
    let body = null
    if (isFunction) {
      body = declBody(source, declRe.lastIndex)
    } else {
      // 箭头函数：regex 已定位到 `{` 之前，取该花括号的配对正文
      const braceStart = declRe.lastIndex - 1
      body = readBalanced(source, braceStart, '{', '}')
    }
    if (body !== null) table.set(name, body)
  }
  return table
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
      const fnBody = functions.get(name)
      if (fnBody === undefined) continue
      const nextTrail = [...trail, name]
      const match = fnBody.match(QUEUED_ENTRY)
      if (match) {
        hits.push({ trail: nextTrail, entry: match[1] })
        continue
      }
      if (depth < maxDepth) walk(fnBody, nextTrail, depth + 1, new Set([...seen, name]))
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
 * Check a set of sources.
 * @param {Array<{path: string, content: string}>} files paths relative to `src/`
 * @returns {string[]} human-readable violations (empty = clean)
 */
export function findViolations(files) {
  const violations = []
  for (const { path, content } of files) {
    const source = stripComments(content)
    if (source.includes('runSaveTxn')) {
      if (!TXN_ALLOWLIST.includes(path)) {
        violations.push(
          `${path}: 出现 runSaveTxn，但不在白名单。确认它不在其它事务回调内（或改用 tx 句柄），` +
            '再把文件加进 TXN_ALLOWLIST —— 白名单是有意设置的复核摩擦',
        )
      }
      const functions = localFunctions(source)
      for (const body of txnCallbackBodies(source)) {
        if (QUEUED_ENTRY.test(body)) {
          violations.push(
            `${path}: runSaveTxn 回调内引用了排队保存入口（runSaveTxn/updateEditorPrefs/saveSplitRatio）——` +
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
    if (path === 'services/config.service.ts') {
      violations.push(...wrapperViolations(path, source))
    }
  }
  return violations
}

/// Walk `src/` for production sources (tests/test-support/declarations excluded).
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
  walk(srcDir)
  return files
}

export function runCli(root) {
  const violations = findViolations(loadSources(root))
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
