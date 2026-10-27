import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'
import {
  checkTag,
  checkVersionConsistency,
  extractNotes,
  findChangelogSection,
  runCli,
} from '../check-release-versions.mjs'

const repoRoot = fileURLToPath(new URL('../..', import.meta.url))

// Never hardcode the repo version: these tests must keep passing across version
// bumps as long as the five files (and the CHANGELOG section) stay in sync —
// that continuous invariant is exactly what they assert.
const repoVersion = (JSON.parse(readFileSync(join(repoRoot, 'package.json'), 'utf-8')) as { version: string }).version

const APP_VERSION = '1.2.3'

function makeFixtureRoot(files: Record<string, string | undefined>): string {
  const root = mkdtempSync(join(tmpdir(), 'hinina-release-check-'))
  for (const [relativePath, content] of Object.entries(files)) {
    if (content === undefined) continue
    const target = join(root, relativePath)
    mkdirSync(dirname(target), { recursive: true })
    writeFileSync(target, content)
  }
  return root
}

function fixtureFiles(overrides: Record<string, string | undefined> = {}): Record<string, string | undefined> {
  return {
    'package.json': JSON.stringify({ version: APP_VERSION }),
    'package-lock.json': JSON.stringify({
      version: APP_VERSION,
      packages: { '': { version: APP_VERSION } },
    }),
    'src-tauri/Cargo.toml': '[package]\nname = "hinina"\nversion = "1.2.3"\n',
    'src-tauri/Cargo.lock':
      '[[package]]\nname = "serde"\nversion = "9.9.9"\n\n[[package]]\nname = "hinina"\nversion = "1.2.3"\n',
    'src-tauri/tauri.conf.json': JSON.stringify({ version: APP_VERSION }),
    'CHANGELOG.md':
      '# Changelog\n\n## [Unreleased]\n\n## [1.2.3] - 2026-10-26\n\n### Added\n- feature one\n- feature two\n',
    ...overrides,
  }
}

function captureGate(argv: string[], root: string) {
  const out: string[] = []
  const code = runCli(argv, root, (line: string) => out.push(line), (line: string) => out.push(line))
  return { code, out: out.join('\n') }
}

describe('checkVersionConsistency', () => {
  it('passes on the real repository (all five files at the same version)', () => {
    const result = checkVersionConsistency(repoRoot)
    expect(result.ok).toBe(true)
    expect(result.version).toBe(repoVersion)
  })

  it('passes when every declared version agrees', () => {
    const result = checkVersionConsistency(makeFixtureRoot(fixtureFiles()))
    expect(result.ok).toBe(true)
    expect(result.version).toBe(APP_VERSION)
  })

  it('fails when tauri.conf.json drifts from package.json', () => {
    const root = makeFixtureRoot(
      fixtureFiles({ 'src-tauri/tauri.conf.json': JSON.stringify({ version: '9.9.9' }) }),
    )
    const result = checkVersionConsistency(root)
    expect(result.ok).toBe(false)
    expect(result.errors.join('\n')).toContain('tauri.conf.json')
  })

  it('fails when the package-lock root version drifts', () => {
    const root = makeFixtureRoot(
      fixtureFiles({
        'package-lock.json': JSON.stringify({
          version: '0.0.1',
          packages: { '': { version: APP_VERSION } },
        }),
      }),
    )
    const result = checkVersionConsistency(root)
    expect(result.ok).toBe(false)
    expect(result.errors.join('\n')).toContain('package-lock.json (root version)')
  })

  it('fails when the Cargo.lock hinina entry drifts', () => {
    const root = makeFixtureRoot(
      fixtureFiles({
        'src-tauri/Cargo.lock':
          '[[package]]\nname = "hinina"\nversion = "0.0.1"\n',
      }),
    )
    const result = checkVersionConsistency(root)
    expect(result.ok).toBe(false)
    expect(result.errors.join('\n')).toContain('Cargo.lock')
  })

  it('reports a missing file as a clean one-line error, not a stack trace', () => {
    const root = makeFixtureRoot({})
    const { code, out } = captureGate([], root)
    expect(code).toBe(1)
    expect(out).toContain('package.json is missing or unreadable')
    expect(out).not.toMatch(/^\s+at /m)
  })

  it('reports invalid JSON as a clean one-line error', () => {
    const root = makeFixtureRoot(fixtureFiles({ 'package.json': '{not json' }))
    const { code, out } = captureGate([], root)
    expect(code).toBe(1)
    expect(out).toContain('package.json is not valid JSON')
    expect(out).not.toMatch(/^\s+at /m)
  })
})

describe('checkTag', () => {
  it('accepts a semver tag matching the app version', () => {
    expect(checkTag('v1.2.3', '1.2.3')).toEqual({ ok: true })
  })

  it('rejects a tag without the v prefix', () => {
    expect(checkTag('1.2.3', '1.2.3').ok).toBe(false)
  })

  it('rejects partial or malformed versions', () => {
    expect(checkTag('v1.2', '1.2').ok).toBe(false)
    expect(checkTag('v01.2.3', '1.2.3').ok).toBe(false)
    expect(checkTag('v1.2.3-rc1', '1.2.3').ok).toBe(false)
    expect(checkTag('vx.y.z', 'x.y.z').ok).toBe(false)
  })

  it('rejects a well-formed tag that does not match the app version', () => {
    expect(checkTag('v1.2.4', '1.2.3').ok).toBe(false)
  })

  it('explains that pre-release tag formats are unsupported', () => {
    const result = checkTag('v0.1.0-rc1', '0.1.0')
    expect(result.ok).toBe(false)
    expect(result.error).toContain('not supported')
  })
})

describe('findChangelogSection / extractNotes', () => {
  it('finds a Keep a Changelog bracketed section and returns its body', () => {
    const root = makeFixtureRoot(fixtureFiles())
    const section = findChangelogSection(root, APP_VERSION)
    expect(section.found).toBe(true)
    expect(section.body).toContain('feature one')
  })

  it('finds an unbracketed section header', () => {
    const root = makeFixtureRoot(
      fixtureFiles({ 'CHANGELOG.md': '# Changelog\n\n## 1.2.3\n\nbody text\n' }),
    )
    expect(findChangelogSection(root, APP_VERSION).found).toBe(true)
  })

  it('does not match a longer version that shares a prefix', () => {
    const root = makeFixtureRoot(
      fixtureFiles({ 'CHANGELOG.md': '# Changelog\n\n## [1.2.34] - 2026-10-26\n' }),
    )
    expect(findChangelogSection(root, '1.2.3').found).toBe(false)
  })

  it('reports missing file and missing section', () => {
    const root = makeFixtureRoot(fixtureFiles({ 'CHANGELOG.md': undefined }))
    expect(findChangelogSection(root, APP_VERSION).found).toBe(false)
    expect(findChangelogSection(makeFixtureRoot(fixtureFiles()), '4.5.6').found).toBe(false)
  })

  it('rejects duplicate sections for the same version as ambiguous', () => {
    const root = makeFixtureRoot(
      fixtureFiles({
        'CHANGELOG.md':
          '# Changelog\n\n## [1.2.3] - 2026-10-25\n\nstale body\n\n## [1.2.3] - 2026-10-26\n\nfresh body\n',
      }),
    )
    const section = findChangelogSection(root, APP_VERSION)
    expect(section.found).toBe(false)
    expect(section.reason).toContain('2 sections')
    expect(captureGate(['--changelog'], root).code).toBe(1)
    expect(captureGate(['--extract-notes'], root).code).toBe(1)
  })

  it('rejects an undated/placeholder date suffix so releases cannot ship with TBD', () => {
    const root = makeFixtureRoot(
      fixtureFiles({ 'CHANGELOG.md': '# Changelog\n\n## [1.2.3] - TBD\n\nbody\n' }),
    )
    const section = findChangelogSection(root, APP_VERSION)
    expect(section.found).toBe(false)
    expect(section.reason).toContain('invalid date')
    expect(captureGate(['--tag', 'v1.2.3', '--changelog'], root).code).toBe(1)
  })

  it('rejects a malformed date and accepts a header with no date suffix', () => {
    const bad = makeFixtureRoot(
      fixtureFiles({ 'CHANGELOG.md': '# Changelog\n\n## [1.2.3] - 2026-13-45\n\nbody\n' }),
    )
    expect(findChangelogSection(bad, APP_VERSION).found).toBe(false)
    const undated = makeFixtureRoot(
      fixtureFiles({ 'CHANGELOG.md': '# Changelog\n\n## [1.2.3]\n\nbody\n' }),
    )
    expect(findChangelogSection(undated, APP_VERSION).found).toBe(true)
  })

  it('rejects dates that parse but do not exist (2026-02-31 rolls over to 03-03)', () => {
    // Date.parse accepts it and rolls forward; only a round-trip check catches it.
    expect(Number.isNaN(Date.parse('2026-02-31T00:00:00Z'))).toBe(false)
    const root = makeFixtureRoot(
      fixtureFiles({ 'CHANGELOG.md': '# Changelog\n\n## [1.2.3] - 2026-02-31\n\nbody\n' }),
    )
    const section = findChangelogSection(root, APP_VERSION)
    expect(section.found).toBe(false)
    expect(section.reason).toContain('invalid date')
    expect(captureGate(['--tag', 'v1.2.3', '--changelog'], root).code).toBe(1)
  })

  it('accepts leap day of a leap year and rejects it in a non-leap year', () => {
    const leap = makeFixtureRoot(
      fixtureFiles({ 'CHANGELOG.md': '# Changelog\n\n## [1.2.3] - 2028-02-29\n\nbody\n' }),
    )
    expect(findChangelogSection(leap, APP_VERSION).found).toBe(true)
    const nonLeap = makeFixtureRoot(
      fixtureFiles({ 'CHANGELOG.md': '# Changelog\n\n## [1.2.3] - 2026-02-29\n\nbody\n' }),
    )
    expect(findChangelogSection(nonLeap, APP_VERSION).found).toBe(false)
  })

  it('stops notes before the trailing link-reference block', () => {
    const root = makeFixtureRoot(
      fixtureFiles({
        'CHANGELOG.md':
          '# Changelog\n\n## [1.2.3] - 2026-10-26\n\n### Added\n- one\n\n[1.2.3]: https://example.test/compare/v1.2.2...v1.2.3\n[1.2.2]: https://example.test/compare/v1.2.1...v1.2.2\n',
      }),
    )
    const notes = extractNotes(root, APP_VERSION)
    expect(notes).toBe('### Added\n- one')
    expect(notes).not.toContain('example.test')
  })

  it('extractNotes returns the section body without the header', () => {
    const root = makeFixtureRoot(fixtureFiles())
    expect(extractNotes(root, APP_VERSION)).toBe('### Added\n- feature one\n- feature two')
  })
})

describe('runCli', () => {
  it('passes the full gate on the real repository for the current version', () => {
    const { code, out } = captureGate(['--tag', `v${repoVersion}`], repoRoot)
    expect(code).toBe(0)
    expect(out).toContain('release version gate:')
    expect(out).toContain('matches app version')
  })

  it('passes the full gate including the CHANGELOG section on the real repository', () => {
    const { code, out } = captureGate(['--tag', `v${repoVersion}`, '--changelog'], repoRoot)
    expect(code).toBe(0)
    expect(out).toContain('CHANGELOG.md')
  })

  it('fails with exit code 1 when versions disagree', () => {
    const root = makeFixtureRoot(
      fixtureFiles({ 'src-tauri/tauri.conf.json': JSON.stringify({ version: '9.9.9' }) }),
    )
    const { code, out } = captureGate([], root)
    expect(code).toBe(1)
    expect(out).toContain('tauri.conf.json')
  })

  it('enforces tag format and CHANGELOG section when requested', () => {
    const root = makeFixtureRoot(fixtureFiles())
    expect(captureGate(['--tag', 'v1.2.3', '--changelog'], root).code).toBe(0)
    expect(captureGate(['--tag', 'v1.2.4', '--changelog'], root).code).toBe(1)
    expect(captureGate(['--tag', 'not-a-tag'], root).code).toBe(1)
    expect(captureGate(['--changelog'], makeFixtureRoot(fixtureFiles({ 'CHANGELOG.md': '# Changelog\n' }))).code).toBe(1)
  })

  it('extract-notes prints only the notes body on stdout', () => {
    const root = makeFixtureRoot(fixtureFiles())
    const { code, out } = captureGate(['--extract-notes'], root)
    expect(code).toBe(0)
    expect(out).toBe('### Added\n- feature one\n- feature two')
  })

  it('extract-notes fails when the section is missing', () => {
    const { code } = captureGate(['--extract-notes'], makeFixtureRoot(fixtureFiles({ 'CHANGELOG.md': '# Changelog\n' })))
    expect(code).toBe(1)
  })

  it('print-version prints just the version', () => {
    const { code, out } = captureGate(['--print-version'], makeFixtureRoot(fixtureFiles()))
    expect(code).toBe(0)
    expect(out).toBe(APP_VERSION)
  })

  it('print-version reads only package.json and works when other version files are absent', () => {
    const root = makeFixtureRoot({ 'package.json': JSON.stringify({ version: APP_VERSION }) })
    const { code, out } = captureGate(['--print-version'], root)
    expect(code).toBe(0)
    expect(out).toBe(APP_VERSION)
  })

  it('rejects unknown arguments', () => {
    const { code, out } = captureGate(['--bogus'], makeFixtureRoot(fixtureFiles()))
    expect(code).toBe(1)
    expect(out).toContain('unknown argument')
  })

  it('rejects --tag combined with --print-version or --extract-notes instead of ignoring it', () => {
    const root = makeFixtureRoot(fixtureFiles())
    const withPrint = captureGate(['--print-version', '--tag', 'v1.2.3'], root)
    expect(withPrint.code).toBe(1)
    expect(withPrint.out).toContain('cannot be combined')
    const withNotes = captureGate(['--extract-notes', '--tag', 'v9.9.9'], root)
    expect(withNotes.code).toBe(1)
    expect(withNotes.out).toContain('cannot be combined')
  })
})
