import { describe, expect, it } from 'vitest'
import {
  TXN_ALLOWLIST,
  findViolations,
  loadSources,
  localFunctions,
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
    expect(violations[0]).toContain('间接引用')
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

  it('内联 function 回调的返回注解含对象类型时仍定位真实正文', () => {
    // `indexOf('{')` 曾把注解里的类型字面量当正文：截出的 ` ok: boolean ` 不含
    // 排队入口，真实回调体整条漏扫。类型本身合法（回调返回 Promise<{…}>）。
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `await configService.runSaveTxn(function (tx): Promise<{ ok: boolean }> {
  configService.saveSplitRatio(0.5)
  return tx.updateConfig(() => ({ ok: true }))
})`,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('排队保存入口')
    expect(violations[0]).toContain('saveSplitRatio')
  })

  it('对象类型返回注解的方法进函数表（(): { ok: boolean } 不再把类型字面量当正文）', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `class Repo {
  helper(): { ok: boolean } { configService.saveSplitRatio(0.5); return { ok: true } }
}
await configService.runSaveTxn((tx) => {
  new Repo().helper()
  return tx.updateConfig(() => {})
})`,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('间接引用排队入口')
    expect(violations[0]).toContain('saveSplitRatio')
  })

  it('function 声明的泛型返回注解含对象类型时正文不被类型字面量替代', () => {
    // `function f(): Array<{...}>` 曾被当成「注解里的 `{` 是正文起点」，函数表
    // 存的是 ` ok: boolean `，间接调用路径静默漏追。正文换行是刻意的：方法分支
    // 只认同行注解，此时仅 `declBody` 能收进函数表（用例覆盖声明分支本身）。
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `function persist(): Array<{ ok: boolean }>
{ return configService.saveSplitRatio(0.5) }
await configService.runSaveTxn((tx) => {
  persist()
  return tx.updateConfig(() => {})
})`,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('间接引用排队入口')
    expect(violations[0]).toContain('saveSplitRatio')
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

  it('参数直传豁免不外溢——三类形近形态全部 fail closed', () => {
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
    // 曾零违规
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

  it('动态指令参数（@[expr]）进入扫描面', () => {
    const violations = findViolations([
      {
        path: 'views/DynArg.vue',
        content: `<template><button @[runSaveTxn(handler)]="go" /></template>
<script setup lang="ts">const go = () => {}</script>`,
      },
    ])
    expect(violations.length).toBeGreaterThanOrEqual(1)
    expect(violations.some((v) => v.includes('不在白名单'))).toBe(true)
  })

  it('三元表达式里调用后跟 `: { … }` 不被误判为方法定义（成员调用无定义尾）', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `const result = flag
  ? configService.runSaveTxn((tx) => configService.saveSplitRatio(0.5)) : { fallback: true }`,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('排队保存入口')
  })

  it('function 回调体内含嵌套箭头时正文不被劫持（function 头先判定）', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `await configService.runSaveTxn(async function doSave(tx) {
  await configService.saveSplitRatio(0.5)
  const nested = () => null
  return nested
})`,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('排队保存入口')
  })

  it('嵌套泛型调用名同样被追踪（persist<Array<string>>）', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `function persist<T>(value: T) { return configService.saveSplitRatio(0.5) }
await configService.runSaveTxn((tx) => {
  persist<Array<string>>([])
  return tx.updateConfig(() => {})
})`,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('persist')
  })

  it('泛型实参里的函数类型（=>）不截断调用名（readAngles 跳过箭头）', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `function persist<T>(value: T) { return configService.saveSplitRatio(0.5) }
await configService.runSaveTxn((tx) => {
  persist<(e: Event) => void>(cb)
  return tx.updateConfig(() => {})
})`,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('persist')
  })

  it('带返回注解 + 嵌套泛型的方法进函数表（方法分支的注解与泛型感知）', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `class Repo {
  save<T>(v: Array<Map<string, T>>): void { configService.saveSplitRatio(0.5) }
}
const repo = new Repo()
await configService.runSaveTxn((tx) => {
  repo.save([])
  return tx.updateConfig(() => {})
})`,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('间接引用排队入口')
  })

  it('返回注解含括号（(): (e: Event) => void）的方法也进函数表（两种配平读取的返回值形态不同）', () => {
    // 旧实现把 readBalanced 返回的「括号内正文」当绝对下标（`closer - i`）：
    // 注解里的非空 `(e: Event)` 让 i 变 NaN 并静默放弃整条注解——这类方法
    // 整条漏出函数表，间接路径漏追。空 `()` 的兄弟形态更糟（i 归零、从文件头
    // 重扫：单行文件死循环，由 gate-cli.spec 的子进程用例守）。
    const table = localFunctions(
      `class C { helper(): (e: Event) => void { return configService.saveSplitRatio(0.5) } }`,
    )
    expect([...table.keys()]).toContain('helper')
    expect(table.get('helper')?.[0]).toContain('saveSplitRatio')
  })

  it('带默认值的转发形参不享受纯转发豁免（默认值可内联排队调用）', () => {
    const violations = findViolations([
      {
        path: 'stores/configStore.ts',
        content: `runSaveTxn<T>(fn = (tx) => configService.saveSplitRatio(0.5), options = {}) {
          return configService.runSaveTxn(fn, options)
        }`,
      },
    ])
    expect(violations.length).toBeGreaterThanOrEqual(1)
    // 默认值里的内联箭头已被赋值箭头分支收进函数表——走私的保存入口被**点名**
    //（比笼统的「无法静态分析」更精确；无法解析的默认值仍走 fail-closed，见下例）
    expect(violations.some((v) => v.includes('排队保存入口'))).toBe(true)
    // 非箭头默认值（无法解析成函数形态）依旧 fail closed
    const opaque = findViolations([
      {
        path: 'stores/configStore.ts',
        content: `runSaveTxn<T>(fn = makeHandler(), options = {}) {
          return configService.runSaveTxn(fn, options)
        }`,
      },
    ])
    expect(opaque.length).toBeGreaterThanOrEqual(1)
    expect(opaque.some((v) => v.includes('无法静态分析'))).toBe(true)
  })

  it('switchObservers 的混淆注册形态 fail closed（方括号 / add.bind）', () => {
    // bind 用例有两个出现点（`.add.` 别名形 + bind 实参里的裸引用），统一逐
    // 出现点判定后各报一条（别名/可选链曾让注册匹配落空）
    const cases: Array<[string, number]> = [
      [`switchObservers['add'](handler)`, 1],
      [`switchObservers.add.bind(switchObservers)(handler)`, 2],
    ]
    for (const [content, expected] of cases) {
      const violations = findViolations([
        { path: 'composables/useSettingsForm.ts', content },
      ])
      expect(violations, content).toHaveLength(expected)
      for (const violation of violations) expect(violation, content).toContain('无法静态分析')
    }
  })

  it('.call / .apply 调用排队入口同样算违规', () => {
    for (const call of [
      'configService.saveSplitRatio.call(null, 0.5)',
      'configService.saveSplitRatio.apply(null, [0.5])',
    ]) {
      const violations = findViolations([
        {
          path: 'composables/useSettingsForm.ts',
          content: `await configService.runSaveTxn((tx) => { ${call}; return tx.updateConfig(() => {}) })`,
        },
      ])
      expect(violations, call).toHaveLength(1)
      expect(violations[0], call).toContain('排队保存入口')
    }
  })

  it('排队入口的其余调用形态不再假绿（括号成员 / .bind / 可选括号 / 逗号表达式 / 标签模板）', () => {
    // 调用形态逐项补正则仍会漏下一种；规则 2 的契约是「不得引用」，
    // 因此改为引用级判定（调用形态 `QUEUED_ENTRY` + 全文件裸引用
    // `bareQueuedReferences`）——下列形态即使调用语法各不相同，
    // 名字出现即违规，不再依赖穷举。
    for (const call of [
      '(configService.saveSplitRatio)(0.5)',
      'configService.saveSplitRatio.bind(configService)(0.5)',
      "configService?.['saveSplitRatio']?.(0.5)",
      '(0, configService.saveSplitRatio)(0.5)',
      'configService.saveSplitRatio`x`',
    ]) {
      const violations = findViolations([
        {
          path: 'composables/useSettingsForm.ts',
          content: `await configService.runSaveTxn((tx) => { ${call}; return tx.updateConfig(() => {}) })`,
        },
      ])
      expect(violations, call).toHaveLength(1)
      expect(violations[0], call).toContain('排队保存入口')
    }
  })

  it('回调内仅引用（未调用）排队入口同样 fail closed（引用可经别名调用）', () => {
    const direct = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `await configService.runSaveTxn((tx) => { const later = configService.saveSplitRatio; return tx.updateConfig(() => {}) })`,
      },
    ])
    expect(direct).toHaveLength(1)
    expect(direct[0]).toContain('引用形态')
    expect(direct[0]).toContain('saveSplitRatio')
    // 同文件辅助函数里的裸引用同样报出（在**出现处**——不依赖调用链可达）
    const viaHelper = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `function pick() { return configService.saveSplitRatio }
await configService.runSaveTxn((tx) => {
  pick()
  return tx.updateConfig(() => {})
})`,
      },
    ])
    expect(viaHelper).toHaveLength(1)
    expect(viaHelper[0]).toContain('引用形态')
    expect(viaHelper[0]).toContain('saveSplitRatio')
  })

  it('模块顶层取别名、回调内经别名调用 → 违规（回调正文只见别名）', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `const up = configStore.updateEditorPrefs
await configStore.runSaveTxn(async (tx) => {
  up({})
  return tx.updateConfig(() => {})
})`,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('引用形态')
    expect(violations[0]).toContain('updateEditorPrefs')
    // 传参 / 属性分离形态同样在出现处违规
    for (const escape of [
      `register(configStore.saveSplitRatio)`,
      `const save = configStore['saveSplitRatio']`,
    ]) {
      const hit = findViolations([
        {
          path: 'composables/useSettingsForm.ts',
          content: `${escape}\nexport const ok = true`,
        },
      ])
      expect(hit, escape).toHaveLength(1)
      expect(hit[0], escape).toContain('引用形态')
    }
  })

  it('点号与属性名之间允许空白：`svc . runSaveTxn(…) : { … }` 仍被扫描', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `const result = flag
  ? configService . runSaveTxn((tx) => configService.saveSplitRatio(0.5)) : { fallback: true }`,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('排队保存入口')
  })

  it('同名函数多份定义全部检查（先出现的无关同名不再遮蔽真正的调用者）', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: `function persist(v) { return v }
await configService.runSaveTxn((tx) => {
  persist(1)
  return tx.updateConfig(() => {})
})
function persist() { return configService.saveSplitRatio(0.5) }`,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('persist')
    expect(violations[0]).toContain('saveSplitRatio')
  })

  it('嵌套模板字符串（`a${`b`}c`）不破坏注释剥离：注释里的调用不误报', () => {
    const violations = findViolations([
      {
        path: 'views/Nested.ts',
        content: `const s = \`a\${\`b\`}c\`
// configService.runSaveTxn(x)
configService.updateEditorPrefs({ fontSize: 15 })`,
      },
    ])
    // 唯一 mention 在注释里 → 零违规；updateEditorPrefs 是合法排队入口（回调外）
    expect(violations).toEqual([])
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

  it('观察者注册的别名/整体赋值形态 fail closed（逐出现点判定）', () => {
    // 别名：注册匹配落空、监听器不被扫描——出现即违规
    const alias = findViolations([
      { path: 'composables/useSettingsForm.ts', content: 'const reg = switchObservers.add\nreg(handler)' },
    ])
    expect(alias).toHaveLength(1)
    expect(alias[0]).toContain('无法静态分析')
    // 可选链：与直接 `.add(` 同为注册形态，监听器照样被扫描
    const optional = findViolations([
      { path: 'composables/useSettingsForm.ts', content: 'switchObservers?.add(handler)' },
    ])
    expect(optional).toHaveLength(1)
    expect(optional[0]).toContain('无法静态分析')
    // 整体赋值为现成 Set（可能把带监听器的集合换进来）→ fail closed
    const reassigned = findViolations([
      { path: 'composables/useSettingsForm.ts', content: 'switchObservers = importedSet' },
    ])
    expect(reassigned).toHaveLength(1)
    expect(reassigned[0]).toContain('无法静态分析')
    // 真树的四种良性形态全部零违规（声明 / delete / 展开 / 直接 add）
    const benign = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content: [
          'const switchObservers = new Set<SwitchObserver>()',
          'const observer = () => { reloadOjFields() }',
          'switchObservers.add(observer)',
          'switchObservers.delete(observer)',
          'const listeners = [...switchObservers]',
        ].join('\n'),
      },
    ])
    expect(benign).toEqual([])
  })

  it('辅助函数表补形态：函数表达式常量 / 类字段箭头 / 嵌套括号参数', () => {
    // 函数表达式常量
    const fnExpr = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content:
          'const persistOne = function (ratio: number) { return configService.saveSplitRatio(ratio) }\n' +
          'await configService.runSaveTxn(async (tx) => { persistOne(0.5); return tx.updateConfig(() => {}) })',
      },
    ])
    expect(fnExpr).toHaveLength(1)
    expect(fnExpr[0]).toContain('间接引用')
    expect(fnExpr[0]).toContain('saveSplitRatio')
    // 类字段箭头（`h = () => …`，无 const/let/var 前缀）
    const classField = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content:
          'class A { h = () => configService.saveSplitRatio(0.5) }\n' +
          'await configService.runSaveTxn(async (tx) => { new A().h(); return tx.updateConfig(() => {}) })',
      },
    ])
    expect(classField).toHaveLength(1)
    expect(classField[0]).toContain('间接引用')
    // 参数里带嵌套括号的箭头（旧 `\[^)]*\` 在 `(a = (1, 2))` 处截断）
    const nested = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content:
          'const persistTwo = (a = (1, 2)) => configService.saveSplitRatio(a)\n' +
          'await configService.runSaveTxn(async (tx) => { persistTwo(); return tx.updateConfig(() => {}) })',
      },
    ])
    expect(nested).toHaveLength(1)
    expect(nested[0]).toContain('间接引用')
  })

  it('以引用方式传入的辅助函数同样被追踪（list.map(persistOne)）', () => {
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content:
          'function persistOne(ratio: number) { return configService.saveSplitRatio(ratio) }\n' +
          'await configService.runSaveTxn(async (tx) => { [0.5].map(persistOne); return tx.updateConfig(() => {}) })',
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('间接引用')
    expect(violations[0]).toContain('persistOne')
  })

  it('对象字面量属性方法体里的排队调用同样被追踪', () => {
    // 冒号形态过去完全不进函数表：回调里 `svc.run()` 只带成员名，间接死锁
    // 路径漏追（运行时会排在回调已持有的保存门后）。属性箭头与函数表达式都收。
    const arrowProp = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content:
          'const svc = { run: async () => configService.saveSplitRatio(1) }\n' +
          'await configService.runSaveTxn(async (tx) => { svc.run(); return tx.updateConfig(() => {}) })',
      },
    ])
    expect(arrowProp).toHaveLength(1)
    expect(arrowProp[0]).toContain('间接引用')
    expect(arrowProp[0]).toContain('run')
    const fnProp = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content:
          'const svc = { persist: function (r) { return configService.updateEditorPrefs({ r }) } }\n' +
          'await configService.runSaveTxn(async (tx) => { svc.persist(1); return tx.updateConfig(() => {}) })',
      },
    ])
    expect(fnProp).toHaveLength(1)
    expect(fnProp[0]).toContain('间接引用')
    // 普通 `key: value` 不误收（快速失败，方向保守）
    const plain = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content:
          'const opts = { ratio: 0.5, label: "x" }\n' +
          'await configService.runSaveTxn(async (tx) => { use(opts.ratio); return tx.updateConfig(() => {}) })',
      },
    ])
    expect(plain).toEqual([])
  })

  it('包装器检查按定义形态定位，不再锁定首个同名文本', () => {
    // 更早出现的同名**定义形态**（对象方法）曾抢先命中 indexOf，真正的包装器
    // 绕过队列反而没被检查
    const violations = findViolations([
      {
        path: 'services/config.service.ts',
        content: [
          'export const legacyCallers = {',
          '  saveSplitRatio(ratio: number) { return this.runSaveTxn((tx) => tx.updateConfig(ratio)) },',
          '}',
          'export class ConfigService {',
          '  async saveSplitRatio(ratio: number): Promise<void> {',
          '    await this.updateConfigNow(() => {})',
          '  }',
          '  async updateEditorPrefs(patch: Partial<EditorPrefs>): Promise<void> {',
          '    return this.runSaveTxn((tx) => { tx.updateConfig(patch) })',
          '  }',
          '}',
        ].join('\n'),
      },
    ])
    expect(violations.length).toBeGreaterThanOrEqual(1)
    expect(violations.some((v) => v.includes('直连 updateConfigNow'))).toBe(true)
  })

  it('定义检测不跨前导换行：log(1) 之后下一行的无关 if 块不再被当成方法体', () => {
    // if 块里放一次**合法的**排队调用（模块层的队列调用本身不违规）：旧实现
    // 把 if 块吸收成 log 的方法体，回调里 `log(cfg)` 经传递路径假报；修复后
    // log 不进函数表，零违规。
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content:
          'const cfg = log(1)\n' +
          'if (depth > 0) { configService.saveSplitRatio(0.5) }\n' +
          'await configService.runSaveTxn(async (tx) => { log(cfg); return tx.updateConfig(() => {}) })',
      },
    ])
    expect(violations).toEqual([])
  })

  it('裸调用后跟三元对象字面量（`: { ok: false }`）仍被扫描', () => {
    // 成员调用的同型用例已闭合；裸名调用的定义尾判定同样不能被
    // 三元后件的对象字面量骗过
    const violations = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content:
          'const r = ok ? runSaveTxn(async (tx) => { await saveSplitRatio(0.5) }) : { ok: false }\n' +
          'export const ok = true',
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('排队保存入口')
    // 对照：`: null`（无对象字面量尾）同样报出
    const control = findViolations([
      {
        path: 'composables/useSettingsForm.ts',
        content:
          'const r = ok ? runSaveTxn(async (tx) => { await saveSplitRatio(0.5) }) : null\n' +
          'export const ok = true',
      },
    ])
    expect(control).toHaveLength(1)
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
