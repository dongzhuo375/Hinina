// CSP runtime probe: verifies that a PACKAGED exe actually serves and enforces
// the configured CSP. The static gate (check-csp-policy.mjs) pins what the
// policy SHOULD be; this probe closes the exe-only gap — dev injects no CSP
// and e2e never enters the webview, so regressions like a newly-needed source
// category being blocked only surface in the packaged app, where the sole
// clue would otherwise be Rust-log CSP lines.
//
// Assertions (single expectation source = tauri.conf.json itself, no second
// policy copy):
//   1. served CSP header matches every configured directive source-for-source
//      (order-independent; the ONLY tolerated addition is sha256-* hashes on
//      script-src — the one augmentation tauri 2.12.5 empirically performs.
//      Anything else the header gains, including loosening sources like
//      'unsafe-eval', fails the probe);
//   2. an EXPLICIT invoke inside the zero-violation window succeeds and
//      produces zero violations (startup IPC may complete before the probe
//      attaches, so a quiet wait alone proves nothing; success + zero
//      violations together prove IPC rides the fetch fast path — the
//      postMessage fallback is silent);
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
const CONNECT_TIMEOUT_MS = 180_000

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

/// CDP 可连 ≠ 页面就绪：窗口刚创建时 pages() 只有 about:blank（CI 实证竞态
/// ——探针报错比应用首行启动日志早 4ms）。轮询直到真正的应用页（打包产物
/// 恒为 tauri.localhost）出现；超时打印全部 page URL 便于归因。
async function waitForAppPage(browser) {
  const deadline = Date.now() + CONNECT_TIMEOUT_MS
  for (;;) {
    const pages = browser.contexts().flatMap((context) => context.pages())
    const appPage = pages.find((page) => {
      try {
        return new URL(page.url()).hostname === 'tauri.localhost'
      } catch {
        return false
      }
    })
    if (appPage) return appPage
    if (Date.now() > deadline) {
      const urls = pages.map((page) => page.url()).join(', ')
      throw new Error(`app page (tauri.localhost) not found within ${CONNECT_TIMEOUT_MS / 1000}s — pages: [${urls}]`)
    }
    await new Promise((resolve) => setTimeout(resolve, 500))
  }
}

const expected = configuredCsp()
const browser = await connectWithRetry()
try {
  const page = await waitForAppPage(browser)
  // DOM 就绪再取头（catch 不致命：header fetch 自带重试兜底）
  await page.waitForLoadState('domcontentloaded', { timeout: 30_000 }).catch(() => {})
  const failures = []

  // 1) served header matches configured sources per directive; only sha256-*
  //    additions on script-src are tolerated (see header comment)
  const header = await page.evaluate(async () => {
    // 页面刚可寻址时偶发 Failed to fetch（CI 实证）——有界小重试 ×2：任何
    // 错误都重试（瞬时/永久不区分），重试耗尽后原样抛出转红（永久错误只多
    // 耗 ~1s，不影响判读）
    let response
    for (let attempt = 0; ; attempt++) {
      try {
        response = await fetch(window.location.href, { mode: 'same-origin' })
        break
      } catch (error) {
        if (attempt >= 2) throw error
        await new Promise((resolve) => setTimeout(resolve, 500))
      }
    }
    return response.headers.get('content-security-policy') ?? ''
  })
  const served = new Map()
  for (const part of header.split(';')) {
    const directive = part.trim()
    if (!directive) continue
    const [name, ...sources] = directive.split(/\s+/)
    served.set(name.toLowerCase(), sources)
  }
  // 唯一容忍的增补：script-src 上的 sha256-* hash（tauri 2.12.5 实证的唯一
  // 增补形态）。其余任何多出来的源（含上游追加以放宽为方向的 'unsafe-eval'
  // 类）一律红——superset 检查会静默漏过这类放宽。
  // 反向盲区同样关闭：服务态**新增整条指令**（配置里没有的指令名）也红——
  // 2.12.5 的 set_csp 机制上不会这么做，此断言是对未来上游行为的观测兜底。
  for (const name of served.keys()) {
    if (!expected.has(name)) failures.push(`served header has a directive not in the config: ${name}`)
  }
  const allowedAddition = /^'sha256-[A-Za-z0-9+/=]+'$/
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
    for (const extra of got) {
      if (sources.includes(extra)) continue
      if (name === 'script-src' && allowedAddition.test(extra)) continue
      failures.push(`served ${name} gained unexpected source ${extra} (only script-src sha256-* hashes may be appended)`)
    }
  }

  // 2) explicit IPC inside the window + zero violations. Startup IPC may
  //    finish before the probe attaches, so a quiet wait alone would be a
  //    vacuous assertion; the explicit invoke makes it deterministic.
  const baseline = await page.evaluate(async () => {
    window.__cspProbeViolations = []
    document.addEventListener(
      'securitypolicyviolation',
      (e) => window.__cspProbeViolations.push(`${e.effectiveDirective}<-${String(e.blockedURI).slice(0, 60)}`),
      true,
    )
    let ipc
    try {
      ipc = JSON.stringify(await window.__TAURI_INTERNALS__.invoke('launch_flags'))
    } catch (error) {
      ipc = `INVOKE_FAILED: ${String(error?.message ?? error).slice(0, 120)}`
    }
    await new Promise((resolve) => setTimeout(resolve, 2000))
    return { ipc, violations: window.__cspProbeViolations }
  })
  if (typeof baseline.ipc !== 'string' || baseline.ipc.startsWith('INVOKE_FAILED')) {
    failures.push(`explicit IPC inside the baseline window failed: ${baseline.ipc}`)
  }
  for (const violation of baseline.violations) {
    failures.push(`baseline violation (explicit IPC ran; must be zero): ${violation}`)
  }

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
    console.log(`csp-probe: OK（${expected.size} 条指令服务态逐源一致（唯一增补=script-src sha256）；窗口内显式 IPC 成功且零违规；外域 fetch 被 connect-src 拦截）`)
  }
} finally {
  await browser.close()
}
