import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'

// Gate for the webview Content Security Policy in src-tauri/tauri.conf.json.
//
// The CSP is the resource-loading layer of the app's three defense lines:
// DOMPurify sanitizes untrusted markdown content, the external-link
// interceptor + opener own navigation, CSP governs what the webview may
// load. It regresses silently — a `null` csp or a quietly loosened directive
// produces no error, only a wider attack surface — so the expected policy is
// pinned here. Changing the policy is fine; do it consciously by updating
// the expected sets below together with the rationale in doc/Architecture.md.

const repoRoot = fileURLToPath(new URL('../..', import.meta.url))

type Security = {
  csp?: unknown
  devCsp?: unknown
  dangerousDisableAssetCspModification?: unknown
}

type TauriConf = { app?: { security?: Security } }
type DirectiveMap = Map<string, string[]>

/// Sources each directive must carry in the production policy (exact set:
/// removals weaken, additions drift — both must touch this gate consciously).
const PROD_POLICY: Record<string, string[]> = {
  'default-src': ["'self'"],
  // No unsafe-eval / unsafe-inline: the app ships no inline scripts and no
  // runtime template compilation; plugins execute in the Rust-side QuickJS.
  'script-src': ["'self'"],
  // 'unsafe-inline' is unavoidable: Vue :style bindings, DOMPurify-preserved
  // presentational inline styles in markdown output, Monaco theme injection.
  'style-src': ["'self'", "'unsafe-inline'"],
  // Problem statements / announcements may embed arbitrary absolute image
  // URLs, and private-LAN OJ deployments rewrite relative images to plain
  // http absolutes; data: URIs survive DOMPurify for <img> by default.
  'img-src': ["'self'", 'http:', 'https:', 'data:'],
  'font-src': ["'self'"],
  // Tauri v2 IPC origins (Windows WebView2 uses http://ipc.localhost).
  'connect-src': ["'self'", 'ipc:', 'http://ipc.localhost'],
  'worker-src': ["'self'"],
  'object-src': ["'none'"],
  'base-uri': ["'self'"],
  'form-action': ["'self'"],
}

/// Dev-only extras on top of the production policy: the Vite HMR websocket
// and Monaco's dev-time blob worker wrappers. Anything else diverging
// between devCsp and csp is drift, not a tooling need.
const DEV_EXTRAS: Record<string, string[]> = {
  'connect-src': ['ws://localhost:1421'],
  'worker-src': ['blob:'],
}

function devPolicy(): Record<string, string[]> {
  const expected: Record<string, string[]> = {}
  for (const [name, sources] of Object.entries(PROD_POLICY)) {
    expected[name] = [...sources, ...(DEV_EXTRAS[name] ?? [])]
  }
  return expected
}

function parseDirectives(policy: unknown, field: string, errors: string[]): DirectiveMap | null {
  if (typeof policy !== 'string' || policy.trim() === '') {
    errors.push(`${field} must be a non-empty string (null disables the webview CSP entirely)`)
    return null
  }
  const map: DirectiveMap = new Map()
  for (const part of policy.split(';')) {
    const directive = part.trim()
    if (directive === '') {
      errors.push(`${field} has an empty directive segment (stray ';'?)`)
      continue
    }
    const [name, ...sources] = directive.split(/\s+/)
    if (sources.length === 0) errors.push(`${field}: directive ${name} has no sources`)
    if (map.has(name)) {
      errors.push(`${field}: duplicate directive ${name} (the second occurrence is silently ignored by CSP)`)
    }
    map.set(name, sources)
  }
  return map
}

function sortedSources(sources: string[]): string {
  return [...sources].sort().join(' ')
}

function checkPolicyShape(map: DirectiveMap, expected: Record<string, string[]>, field: string, errors: string[]): void {
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
export function collectPolicyErrors(conf: unknown): string[] {
  const errors: string[] = []
  const security = (conf as TauriConf | null)?.app?.security
  if (!security) return ['tauri.conf.json: app.security is missing']
  if ('dangerousDisableAssetCspModification' in security) {
    errors.push(
      'app.security.dangerousDisableAssetCspModification must stay unset: Tauri injects nonces/hashes for its IPC bootstrap scripts at compile time',
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

describe('webview CSP policy gate (real repository)', () => {
  it('the real tauri.conf.json satisfies the pinned policy', () => {
    expect(collectPolicyErrors(readRealConf())).toEqual([])
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
        s.csp = "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' http: https: data:; font-src 'self'; connect-src 'self' ipc: http://ipc.localhost; worker-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self'"
      }),
    )
    expect(errors.some((e) => e.includes("app.security.csp: script-src must be exactly ['self']"))).toBe(true)
  })

  it("rejects 'unsafe-eval' sneaking into script-src", () => {
    const errors = collectPolicyErrors(
      withSecurity((s) => {
        s.devCsp = "default-src 'self'; script-src 'self' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' http: https: data:; font-src 'self'; connect-src 'self' ipc: http://ipc.localhost ws://localhost:1421; worker-src 'self' blob:; object-src 'none'; base-uri 'self'; form-action 'self'"
      }),
    )
    expect(errors.some((e) => e.includes("app.security.devCsp: script-src must be exactly ['self']"))).toBe(true)
  })

  it('rejects img-src dropping http: (private-LAN OJ images would break)', () => {
    const errors = collectPolicyErrors(
      withSecurity((s) => {
        s.csp = s.csp === null ? null : String(s.csp).replace('http: https: data:', 'https: data:')
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
        s.devCsp = typeof s.devCsp === 'string' ? s.devCsp.replace(' blob:', '') : s.devCsp
      }),
    )
    expect(errors.some((e) => e.includes('app.security.devCsp: worker-src'))).toBe(true)
  })

  it('rejects the production csp gaining dev-only sources', () => {
    const errors = collectPolicyErrors(
      withSecurity((s) => {
        s.csp = typeof s.csp === 'string' ? `${s.csp.replace("worker-src 'self'", "worker-src 'self' blob:")}` : s.csp
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
        s.devCsp = typeof s.devCsp === 'string' ? `${s.devCsp}; media-src 'self'` : s.devCsp
      }),
    )
    expect(errors.some((e) => e.includes('adds directive media-src'))).toBe(true)
  })

  it('flags duplicate directives (CSP silently ignores the second one)', () => {
    const errors = collectPolicyErrors(
      withSecurity((s) => { s.csp = "default-src 'self'; default-src 'none'; script-src 'self'" }),
    )
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
