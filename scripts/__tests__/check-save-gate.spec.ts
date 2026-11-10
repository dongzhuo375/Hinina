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
        path: 'views/ProblemSolveView.ts',
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

  it('回调经同文件辅助函数间接调用排队入口 → 违规（含调用链）', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `
          async function performSwitch(target: string): Promise<boolean> {
            await configService.saveSplitRatio(0.5)
            return true
          }
          export async function save() {
            await configService.runSaveTxn(async (tx) => {
              await tx.updateConfig(() => {})
              await performSwitch('Hydro')
            })
          }
        `,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('performSwitch')
    expect(violations[0]).toContain('saveSplitRatio')
    expect(violations[0]).toContain('间接调用')
  })

  it('两跳辅助函数同样能追到（回调 → a → b → 排队入口）', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `
          function innerHelper() { return configService.updateEditorPrefs({ fontSize: 15 }) }
          function outerHelper() { return innerHelper() }
          await configService.runSaveTxn((tx) => {
            outerHelper()
            return tx.updateConfig(() => {})
          })
        `,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('outerHelper')
    expect(violations[0]).toContain('innerHelper')
    expect(violations[0]).toContain('updateEditorPrefs')
  })

  it('表达式体箭头辅助函数同样追踪（=> expr，A 形态）', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `
          const pushPrefs = (tx) => tx.saveSplitRatio?.(0.5)
          await configService.runSaveTxn((tx) => {
            pushPrefs(tx)
            return tx.updateConfig(() => {})
          })
        `,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('pushPrefs')
    expect(violations[0]).toContain('saveSplitRatio')
  })

  it('带返回类型注解、正文换行的命名函数同样追踪（B 形态）', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `
          function pick(): () => void {
            return configService.saveSplitRatio(0.5)
          }
          await configService.runSaveTxn((tx) => {
            pick()
            return tx.updateConfig(() => {})
          })
        `,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('pick')
    expect(violations[0]).toContain('saveSplitRatio')
  })

  it('类/对象方法辅助函数同样追踪（C 形态）', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `
          class Ctl {
            helper() { return configService.updateEditorPrefs({ fontSize: 15 }) }
          }
          const ctl = new Ctl()
          await configService.runSaveTxn((tx) => {
            ctl.helper()
            return tx.updateConfig(() => {})
          })
        `,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('helper')
    expect(violations[0]).toContain('updateEditorPrefs')
  })

  it('带注解的函数声明正文不被返回类型注解里的 => 截断（B 的对照）', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `
          function makeLoader(): () => Promise<void> {
            return async () => { await configService.updateEditorPrefs({ fontSize: 15 }) }
          }
          await configService.runSaveTxn(async (tx) => {
            await makeLoader()
            await tx.updateConfig(() => {})
          })
        `,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('makeLoader')
    expect(violations[0]).toContain('updateEditorPrefs')
  })

  it('安全的辅助函数（只调独立队列 switchOj）不报违规', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `
          async function performSwitch(target: string) {
            await configService.switchOj(target)
          }
          await configService.runSaveTxn(async (tx) => {
            await performSwitch('Hydro')
            await tx.updateConfig(() => {})
          })
        `,
      },
    ])
    expect(violations).toEqual([])
  })

  it('互相递归的辅助函数不会让扫描挂住（环检测）', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `
          function ping() { return pong() }
          function pong() { return ping() }
          await configService.runSaveTxn((tx) => {
            ping()
            return tx.updateConfig(() => {})
          })
        `,
      },
    ])
    expect(violations).toEqual([])
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

  it('内联命名 function 回调同样被扫描（块体）', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `
          await configService.runSaveTxn(async function doSave(tx) {
            await configService.saveSplitRatio(0.5)
          })
        `,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('排队保存入口')
  })

  it('引用同文件命名回调：解析其正文并扫描（不再静默跳过）', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `
          async function doSave(tx) {
            await configService.updateEditorPrefs({ fontSize: 16 })
          }
          await configService.runSaveTxn(doSave)
        `,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('排队保存入口')
  })

  it('引用无法解析的回调（跨文件/别名/表达式）→ fail closed 违规', () => {
    for (const call of [
      'await configService.runSaveTxn(sharedSaveHandler)',
      'await configService.runSaveTxn(handlers.save)',
      'await configService.runSaveTxn(makeHandler())',
    ]) {
      const violations = findViolations([
        { path: 'composables/useSettingsForm.ts', content: call },
      ])
      expect(violations, call).toHaveLength(1)
      expect(violations[0]).toContain('无法静态分析')
    }
  })

  it('参数直传（纯转发，如 configStore）免扫——回调正文在调用点被逐点扫描', () => {
    // 无注解方法形态
    expect(
      findViolations([
        {
          path: 'stores/configStore.ts',
          content: `export const useStore = defineStore('c', { actions: {
            runSaveTxn<T>(fn, options = {}) {
              return configService.runSaveTxn(fn, options)
            },
          } })`,
        },
      ]),
    ).toEqual([])
    // 带返回注解的方法形态（configStore 的真实写法）
    expect(
      findViolations([
        {
          path: 'stores/configStore.ts',
          content: `runSaveTxn<T>(
            fn: (tx: SaveTxn) => Promise<T>,
            options: { settingsPage?: boolean } = {},
          ): Promise<T> {
            return configService.runSaveTxn(fn, options)
          }`,
        },
      ]),
    ).toEqual([])
    // function 声明形态
    expect(
      findViolations([
        {
          path: 'stores/configStore.ts',
          content: 'function forward(fn) { return configService.runSaveTxn(fn) }',
        },
      ]),
    ).toEqual([])
  })

  it('参数直传豁免不外溢：普通实参（非函数参数）仍 fail closed', () => {
    // 调用参数不是任何函数的形参 → 不算纯转发
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: 'const x = configService.runSaveTxn(sharedSaveHandler)',
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('无法静态分析')
  })

  it('可选调用 saveSplitRatio?.(…) / updateEditorPrefs?.(…) 也算排队入口', () => {
    for (const call of [
      'configService.saveSplitRatio?.(0.5)',
      'configService.updateEditorPrefs?.({ fontSize: 16 })',
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

  it('可选调用出现在同文件辅助函数里也能追踪到', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `
          function maybePersist() { return configService.saveSplitRatio?.(0.5) }
          await configService.runSaveTxn((tx) => {
            maybePersist()
            return tx.updateConfig(() => {})
          })
        `,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('maybePersist')
    expect(violations[0]).toContain('saveSplitRatio')
  })

  it('switchObservers 监听器调用排队入口 → 违规（观察者在事务回调内被 await）', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `
          const observer = (target) => {
            void configService.saveSplitRatio(0.5)
          }
          switchObservers.add(observer)
        `,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('switchObservers 监听器')
    expect(violations[0]).toContain('saveSplitRatio')
  })

  it('switchObservers 监听器经辅助函数间接调用排队入口 → 违规', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `
          function persistLater() { return configService.updateEditorPrefs({ fontSize: 15 }) }
          const observer = () => { persistLater() }
          switchObservers.add(observer)
        `,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('persistLater')
    expect(violations[0]).toContain('updateEditorPrefs')
  })

  it('安全的 switchObservers 监听器不报违规；无法解析的监听器 fail closed', () => {
    const safe = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `
          const observer = () => { reloadOjFields() }
          switchObservers.add(observer)
        `,
      },
    ])
    expect(safe).toEqual([])

    const unsafe = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `
          switchObservers.add(externalObserver)
        `,
      },
    ])
    expect(unsafe).toHaveLength(1)
    expect(unsafe[0]).toContain('无法静态分析')
  })
})

describe('check-save-gate — 真实源码树', () => {
  it('src/ 生产代码零违规（事务回调内无排队入口，包装经队列）', () => {
    const violations = findViolations(loadSources(process.cwd()))
    // 失败先看 scripts/check-save-gate.mjs 顶部三条规则；新增事务调用方要显式加白名单
    expect(violations).toEqual([])
  })

  it('白名单只列真正需要事务的文件（防止被顺手扩大）', () => {
    // stores/configStore.ts 不是事务调用方——它是分层收编后的纯转发入口
    // （2026-11-09 严格分层），列入只为通过规则 1 的标识符扫描
    expect(TXN_ALLOWLIST).toEqual([
      'services/config.service.ts',
      'composables/useSettingsForm.ts',
      'stores/configStore.ts',
    ])
  })
})
