import { describe, expect, it } from 'vitest'
import {
  TXN_ALLOWLIST,
  findViolations,
  loadSources,
  txnCallbackBodies,
} from '../../scripts/check-save-gate.mjs'

/**
 * 保存门禁的静态约束用例。
 *
 * 被测逻辑在 `scripts/check-save-gate.mjs`（与 `check-release-versions.mjs` 同款
 * 脚本门禁：本地 `npm run check:save-gate`、CI frontend job 各跑一次）。放在
 * `src/` 之外是刻意的——`src/**` 的 tsconfig 不含 Node 类型，而且用 Vite `?raw`
 * 全量 import 源码会把 `.ts` 文件误标为已覆盖（实测把行覆盖从 72% 抬到 88%），
 * 静态检查不该污染覆盖率基线。
 *
 * 断言分两层：先用合成夹具锁住每条规则的判定（含变异验证的等价物），再对真实
 * `src/` 树跑一遍——后者才是「回调里嵌套排队入口」这道红线在 `npm test` 里的
 * 实际保障。
 */

const cleanTxnFile = {
  path: 'composables/useSettingsForm.ts',
  content: `
    export async function save() {
      await configService.runSaveTxn(async (tx) => {
        await tx.updateConfig((draft) => { draft.editor.fontSize = 16 })
      }, { settingsPage: true })
    }
  `,
}

const cleanServiceFile = {
  path: 'services/config.service.ts',
  content: `
    export class ConfigService {
      async runSaveTxn<T>(fn: (tx: SaveTxn) => Promise<T>): Promise<T> {
        return this.saveQueue.enqueue(() => fn({ updateConfig: (m) => this.updateConfigNow(m) }))
      }
      async updateEditorPrefs(patch) {
        await this.runSaveTxn((tx) => tx.updateConfig((draft) => { draft.editor.fontSize = patch.size }))
      }
      async saveSplitRatio(ratio) {
        await this.runSaveTxn((tx) => tx.updateConfig((draft) => { draft.layout.splitRatio = ratio }))
      }
    }
  `,
}

describe('check-save-gate — 规则判定（合成夹具）', () => {
  it('合规形态零违规', () => {
    expect(findViolations([cleanTxnFile, cleanServiceFile])).toEqual([])
  })

  it('回调内嵌套 runSaveTxn → 违规', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `
          await configService.runSaveTxn(async (tx) => {
            await configService.runSaveTxn((inner) => inner.updateConfig(() => {}))
          })
        `,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('排队保存入口')
  })

  it('回调内调用语义化包装（updateEditorPrefs / saveSplitRatio）→ 违规', () => {
    for (const call of [
      'await configService.updateEditorPrefs({ fontSize: 16 })',
      'void configService.saveSplitRatio(0.5)',
    ]) {
      const violations = findViolations([
        {
          path: 'composables/useSettingsForm.ts',
          content: `
            await configService.runSaveTxn(async (tx) => {
              ${call}
              await tx.updateConfig(() => {})
            })
          `,
        },
      ])
      expect(violations, call).toHaveLength(1)
      expect(violations[0]).toContain('排队保存入口')
    }
  })

  it('白名单外的文件使用 runSaveTxn → 违规（新增调用方须复核）', () => {
    const violations = findViolations([
      {
        path: 'views/ProblemSolveView.vue',
        content: `
          configService.runSaveTxn((tx) => tx.updateConfig((draft) => { draft.layout.splitRatio = r }))
        `,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('不在白名单')
  })

  it('公开包装绕过队列（直连 updateConfigNow）→ 违规', () => {
    const violations = findViolations([
      {
        path: 'services/config.service.ts',
        content: `
          export class ConfigService {
            async runSaveTxn<T>(fn) { return this.saveQueue.enqueue(() => fn(this.tx)) }
            async updateEditorPrefs(patch) { await this.runSaveTxn((tx) => tx.updateConfig(patch)) }
            async saveSplitRatio(ratio) { await this.updateConfigNow(() => {}) }
          }
        `,
      },
    ])
    expect(violations.some((v) => v.includes('saveSplitRatio'))).toBe(true)
  })

  it('注释里的示例不触发违规（正则扫描前剥离注释）', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `
          // 反例：不要在回调里再调 runSaveTxn / updateEditorPrefs
          await configService.runSaveTxn((tx) => tx.updateConfig(() => {}))
        `,
      },
    ])
    expect(violations).toEqual([])
  })

  it('回调提取同时支持块体与表达式体', () => {
    expect(txnCallbackBodies('runSaveTxn((tx) => tx.updateConfig(() => {}))')).toHaveLength(1)
    expect(txnCallbackBodies('runSaveTxn(async (tx) => { await tx.updateConfig(() => {}) })')).toHaveLength(1)
    // 定义本身不算调用点
    expect(txnCallbackBodies('async runSaveTxn<T>(fn: (tx: SaveTxn) => Promise<T>) {}')).toEqual([])
  })
})

describe('check-save-gate — 真实源码树', () => {
  it('src/ 生产代码零违规（事务回调内无排队入口，包装经队列）', () => {
    const violations = findViolations(loadSources(process.cwd()))
    // 失败先看 scripts/check-save-gate.mjs 顶部三条规则；新增事务调用方要显式加白名单
    expect(violations).toEqual([])
  })

  it('白名单只列真正需要事务的两个文件（防止被顺手扩大）', () => {
    expect(TXN_ALLOWLIST).toEqual([
      'services/config.service.ts',
      'composables/useSettingsForm.ts',
    ])
  })
})
