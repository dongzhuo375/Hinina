import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'
import { collectPolicyErrors, DEV_EXTRAS, PROD_POLICY, runGate } from '../check-csp-policy.mjs'

// Gate for the webview Content Security Policy in src-tauri/tauri.conf.json.
// The policy-shape logic lives in scripts/check-csp-policy.mjs (run by
// `npm run check:csp-policy` and ci.yml); this suite pins the real config
// and proves with mutations that the gate actually catches regressions.
//
// The CSP is the resource-loading layer of the app's defense lines:
// DOMPurify sanitizes untrusted markdown content, the external-link
// interceptor + opener own navigation, CSP governs what the webview may
// load. It regresses silently — a `null` csp or a quietly loosened directive
// produces no error, only a wider attack surface.

const repoRoot = fileURLToPath(new URL('../..', import.meta.url))

type Security = {
  csp?: unknown
  devCsp?: unknown
  dangerousDisableAssetCspModification?: unknown
}

type TauriConf = { app?: { security?: Security } }

function readRealConf(): TauriConf {
  return JSON.parse(readFileSync(join(repoRoot, 'src-tauri', 'tauri.conf.json'), 'utf-8')) as TauriConf
}

function withSecurity(mutate: (security: Security) => void): TauriConf {
  const conf = readRealConf()
  const security = conf.app?.security
  if (!security) throw new Error('real conf has no app.security to mutate')
  mutate(security)
  return conf
}

/// Targeted mutation on the real policy string: the anchor must be present,
/// so a test can never silently keep passing against a changed policy the
/// way a hardcoded full-policy copy would.
function swap(policy: unknown, from: string, to: string): string {
  const text = typeof policy === 'string' ? policy : ''
  expect(text).toContain(from)
  return text.replace(from, to)
}

describe('webview CSP policy gate (real repository)', () => {
  it('the real tauri.conf.json satisfies the pinned policy', () => {
    expect(collectPolicyErrors(readRealConf())).toEqual([])
  })

  it('runGate passes on the real repository and prints the OK verdict', () => {
    const lines: string[] = []
    const result = runGate(repoRoot, (line) => lines.push(line))
    expect(result.ok).toBe(true)
    expect(lines.join('\n')).toContain('OK')
  })

  it('rejects csp: null (the pre-gate state: CSP disabled in production)', () => {
    const errors = collectPolicyErrors(withSecurity((s) => { s.csp = null }))
    expect(errors.some((e) => e.includes('app.security.csp must be a non-empty string'))).toBe(true)
  })

  it('rejects devCsp: null (dev would fall back unprotected or diverge)', () => {
    const errors = collectPolicyErrors(withSecurity((s) => { s.devCsp = null }))
    expect(errors.some((e) => e.includes('app.security.devCsp must be a non-empty string'))).toBe(true)
  })

  it("rejects 'unsafe-inline' sneaking into script-src", () => {
    const errors = collectPolicyErrors(
      withSecurity((s) => {
        s.csp = swap(s.csp, "script-src 'self'", "script-src 'self' 'unsafe-inline'")
      }),
    )
    expect(errors.some((e) => e.includes("app.security.csp: script-src must be exactly ['self']"))).toBe(true)
  })

  it("rejects 'unsafe-eval' sneaking into script-src", () => {
    const errors = collectPolicyErrors(
      withSecurity((s) => {
        s.devCsp = swap(s.devCsp, "script-src 'self'", "script-src 'self' 'unsafe-eval'")
      }),
    )
    expect(errors.some((e) => e.includes("app.security.devCsp: script-src must be exactly ['self']"))).toBe(true)
  })

  it('rejects img-src dropping http: (private-LAN OJ images would break)', () => {
    const errors = collectPolicyErrors(
      withSecurity((s) => {
        s.csp = swap(s.csp, "img-src 'self' http: https: data:", "img-src 'self' https: data:")
      }),
    )
    expect(errors.some((e) => e.includes('img-src'))).toBe(true)
  })

  it('rejects devCsp dropping the HMR websocket source', () => {
    const errors = collectPolicyErrors(withSecurity((s) => { s.devCsp = s.csp }))
    expect(errors.some((e) => e.includes('app.security.devCsp: connect-src'))).toBe(true)
  })

  it('rejects devCsp dropping blob: from worker-src (Monaco dev workers would break)', () => {
    const errors = collectPolicyErrors(
      withSecurity((s) => {
        s.devCsp = swap(s.devCsp, "worker-src 'self' blob:", "worker-src 'self'")
      }),
    )
    expect(errors.some((e) => e.includes('app.security.devCsp: worker-src'))).toBe(true)
  })

  it('rejects the production csp gaining dev-only sources', () => {
    const errors = collectPolicyErrors(
      withSecurity((s) => {
        s.csp = swap(s.csp, "worker-src 'self'", "worker-src 'self' blob:")
      }),
    )
    expect(errors.some((e) => e.includes("app.security.csp: worker-src must be exactly ['self']"))).toBe(true)
  })

  it('rejects dangerousDisableAssetCspModification being set', () => {
    const errors = collectPolicyErrors(withSecurity((s) => { s.dangerousDisableAssetCspModification = ['csp'] }))
    expect(errors.some((e) => e.includes('dangerousDisableAssetCspModification'))).toBe(true)
  })

  it('rejects a directive fork between csp and devCsp (extra directive on one side)', () => {
    const errors = collectPolicyErrors(
      withSecurity((s) => {
        s.devCsp = typeof s.devCsp === 'string' ? `${s.devCsp}; child-src 'self'` : s.devCsp
      }),
    )
    expect(errors.some((e) => e.includes('adds directive child-src'))).toBe(true)
  })

  it('rejects a brand-new directive added to both sides in lockstep (the set is pinned, not just the sources)', () => {
    const errors = collectPolicyErrors(
      withSecurity((s) => {
        const extra = '; frame-src *'
        s.csp = typeof s.csp === 'string' ? s.csp + extra : s.csp
        s.devCsp = typeof s.devCsp === 'string' ? s.devCsp + extra : s.devCsp
      }),
    )
    expect(errors.filter((e) => e.includes('unexpected directive frame-src')).length).toBe(2)
  })

  it('rejects prototype-chain directive names (constructor is not a pinned directive)', () => {
    // With `name in expected` this passed the gate as "pinned"; Object.hasOwn
    // closes it. Browsers ignore unknown directives, so this is a gate-integrity
    // fix rather than a served-policy hole.
    const errors = collectPolicyErrors(
      withSecurity((s) => {
        s.csp = typeof s.csp === 'string' ? `constructor *; ${s.csp}` : s.csp
      }),
    )
    expect(errors.some((e) => e.includes('unexpected directive constructor'))).toBe(true)
  })

  it('rejects case-camouflaged duplicates (CSP names are case-insensitive, first occurrence wins in browsers)', () => {
    const errors = collectPolicyErrors(
      withSecurity((s) => {
        s.csp = typeof s.csp === 'string' ? `SCRIPT-SRC *; ${s.csp}` : s.csp
      }),
    )
    expect(errors.some((e) => e.includes('duplicate directive script-src'))).toBe(true)
  })

  it('keeps DEV_EXTRAS keys aligned with PROD_POLICY (a typo key would be silently swallowed)', () => {
    for (const key of Object.keys(DEV_EXTRAS)) {
      expect(Object.hasOwn(PROD_POLICY, key)).toBe(true)
    }
  })

  it('flags duplicate directives (CSP silently ignores the second one)', () => {
    const errors = collectPolicyErrors(withSecurity((s) => { s.csp = "default-src 'self'; default-src 'none'; script-src 'self'" }))
    expect(errors.some((e) => e.includes('duplicate directive default-src'))).toBe(true)
  })

  it('flags empty directive segments from a stray semicolon', () => {
    const errors = collectPolicyErrors(withSecurity((s) => { s.csp = "default-src 'self';;" }))
    expect(errors.some((e) => e.includes('empty directive segment'))).toBe(true)
  })

  it('flags directives without sources', () => {
    const errors = collectPolicyErrors(withSecurity((s) => { s.csp = "default-src 'self'; script-src" }))
    expect(errors.some((e) => e.includes('script-src has no sources'))).toBe(true)
  })
})

describe('runGate (CLI entry)', () => {
  it('passes on the real repository', () => {
    expect(runGate(repoRoot).ok).toBe(true)
  })

  it('fails with a clean one-line error when tauri.conf.json is missing', () => {
    const lines: string[] = []
    const result = runGate(join(tmpdir(), 'hinina-csp-gate-missing'), (line) => lines.push(line))
    expect(result.ok).toBe(false)
    expect(lines.join('\n')).toContain('missing or unreadable')
    expect(lines.join('\n')).not.toMatch(/^\s+at /m)
  })

  it('fails with a clean one-line error on invalid JSON', () => {
    const root = mkdtempSync(join(tmpdir(), 'hinina-csp-gate-'))
    mkdirSync(join(root, 'src-tauri'), { recursive: true })
    writeFileSync(join(root, 'src-tauri', 'tauri.conf.json'), '{not json')
    const lines: string[] = []
    const result = runGate(root, (line) => lines.push(line))
    expect(result.ok).toBe(false)
    expect(lines.join('\n')).toContain('not valid JSON')
  })

  it('fails on a null csp fixture (exit path CI would take)', () => {
    const root = mkdtempSync(join(tmpdir(), 'hinina-csp-gate-'))
    mkdirSync(join(root, 'src-tauri'), { recursive: true })
    writeFileSync(join(root, 'src-tauri', 'tauri.conf.json'), JSON.stringify({ app: { security: { csp: null } } }))
    const lines: string[] = []
    const result = runGate(root, (line) => lines.push(line))
    expect(result.ok).toBe(false)
    expect(lines.join('\n')).toContain('app.security.csp must be a non-empty string')
  })
})
