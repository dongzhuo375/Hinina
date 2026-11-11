import { describe, expect, it } from 'vitest'
import {
  TXN_ALLOWLIST,
  findViolations,
  loadSources,
  txnCallSites,
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

  it('参数直传（纯转发）免扫——仅限同名转发入口：configStore.ts 里名为 runSaveTxn 的方法', () => {
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
  })

  it('参数直传豁免不外溢（评审 P2）——三类形近形态全部 fail closed', () => {
    // ① 任意 forward 别名：调用方写 forward(callback)，调用点不会出现
    //    runSaveTxn，回调正文根本没有被扫描 → 不豁免（方法名不是 runSaveTxn）
    const alias = findViolations([
      {
        path: 'stores/configStore.ts',
        content: 'export function forward(fn) { return configService.runSaveTxn(fn) }',
      },
    ])
    expect(alias).toHaveLength(1)
    expect(alias[0]).toContain('无法静态分析')
    // ② 同名外部回调撞上文件级参数名 → 不豁免（豁免只认转发入口自己的形参）
    const collision = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `import { sharedSaveHandler } from './handlers'
function unrelated(sharedSaveHandler) {}
await configService.runSaveTxn(sharedSaveHandler)`,
      },
    ])
    expect(collision).toHaveLength(1)
    expect(collision[0]).toContain('无法静态分析')
    // ③ 非参数的裸标识符（模块级常量等）→ 不豁免
    const moduleConst = findViolations([
      {
        path: 'stores/configStore.ts',
        content: 'const handler = makeHandler()\nrunSaveTxn<T>(fn) { return configService.runSaveTxn(handler) }',
      },
    ])
    expect(moduleConst.length).toBeGreaterThanOrEqual(1)
    expect(moduleConst.some((v) => v.includes('无法静态分析'))).toBe(true)
  })

  it('显式泛型调用 runSaveTxn<T>(cb) 被扫描（定义按形态跳过，不再按字面 <T>）', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `await configService.runSaveTxn<void>((tx) => {
          configService.saveSplitRatio(0.5)
          return tx.updateConfig(() => {})
        })`,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('排队保存入口')
    // 定义形态（含返回注解）不算调用点
    expect(
      txnCallSites(`async runSaveTxn<T>(
  fn,
  options = {},
): Promise<T> {
  return x(fn)
}`),
    ).toHaveLength(0)
    expect(txnCallSites('runSaveTxn<T>(fn, options = {}) { return configService.runSaveTxn(fn, options) }')).toHaveLength(1)
  })

  it('参数直传豁免限定文件：其它白名单文件里的同名方法形态仍 fail closed', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `runSaveTxn<T>(fn, options = {}) {
          return configService.runSaveTxn(fn, options)
        }`,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('无法静态分析')
  })

  it('Vue 模板指令表达式进入扫描面：@click 里的保存队列自等待 → 两条违规', () => {
    // 与 main 行为对齐：白名单 + 回调内排队入口各自报一条；模板里的撇号
    //（Don't）不得破坏 script 块的判定
    const violations = findViolations([
      {
        path: 'views/SomeView.vue',
        content: `<template>
  <p>Don't</p>
  <button @click="configStore.runSaveTxn((tx) => configStore.saveSplitRatio(0.5))" />
</template>
<script setup lang="ts">
const configStore = useConfigStore()
</script>`,
      },
    ])
    expect(violations).toHaveLength(2)
    expect(violations.some((v) => v.includes('不在白名单'))).toBe(true)
    expect(violations.some((v) => v.includes('排队保存入口'))).toBe(true)
  })

  it('Vue 插值表达式进入扫描面：{{ }} 里的保存队列自等待 → 两条违规', () => {
    const violations = findViolations([
      {
        path: 'views/InterpView.vue',
        content: `<template>
  <div>
    {{ configStore.runSaveTxn((tx) => configStore.saveSplitRatio(0.5)) }}
  </div>
</template>
<script setup lang="ts">const configStore = useConfigStore()</script>`,
      },
    ])
    expect(violations).toHaveLength(2)
    expect(violations.some((v) => v.includes('不在白名单'))).toBe(true)
    expect(violations.some((v) => v.includes('排队保存入口'))).toBe(true)
  })

  it('HTML 注释里的伪 @click 不产生违规（模板 AST 不含注释节点）', () => {
    const violations = findViolations([
      {
        path: 'views/CommentedView.vue',
        content: `<template>
  <!-- <button @click="configStore.runSaveTxn((tx) => configStore.saveSplitRatio(0.7))" /> -->
</template>
<script setup lang="ts">const c = 1</script>`,
      },
    ])
    expect(violations).toEqual([])
  })

  it('模板里的 HTML 实体不隐藏调用（带/不带分号、十进制/十六进制、插值同样）', () => {
    // 属性语境的浏览器解码规则：数字引用无论有无分号都解码——无分号变体
    // 曾零违规（评审第十一轮）
    const handlers = [
      'configStore.&#114;unSaveTxn((tx) => configStore.saveSplitRatio(0.8))',
      'configStore.&#114unSaveTxn((tx) => configStore.saveSplitRatio(0.8))',
      'configStore.&#x72unSaveTxn((tx) => configStore.saveSplitRatio(0.8))',
    ]
    for (const handler of handlers) {
      const violations = findViolations([
        {
          path: 'views/EntityView.vue',
          content: `<template>
  <button @click="${handler}" />
</template>
<script setup lang="ts">const configStore = useConfigStore()</script>`,
        },
      ])
      expect(violations, handler).toHaveLength(2)
      expect(violations.some((v) => v.includes('不在白名单')), handler).toBe(true)
      expect(violations.some((v) => v.includes('排队保存入口')), handler).toBe(true)
    }
    // 插值同样解码
    const interp = findViolations([
      {
        path: 'views/EntityInterp.vue',
        content: `<template><div>{{ configStore.&#114unSaveTxn((tx) => configStore.saveSplitRatio(0.8)) }}</div></template>
<script setup lang="ts">const configStore = useConfigStore()</script>`,
      },
    ])
    expect(interp).toHaveLength(2)
  })

  it('回调正文里的 /[}]/ 不截断事务回调（readBalanced 正则感知）', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `await configService.runSaveTxn((tx) => {
  const r = /[}]/
  configService.saveSplitRatio(0.5)
  return tx.updateConfig(() => {})
})`,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('排队保存入口')
  })

  it('模板散文不产生指令违规；script 字符串只有 mention 级触发（rule 1 口径）', () => {
    // 散文与 script 内的 @click 形字符串不含 runSaveTxn mention → 零违规：
    // 指令表达式提取不会把 script 内字符串当模板指令
    expect(
      findViolations([
        {
          path: 'views/Doc.vue',
          content: `<template><p>文档说 @click 会排队</p></template>
<script setup lang="ts">
const s = '@click="configStore.saveSplitRatio(0.5)"'
</script>`,
        },
      ]),
    ).toEqual([])
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
