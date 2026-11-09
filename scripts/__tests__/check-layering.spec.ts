import { describe, expect, it } from 'vitest'
import {
  findViolations,
  loadSources,
  stripComments,
} from '../../scripts/check-layering.mjs'

/**
 * 分层门禁的静态约束用例（与 check-save-gate.spec.ts 同款机制）。
 *
 * 被测逻辑在 `scripts/check-layering.mjs`（本地 `npm run check:layering`、
 * CI frontend job 各跑一次）。放在 `src/` 之外是刻意的——`src/**` 的 tsconfig
 * 不含 Node 类型，静态检查不该污染覆盖率基线。
 *
 * 断言分两层：先用合成夹具锁住每条规则的判定（含变异验证的等价物——每类
 * 违规形态各给一份「种进去必须红」的夹具），再对真实 `src/` 树跑一遍——后者
 * 是「View/composable 不 import Service、View/component 不 import bridge」
 * 两条红线在 `npm test` 里的实际保障（2026-11-09 分层欠账清偿后恒绿；
 * 新增直连会在这里红出来）。
 */

const cleanView = {
  path: 'views/Foo.vue',
  content: `
    <script setup lang="ts">
    import { useConfigStore } from '@/stores/configStore'
    import { useSubmissionStore } from '@/stores/submissionStore'
    const configStore = useConfigStore()
    await configStore.getOjBaseUrl()
    </script>
    <template><div /></template>
  `,
}

describe('check-layering — 规则判定（合成夹具）', () => {
  it('合规形态零违规（store 消费、类型从 store 取）', () => {
    expect(findViolations([cleanView])).toEqual([])
  })

  it('规则A：静态具名 import Service → 违规', () => {
    const violations = findViolations([
      { path: 'views/A.vue', content: `import { configService } from '@/services/config.service'` },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('规则A')
    expect(violations[0]).toContain('views/A.vue:1:')
  })

  it('规则A：import type 同样算边（登记口径计入类型边）', () => {
    const violations = findViolations([
      { path: 'components/B.vue', content: `import type { EditorPrefs } from '@/services/config.service'` },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('规则A')
  })

  it('规则A：多行 import 的 from 行被捕获', () => {
    const violations = findViolations([
      {
        path: 'composables/C.ts',
        content: "import type {\n  EditorPrefs,\n  PollSchedule,\n} from '@/services/config.service'",
      },
    ])
    expect(violations).toHaveLength(1)
  })

  it('规则A：动态 import() 与副作用 import 同样算边', () => {
    const dynamic = findViolations([
      { path: 'composables/D.ts', content: `const m = await import('@/services/config.service')` },
    ])
    expect(dynamic).toHaveLength(1)
    const sideEffect = findViolations([
      { path: 'views/E.vue', content: `import '@/services/config.service'` },
    ])
    expect(sideEffect).toHaveLength(1)
  })

  it('规则A：相对路径跨入 services/ 目录同样算边', () => {
    const violations = findViolations([
      { path: 'views/F.vue', content: `import { configService } from '../services/config.service'` },
    ])
    expect(violations).toHaveLength(1)
  })

  it('规则A：export ... from 再导出也算边', () => {
    const violations = findViolations([
      { path: 'views/G.vue', content: `export { EditorPrefs } from '@/services/config.service'` },
    ])
    expect(violations).toHaveLength(1)
  })

  it('规则B：View/component/App.vue import bridge → 违规（含子路径与相对路径）', () => {
    for (const [path, content] of [
      ['views/H.vue', `import { IpcError } from '@/bridge'`],
      ['components/I.vue', `import * as configBridge from '../../bridge/config.bridge'`],
      ['App.vue', `import '@/bridge'`],
    ] as const) {
      const violations = findViolations([{ path, content }])
      expect(violations, path).toHaveLength(1)
      expect(violations[0]).toContain('规则B')
    }
  })

  it('规则B 口径：composable import bridge 不在红线内（红线只列 View/component）', () => {
    const violations = findViolations([
      { path: 'composables/J.ts', content: `import { IpcError } from '@/bridge'` },
    ])
    expect(violations).toEqual([])
  })

  it('一行同时命中两层规则 → 两条违规各自报出（实际写法不存在，锁定扫描器语义）', () => {
    const violations = findViolations([
      {
        path: 'views/K.vue',
        content: `export { a } from '@/bridge'; export { b } from '@/services/config.service'`,
      },
    ])
    expect(violations).toHaveLength(2)
  })

  it('注释里的示例不触发违规（正则扫描前剥离注释）', () => {
    const violations = findViolations([
      {
        path: 'views/L.vue',
        content: `
          // 迁移前这里 from '@/services/config.service'（已收编 configStore）
          /* import { configService } from '@/services/config.service' */
          import { useConfigStore } from '@/stores/configStore'
        `,
      },
    ])
    expect(violations).toEqual([])
  })

  it('stripComments 保留字符串字面量（https:// 不被读成行注释）', () => {
    const src = `const url = 'https://hoj.example.com' // 注释`
    expect(stripComments(src)).toContain("'https://hoj.example.com'")
    expect(stripComments(src)).not.toContain('注释')
  })

  it('普通方法调用里的 from（如 Array.from）不误报', () => {
    const violations = findViolations([
      { path: 'composables/N.ts', content: `const ids = Array.from('abc')` },
    ])
    expect(violations).toEqual([])
  })
})

describe('check-layering — 真实源码树', () => {
  it('src/ 生产代码零违规（两条红线在迁移后恒绿；新增直连在此红出）', () => {
    expect(findViolations(loadSources(process.cwd()))).toEqual([])
  })

  it('扫描面完整（四个作用域均有文件被扫到，防目录改名后的假绿）', () => {
    const files = loadSources(process.cwd())
    const paths = files.map((f) => f.path)
    expect(paths.some((p) => p.startsWith('views/'))).toBe(true)
    expect(paths.some((p) => p.startsWith('components/'))).toBe(true)
    expect(paths.some((p) => p.startsWith('composables/'))).toBe(true)
    expect(paths).toContain('App.vue')
  })
})
