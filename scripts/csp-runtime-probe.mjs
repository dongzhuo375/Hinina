// CSP runtime probe: verifies that a PACKAGED exe actually serves and enforces
// the configured CSP. The static gate (check-csp-policy.mjs) pins what the
// policy SHOULD be; this probe closes the exe-only gap — dev injects no CSP
// and e2e never enters the webview, so regressions like a newly-needed source
// category being blocked only surface in the packaged app, where the sole
// clue would otherwise be Rust-log CSP lines.
//
// Assertions (single expectation source = tauri.conf.json itself, no second
// policy copy):
//   1. served CSP header keeps every configured directive and every
//      configured source (superset compare, order-independent — tauri appends
//      script-src hashes and rebuilds the header from a map);
//   2. zero violations in a quiet session (proves IPC rides the fetch fast
//      path; the postMessage fallback is silent);
//   3. an external fetch IS blocked by connect-src (enforcement is live).
//
// Usage: node scripts/csp-runtime-probe.mjs [cdpPort=9223]
// The exe must be running with
//   WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<port>
// (connectOverCDP needs only the playwright library, no browser download).
// Not covered by vitest — needs a live exe; runs in the nightly
// integration.yml windows build leg.

import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from '@playwright/test'

const port = process.argv[2] ?? '9223'
const repoRoot = fileURLToPath(new URL('..', import.meta.url))
const CONNECT_TIMEOUT_MS = 60_000

function configuredCsp() {
  const conf = JSON.parse(readFileSync(join(repoRoot, 'src-tauri', 'tauri.conf.json'), 'utf-8'))
  const csp = conf?.app?.security?.csp
  if (typeof csp !== 'string') throw new Error('tauri.conf.json app.security.csp must be a non-empty string')
  const map = new Map()
  for (const part of csp.split(';')) {
    const directive = part.trim()
    if (!directive) continue
    const [name, ...sources] = directive.split(/\s+/)
    map.set(name.toLowerCase(), sources)
  }
  return map
}

async function connectWithRetry() {
  const deadline = Date.now() + CONNECT_TIMEOUT_MS
  for (;;) {
    try {
      return await chromium.connectOverCDP(`http://127.0.0.1:${port}`)
    } catch (error) {
      if (Date.now() > deadline) {
        throw new Error(`CDP port ${port} unreachable after ${CONNECT_TIMEOUT_MS / 1000}s — exe window not up? (${error.message})`)
      }
      await new Promise((resolve) => setTimeout(resolve, 2000))
    }
  }
}

const expected = configuredCsp()
const browser = await connectWithRetry()
try {
  const page = browser.contexts()[0]?.pages()[0]
  if (!page) {
    throw new Error('no page found over CDP — webview window not created?')
  }
  const failures = []

  // 1) served header ⊇ configured sources, per directive
  const header = await page.evaluate(async () => {
    const response = await fetch(window.location.href, { mode: 'same-origin' })
    return response.headers.get('content-security-policy') ?? ''
  })
  const served = new Map()
  for (const part of header.split(';')) {
    const directive = part.trim()
    if (!directive) continue
    const [name, ...sources] = directive.split(/\s+/)
    served.set(name.toLowerCase(), sources)
  }
  for (const [name, sources] of expected) {
    const got = served.get(name)
    if (!got) {
      failures.push(`served header lacks directive ${name}`)
      continue
    }
    const gotSet = new Set(got)
    for (const source of sources) {
      if (!gotSet.has(source)) failures.push(`served ${name} lost source ${source} (configured: ${sources.join(' ')})`)
    }
  }

  // 2) zero-violation baseline in a quiet session
  const baseline = await page.evaluate(async () => {
    window.__cspProbeViolations = []
    document.addEventListener(
      'securitypolicyviolation',
      (e) => window.__cspProbeViolations.push(`${e.effectiveDirective}<-${String(e.blockedURI).slice(0, 60)}`),
      true,
    )
    await new Promise((resolve) => setTimeout(resolve, 3000))
    return window.__cspProbeViolations
  })
  for (const violation of baseline) failures.push(`baseline violation (quiet session must be zero): ${violation}`)

  // 3) enforcement probe: external fetch must hit connect-src
  //    (CSP refuses before the network, so this fires even offline)
  const enforced = await page.evaluate(async () => {
    await fetch('https://example.com/', { mode: 'no-cors' }).catch(() => {})
    await new Promise((resolve) => setTimeout(resolve, 2000))
    return window.__cspProbeViolations.some((entry) => entry.startsWith('connect-src'))
  })
  if (!enforced) failures.push('external fetch was NOT blocked — CSP enforcement missing?!')

  if (failures.length > 0) {
    for (const failure of failures) console.error(`csp-probe: ${failure}`)
    process.exitCode = 1
  } else {
    console.log(`csp-probe: OK（${expected.size} 条指令服务态保持配置源集；静默会话零违规；外域 fetch 被 connect-src 拦截）`)
  }
} finally {
  await browser.close()
}
