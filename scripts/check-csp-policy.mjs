// Webview CSP policy gate: single source of truth for pinning the CSP shape.
// Reads app.security from src-tauri/tauri.conf.json and fails when the
// directive set, any directive's sources, or the csp -> devCsp superset
// relationship drift from the pinned policy below. Regressions here are
// silent (a `null` csp or a quietly loosened directive produces no error,
// only a wider attack surface), so both the set and each source list are
// exact-matched. Changing the policy is fine; do it consciously by updating
// the pins below together with the rationale in doc/Architecture.md.
//
// Used locally (`npm run check:csp-policy`) and by ci.yml alongside
// check:save-gate; the vitest suite in scripts/__tests__/csp-policy.spec.ts
// imports these functions for mutation coverage.

import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { pathToFileURL, fileURLToPath } from 'node:url'

/// Sources each directive must carry in the production policy (exact set:
/// removals weaken, additions drift — both must touch this gate consciously).
export const PROD_POLICY = {
  'default-src': ["'self'"],
  // No unsafe-eval / unsafe-inline: the app ships no inline scripts and no
  // runtime template compilation; plugins execute in the Rust-side QuickJS.
  'script-src': ["'self'"],
  // 'unsafe-inline' is unavoidable for two real dependents: inline style
  // ATTRIBUTES in DOMPurify-preserved markdown output (v-html parses them as
  // markup, which CSP governs) and Monaco's runtime theme <style> injection.
  // (Vue :style object bindings write via CSSOM — style[k]=v — and are NOT
  // subject to CSP, so they are not a reason.)
  'style-src': ["'self'", "'unsafe-inline'"],
  // Problem statements / announcements may embed arbitrary absolute image
  // URLs, and private-LAN OJ deployments rewrite relative images to plain
  // http absolutes; data: URIs survive DOMPurify for <img> by default.
  // Accepted tradeoff: passive content is loadable from any http(s) host,
  // so tracking pixels in statements can leak the client IP — CSP is static
  // while the OJ origin is runtime configuration, so narrowing to the
  // configured origin is impossible. Event attributes are stripped by
  // DOMPurify; <style>/<link> elements are stripped too (no CSS exfiltration
  // via attribute selectors), see utils/markdown.
  'img-src': ["'self'", 'http:', 'https:', 'data:'],
  // <video>/<audio>/<source>/<track> survive DOMPurify's html profile too;
  // same rationale as img-src — without this the fallback to default-src
  // 'self' would silently block remote media that played before the CSP.
  'media-src': ["'self'", 'http:', 'https:', 'data:'],
  // data: covers Vite's build-time inlined font subsets (under the asset
  // inline threshold — e.g. the cyrillic/emoji unicode-range blocks of the
  // bundled variable fonts). Without it those ranges silently fall back to
  // system fonts in the production exe ONLY (dev CSP is not injected and
  // e2e runs against the dev server, so CI cannot see this regression).
  'font-src': ["'self'", 'data:'],
  // Tauri v2 IPC origins (Windows WebView2 uses http://ipc.localhost). These
  // are load-bearing: the tauri 2.12.5 core has NO automatic connect-src
  // augmentation for IPC, and when the fetch channel is CSP-blocked the IPC
  // layer silently degrades to postMessage with a single console.warn —
  // config drift here is a quiet slowdown, not a loud failure.
  'connect-src': ["'self'", 'ipc:', 'http://ipc.localhost'],
  'worker-src': ["'self'"],
  'object-src': ["'none'"],
  'base-uri': ["'self'"],
  'form-action': ["'self'"],
}

/// Dev-only extras on top of the production policy: the Vite HMR websocket
/// (scheme-source ws: covers both the default same-origin flow and the
/// TAURI_DEV_HOST variant, whose HMR host is whatever that variable holds).
/// Anything else diverging between devCsp and csp is drift, not a tooling
/// need — note in particular that Vite 6's dev-time ?worker wrapper serves a
/// SAME-ORIGIN URL; createObjectURL workers only exist in the build-time
/// `?worker&inline` branch (unused here), so worker-src needs no blob:.
export const DEV_EXTRAS = {
  'connect-src': ['ws:'],
}

export function devPolicy() {
  const expected = {}
  for (const [name, sources] of Object.entries(PROD_POLICY)) {
    expected[name] = [...sources, ...(DEV_EXTRAS[name] ?? [])]
  }
  return expected
}

export function parseDirectives(policy, field, errors) {
  if (typeof policy !== 'string' || policy.trim() === '') {
    errors.push(`${field} must be a non-empty string (null disables the webview CSP entirely)`)
    return null
  }
  const map = new Map()
  for (const part of policy.split(';')) {
    const directive = part.trim()
    if (directive === '') {
      errors.push(`${field} has an empty directive segment (stray ';'?)`)
      continue
    }
    // CSP directive names are case-insensitive: normalize so that a
    // `SCRIPT-SRC`-style duplicate is caught instead of masquerading as a
    // distinct directive (browsers honor the first occurrence and silently
    // drop the rest).
    const [rawName, ...sources] = directive.split(/\s+/)
    const name = rawName.toLowerCase()
    if (sources.length === 0) errors.push(`${field}: directive ${name} has no sources`)
    if (map.has(name)) {
      errors.push(`${field}: duplicate directive ${name} (the second occurrence is silently ignored by CSP)`)
    }
    map.set(name, sources)
  }
  return map
}

function sortedSources(sources) {
  return [...sources].sort().join(' ')
}

function checkPolicyShape(map, expected, field, errors) {
  // Lock the directive SET as well as each directive's sources: a brand-new
  // directive (media-src/frame-src/child-src …) is the most natural form of
  // future loosening, so an unexpected name must fail the gate even when
  // both csp and devCsp gain it in lockstep. Object.hasOwn, not `in`:
  // prototype keys (constructor/toString/__proto__) would otherwise pass as
  // pinned directives.
  for (const name of map.keys()) {
    if (!Object.hasOwn(expected, name)) {
      errors.push(`${field}: unexpected directive ${name} is not part of the pinned policy (extend the pinning deliberately if intended)`)
    }
  }
  for (const [name, sources] of Object.entries(expected)) {
    const actual = map.get(name)
    if (!actual) {
      errors.push(`${field}: missing directive ${name}`)
      continue
    }
    if (sortedSources(actual) !== sortedSources(sources)) {
      errors.push(`${field}: ${name} must be exactly [${sortedSources(sources)}], got [${sortedSources(actual)}]`)
    }
  }
}

/// Full gate: returns every policy violation found in a parsed tauri.conf.json.
/// Never throws on malformed shapes — a non-object app.security surfaces as a
/// clean error line (the CLI promises no stack traces).
export function collectPolicyErrors(conf) {
  const errors = []
  const security = conf?.app?.security
  if (security === undefined) return ['tauri.conf.json: app.security is missing']
  // null 单独点名：typeof null === 'object' 会把 null 报成 "got object"（经典坑）
  if (security === null) return ['app.security must be an object, got null']
  if (typeof security !== 'object' || Array.isArray(security)) {
    return [`app.security must be an object, got ${Array.isArray(security) ? 'array' : typeof security}`]
  }
  if ('dangerousDisableAssetCspModification' in security) {
    errors.push(
      'app.security.dangerousDisableAssetCspModification must stay unset (an explicit false is rejected too — touching the knob at all requires a conscious decision): keep Tauri\u2019s serve-time CSP augmentation (observed: script-src gains one sha256 hash per bundled script asset; every other directive\u2019s source list is served unchanged, though the header is rebuilt from a map and its order is not guaranteed)',
    )
  }
  const prod = parseDirectives(security.csp, 'app.security.csp', errors)
  const dev = parseDirectives(security.devCsp, 'app.security.devCsp', errors)
  if (prod) checkPolicyShape(prod, PROD_POLICY, 'app.security.csp', errors)
  if (dev) checkPolicyShape(dev, devPolicy(), 'app.security.devCsp', errors)
  // devCsp must be a superset of csp, not a fork: identical directive set,
  // dev-only source additions are pinned by DEV_EXTRAS above.
  if (prod && dev) {
    for (const name of prod.keys()) {
      if (!dev.has(name)) errors.push(`app.security.devCsp drops directive ${name} present in csp (dev must be a superset)`)
    }
    for (const name of dev.keys()) {
      if (!prod.has(name)) errors.push(`app.security.devCsp adds directive ${name} absent from csp (dev must be a superset, not a fork)`)
    }
  }
  return errors
}

/// CLI entry: reads tauri.conf.json under `root`, prints a clean one-line
/// verdict (never a stack trace), returns { ok, errors }.
export function runGate(root, log = () => {}) {
  const confPath = join(root, 'src-tauri', 'tauri.conf.json')
  let text
  try {
    text = readFileSync(confPath, 'utf-8')
  } catch (error) {
    const message = `tauri.conf.json is missing or unreadable (${error.code ?? error.message})`
    log(message)
    return { ok: false, errors: [message] }
  }
  let conf
  try {
    conf = JSON.parse(text)
  } catch (error) {
    const message = `tauri.conf.json is not valid JSON: ${error.message}`
    log(message)
    return { ok: false, errors: [message] }
  }
  let errors
  try {
    errors = collectPolicyErrors(conf)
  } catch (error) {
    // 契约：任何畸形配置都出干净单行错误，绝不吐堆栈
    errors = [`collectPolicyErrors failed on malformed config: ${error.message}`]
  }
  for (const error of errors) log(error)
  if (errors.length === 0) {
    log(`OK（csp/devCsp 与钉死策略一致：${Object.keys(PROD_POLICY).length} 条指令逐项 exact，devCsp ⊇ csp）`)
  }
  return { ok: errors.length === 0, errors }
}

const isMainModule = process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href
if (isMainModule) {
  const result = runGate(fileURLToPath(new URL('..', import.meta.url)), (line) => console.log(`csp-policy: ${line}`))
  process.exitCode = result.ok ? 0 : 1
}
