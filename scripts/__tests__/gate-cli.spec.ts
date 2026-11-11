import { describe, expect, it } from 'vitest'
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs'
import { join } from 'node:path'
import { tmpdir } from 'node:os'
import { runCli as runLayering } from '../../scripts/check-layering.mjs'
import { runCli as runSave } from '../../scripts/check-save-gate.mjs'

/**
 * 两个源码门禁的 CLI 出口（`runCli`）用例：捕获返回码而不是内部判定。
 *
 * 仓库惯例：`check-release-versions.spec.ts` 用 captureGate 断言 CLI 返回码。
 * 没有这层，改坏 `runCli` / 主模块守卫时 CI 上的门禁恒绿而全部 spec 仍绿。
 */
function withTempSrc(files, fn) {
  const root = mkdtempSync(join(tmpdir(), 'hinina-gate-'))
  try {
    mkdirSync(join(root, 'src'), { recursive: true })
    for (const [rel, content] of Object.entries(files)) {
      const full = join(root, 'src', rel)
      mkdirSync(join(full, '..'), { recursive: true })
      writeFileSync(full, content)
    }
    return fn(root)
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
}

describe('check-layering — CLI 出口', () => {
  it('真实树通过返回 0', () => {
    expect(runLayering(process.cwd())).toBe(0)
  })

  it('空扫描面返回 1（而不是静默通过）', () => {
    expect(withTempSrc({}, (root) => runLayering(root))).toBe(1)
  })
})

describe('check-save-gate — CLI 出口', () => {
  it('真实树通过返回 0', () => {
    expect(runSave(process.cwd())).toBe(0)
  })

  it('空扫描面返回 1（fail closed）', () => {
    expect(withTempSrc({}, (root) => runSave(root))).toBe(1)
  })

  it('关键文件缺失返回 1（config.service.ts 被改名/挪走）', () => {
    expect(
      withTempSrc(
        { 'views/Ok.vue': '<template><div /></template>\n<script setup>const a = 1</script>' },
        (root) => runSave(root),
      ),
    ).toBe(1)
  })

  it('有违规时返回 1（门禁真的会红）', () => {
    const root = mkdtempSync(join(tmpdir(), 'hinina-gate-'))
    try {
      mkdirSync(join(root, 'src', 'views'), { recursive: true })
      writeFileSync(
        join(root, 'src', 'views', 'Bad.vue'),
        '<template><button @[runSaveTxn(h)]="go" /></template>\n<script setup>const a = 1</script>',
      )
      mkdirSync(join(root, 'src', 'services'), { recursive: true })
      writeFileSync(join(root, 'src', 'services', 'config.service.ts'), 'export const configService = {}')
      expect(runSave(root)).toBe(1)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})
