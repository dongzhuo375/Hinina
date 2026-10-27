// Release version gate: single source of truth for release version checks.
// Verifies that every version-bearing file agrees, and optionally that a tag
// matches the app version and that CHANGELOG.md has a section for it.
// Used locally (`npm run release:check`) and by release.yml (validate runs the
// gate; draft-release extracts the CHANGELOG section as release notes).
//
// Usage:
//   node scripts/check-release-versions.mjs [--tag vX.Y.Z] [--changelog] [--extract-notes] [--print-version]
//
// `--print-version` only reads package.json (no consistency check); every
// caller must have run the full gate first — release.yml re-runs the gate in
// the same job immediately before using it.

import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { pathToFileURL, fileURLToPath } from 'node:url'

// All file access failures surface as clean one-line errors through runCli's
// catch, never as raw stack traces.
function readJsonFile(filePath, label) {
  let text
  try {
    text = readFileSync(filePath, 'utf-8')
  } catch (error) {
    throw new Error(`${label} is missing or unreadable (${error.code ?? error.message})`)
  }
  try {
    return JSON.parse(text)
  } catch (error) {
    throw new Error(`${label} is not valid JSON: ${error.message}`)
  }
}

function readTextFile(filePath, label) {
  try {
    return readFileSync(filePath, 'utf-8')
  } catch (error) {
    throw new Error(`${label} is missing or unreadable (${error.code ?? error.message})`)
  }
}

// package-lock.json carries the version twice (root and packages[""]);
// Cargo.lock carries it inside the hinina [[package]] entry. All must agree
// with package.json for a release.
export function readDeclaredVersions(root) {
  const pkg = readJsonFile(join(root, 'package.json'), 'package.json')
  const lock = readJsonFile(join(root, 'package-lock.json'), 'package-lock.json')
  const tauri = readJsonFile(join(root, 'src-tauri', 'tauri.conf.json'), 'src-tauri/tauri.conf.json')
  return {
    'package.json': pkg.version,
    'package-lock.json (root version)': lock.version,
    'package-lock.json (packages[""] version)': lock.packages?.['']?.version,
    'src-tauri/Cargo.toml ([package] version)': readCargoTomlVersion(join(root, 'src-tauri', 'Cargo.toml'), 'src-tauri/Cargo.toml'),
    'src-tauri/Cargo.lock (hinina version)': readCargoLockVersion(join(root, 'src-tauri', 'Cargo.lock'), 'src-tauri/Cargo.lock'),
    'src-tauri/tauri.conf.json (version)': tauri.version,
  }
}

export function checkVersionConsistency(root) {
  const versions = readDeclaredVersions(root)
  const version = versions['package.json']
  const errors = []
  if (typeof version !== 'string' || version.length === 0) {
    errors.push('package.json has no version field')
  } else {
    for (const [location, declared] of Object.entries(versions)) {
      if (location === 'package.json') continue
      if (declared !== version) {
        errors.push(`${location} is ${declared ?? 'missing'}, expected ${version}`)
      }
    }
  }
  return { ok: errors.length === 0, version, versions, errors }
}

const TAG_PATTERN = /^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/

export function checkTag(tag, appVersion) {
  if (!TAG_PATTERN.test(tag)) {
    return {
      ok: false,
      error: `tag "${tag}" is not v<major>.<minor>.<patch> (pre-release and other tag formats are not supported by the release workflow)`,
    }
  }
  if (tag.slice(1) !== appVersion) {
    return { ok: false, error: `tag ${tag} does not match app version ${appVersion}` }
  }
  return { ok: true }
}

// Accepts both `## [0.1.0] - 2026-10-26` (Keep a Changelog) and `## 0.1.0`.
// Duplicate sections for the same version are rejected as ambiguous.
// When a header carries a date suffix it must be a real YYYY-MM-DD date, so a
// release cannot ship with the `- TBD` placeholder still in place.
export function findChangelogSection(root, version) {
  let text
  try {
    text = readFileSync(join(root, 'CHANGELOG.md'), 'utf-8')
  } catch {
    return { found: false, reason: 'CHANGELOG.md not found' }
  }
  const headerPattern = new RegExp(`^##\\s+\\[?v?${escapeRegExp(version)}\\]?(\\s|$)`)
  const lines = text.split(/\r?\n/)
  const starts = []
  for (let i = 0; i < lines.length; i += 1) {
    if (headerPattern.test(lines[i])) starts.push(i)
  }
  if (starts.length === 0) {
    return { found: false, reason: `CHANGELOG.md has no section for ${version}` }
  }
  if (starts.length > 1) {
    return { found: false, reason: `CHANGELOG.md has ${starts.length} sections for ${version}; keep exactly one` }
  }
  const start = starts[0]
  const header = lines[start]
  const dateSuffix = header.match(/^##\s+\S+\s+-\s+(.+?)\s*$/)
  if (dateSuffix) {
    const date = dateSuffix[1]
    // Date.parse rolls impossible dates forward (2026-02-31 → 2026-03-03), so a
    // pattern match alone is not enough: round-trip through UTC and require the
    // ISO prefix to equal the input.
    const parsed = new Date(`${/^\d{4}-\d{2}-\d{2}$/.test(date) ? date : 'invalid'}T00:00:00Z`)
    const isRealDate = !Number.isNaN(parsed.getTime()) && parsed.toISOString().startsWith(date)
    if (!isRealDate) {
      return { found: false, reason: `CHANGELOG.md section header for ${version} has an invalid date "${date}" (expected a real YYYY-MM-DD date)` }
    }
  }
  let end = lines.length
  for (let i = start + 1; i < lines.length; i += 1) {
    // A new section header, or the trailing link-reference block that Keep a
    // Changelog puts at the end of the file ([1.0.0]: https://...) — that block
    // belongs to the document, not to this version's notes.
    if (/^##\s/.test(lines[i]) || /^\[[^\]]+\]:\s/.test(lines[i])) {
      end = i
      break
    }
  }
  return { found: true, header, body: lines.slice(start + 1, end).join('\n').trim() }
}

export function extractNotes(root, version) {
  const section = findChangelogSection(root, version)
  return section.found ? section.body : null
}

function readCargoTomlVersion(filePath, label) {
  let inPackageSection = false
  for (const line of readTextFile(filePath, label).split(/\r?\n/)) {
    const heading = line.match(/^\[(.+?)\]/)
    if (heading) {
      inPackageSection = heading[1] === 'package'
      continue
    }
    if (!inPackageSection) continue
    const version = line.match(/^version\s*=\s*"([^"]+)"/)
    if (version) return version[1]
  }
  return undefined
}

function readCargoLockVersion(filePath, label) {
  let currentName = null
  for (const line of readTextFile(filePath, label).split(/\r?\n/)) {
    if (line.startsWith('[[')) {
      currentName = null
      continue
    }
    const name = line.match(/^name\s*=\s*"([^"]+)"/)
    if (name) {
      currentName = name[1]
      continue
    }
    if (currentName === 'hinina') {
      const version = line.match(/^version\s*=\s*"([^"]+)"/)
      if (version) return version[1]
    }
  }
  return undefined
}

function escapeRegExp(text) {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

// Exit codes: 0 = gate passed (or notes/version printed), 1 = gate failed or
// bad usage. --extract-notes prints ONLY the notes body on stdout (diagnostics
// go to stderr) so its output can be redirected into a release notes file.
export function runCli(argv, root, log = console.log, errorLog = console.error) {
  try {
    return runGate(argv, root, log, errorLog)
  } catch (error) {
    errorLog(`error: ${error.message}`)
    return 1
  }
}

function runGate(argv, root, log, errorLog) {
  const flags = { changelog: false, extractNotes: false, printVersion: false }
  let tag = null
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i]
    if (arg === '--changelog') {
      flags.changelog = true
    } else if (arg === '--extract-notes') {
      flags.extractNotes = true
    } else if (arg === '--print-version') {
      flags.printVersion = true
    } else if (arg === '--tag' || arg.startsWith('--tag=')) {
      tag = arg.startsWith('--tag=') ? arg.slice('--tag='.length) : argv[(i += 1)]
      if (tag === undefined) {
        errorLog('error: --tag requires a value')
        return 1
      }
    } else {
      errorLog(`error: unknown argument "${arg}"`)
      errorLog('usage: node scripts/check-release-versions.mjs [--tag vX.Y.Z] [--changelog] [--extract-notes] [--print-version]')
      return 1
    }
  }

  // print-version reads only package.json and extract-notes only needs the app
  // version; both would silently ignore a --tag, which would hide a mistyped or
  // mismatched tag from the caller. Reject the combination instead.
  if (tag !== null && (flags.printVersion || flags.extractNotes)) {
    errorLog('error: --tag cannot be combined with --print-version or --extract-notes')
    return 1
  }

  // print-version is a pure readout of the app version: it must not touch any
  // of the other version files, so it runs before the consistency check.
  if (flags.printVersion) {
    const version = readJsonFile(join(root, 'package.json'), 'package.json').version
    if (typeof version !== 'string' || version.length === 0) {
      errorLog('error: package.json has no version field')
      return 1
    }
    log(version)
    return 0
  }

  const consistency = checkVersionConsistency(root)

  if (flags.extractNotes) {
    if (!consistency.ok) {
      for (const error of consistency.errors) errorLog(`error: ${error}`)
      return 1
    }
    const section = findChangelogSection(root, consistency.version)
    if (!section.found) {
      errorLog(`error: ${section.reason}`)
      return 1
    }
    if (!section.body) {
      errorLog('error: CHANGELOG section body is empty')
      return 1
    }
    log(section.body)
    return 0
  }

  log('release version gate:')
  for (const [location, declared] of Object.entries(consistency.versions)) {
    const mark = declared === consistency.version ? 'ok ' : 'BAD'
    log(`  ${mark} ${location}: ${declared ?? 'missing'}`)
  }
  if (!consistency.ok) {
    for (const error of consistency.errors) log(`error: ${error}`)
    return 1
  }

  if (tag !== null) {
    const result = checkTag(tag, consistency.version)
    log(`  ${result.ok ? 'ok ' : 'BAD'} tag ${tag}: ${result.ok ? 'matches app version' : result.error}`)
    if (!result.ok) return 1
  }

  if (flags.changelog) {
    const section = findChangelogSection(root, consistency.version)
    log(`  ${section.found ? 'ok ' : 'BAD'} CHANGELOG.md: ${section.found ? `section for ${consistency.version}` : section.reason}`)
    if (!section.found) return 1
  }

  return 0
}

const isMainModule = process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href
if (isMainModule) {
  process.exitCode = runCli(
    process.argv.slice(2),
    fileURLToPath(new URL('..', import.meta.url)),
  )
}
