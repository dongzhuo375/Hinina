import { describe, expect, it } from 'vitest'
import { findViolations, loadSources } from '../../scripts/check-layering.mjs'
import { stripComments } from '../../scripts/lib/source-scan.mjs'

/**
 * 分层门禁的静态约束用例（与 check-save-gate.spec.ts 同款机制）。
 *
 * 被测逻辑在 `scripts/check-layering.mjs`（本地 `npm run check:layering`、
 * CI frontend job 各跑一次）；注释剥离 / `.vue` 处理 / 目录遍历来自共享模块
 * `scripts/lib/source-scan.mjs`。放在 `src/` 之外是刻意的——`src/**` 的 tsconfig
 * 不含 Node 类型，静态检查不该污染覆盖率基线。
 *
 * 断言分两层：先用合成夹具锁住每条规则的判定（含变异验证的等价物——每类
 * 违规形态各给一份「种进去必须红」的夹具；跨行、动态 import 的非字面量形态、
 * Vue 模板文本三类曾是缺口，负例逐条覆盖），再对真实 `src/` 树跑一遍——后者
 * 是「View/composable 不 import Service、View/component 不 import bridge」
 * 两条红线在 `npm test` 里的实际保障（2026-11-09 分层欠账清偿后恒绿；
 * 新增直连会在这里红出来）。
 *
 * `.vue` 夹具注意：扫描只看 `<script>` 块，裸脚本夹具一律用 `.ts` 路径，
 * SFC 行为由专门的 `.vue` 用例覆盖。
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
      { path: 'views/A.ts', content: `import { configService } from '@/services/config.service'` },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('规则A')
    expect(violations[0]).toContain('views/A.ts:1:')
  })

  it('规则A：import type 同样算边（登记口径计入类型边）', () => {
    const violations = findViolations([
      { path: 'components/B.ts', content: `import type { EditorPrefs } from '@/services/config.service'` },
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
      { path: 'views/E.ts', content: `import '@/services/config.service'` },
    ])
    expect(sideEffect).toHaveLength(1)
  })

  it('规则A：相对路径跨入 services/ 目录同样算边', () => {
    const violations = findViolations([
      { path: 'views/F.ts', content: `import { configService } from '../services/config.service'` },
    ])
    expect(violations).toHaveLength(1)
  })

  it('规则A：裸目录 barrel（from 加 @/services、无尾斜杠）同样算边——与 bridge 判据对称', () => {
    const violations = findViolations([
      { path: 'views/F.ts', content: `import { configService } from '@/services'` },
    ])
    expect(violations).toHaveLength(1)
    // 同形不误伤：services 前缀的其它模块（如假设的 @/services-extra）不在判据内
    expect(findViolations([
      { path: 'views/F.ts', content: `import x from '@/services-extra'` },
    ])).toEqual([])
  })

  it('规则A：export ... from 再导出也算边', () => {
    const violations = findViolations([
      { path: 'views/G.ts', content: `export { EditorPrefs } from '@/services/config.service'` },
    ])
    expect(violations).toHaveLength(1)
  })

  it('规则B：View/component/App.vue import bridge → 违规（含子路径与相对路径）', () => {
    for (const [path, content] of [
      ['views/H.ts', `import { IpcError } from '@/bridge'`],
      ['components/I.ts', `import * as configBridge from '../../bridge/config.bridge'`],
      ['App.vue', `<script setup lang="ts">\nimport '@/bridge'\n</script>`],
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
        path: 'views/K.ts',
        content: `export { a } from '@/bridge'; export { b } from '@/services/config.service'`,
      },
    ])
    expect(violations).toHaveLength(2)
  })

  it('注释里的示例不触发违规（扫描前剥离注释）', () => {
    const violations = findViolations([
      {
        path: 'views/L.ts',
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

  it('属性访问与标签模板不误报（x.from / x.import(…）', () => {
    const violations = findViolations([
      {
        path: 'composables/U.ts',
        content: `const f = x.from
const g = y.import('z')`,
      },
    ])
    expect(violations).toEqual([])
  })

  it('规则A：动态 import 换行（import 与说明符分离）同样算边', () => {
    const violations = findViolations([
      {
        path: 'views/P.ts',
        content: `const module = await import(
  '@/services/config.service'
)`,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('规则A')
  })

  it('规则A：副作用 import 换行同样算边', () => {
    const violations = findViolations([
      {
        path: 'views/Q.ts',
        content: `import
  '@/services/config.service'`,
      },
    ])
    expect(violations).toHaveLength(1)
  })

  it('规则A：from 与说明符换行分离同样算边（静态与 export type 两形态）', () => {
    const violations = findViolations([
      {
        path: 'views/R.ts',
        content: `import { configService } from
  '@/services/config.service'`,
      },
      {
        path: 'components/S.ts',
        content: `export type { EditorPrefs } from
  '@/services/config.service'`,
      },
    ])
    expect(violations).toHaveLength(2)
    // 行号回溯到 from 所在行（第二行说明符）
    expect(violations[0]).toContain('views/R.ts:1:')
  })

  it('规则B：bridge 的跨行 import 同样算边', () => {
    const violations = findViolations([
      {
        path: 'views/T.ts',
        content: `const bridge = await import(
  '@/bridge'
)`,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('规则B')
  })

  // ── 动态 import 的说明符形态：字面量可判定，其余 fail closed ──

  it('规则A：模板字符串字面量（无 ${}）的动态 import 算边', () => {
    const violations = findViolations([
      { path: 'views/V.ts', content: 'const m = await import(`@/services/config.service`)' },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('规则A')
  })

  it('动态 import 的非字面量形态一律 fail closed（变量 / 拼接 / ${} 插值）', () => {
    const shapes = [
      'const m = await import(spec)',
      "const m = await import('@/services/' + name)",
      'const m = await import(`@/services/${name}`)',
    ]
    for (const content of shapes) {
      const violations = findViolations([{ path: 'views/W.ts', content }])
      expect(violations, content).toHaveLength(1)
      expect(violations[0], content).toContain('无法静态分析')
    }
  })

  // ── Vue SFC：只扫 <script> 块 ──

  it('规则A：<script setup> 与 import 同处一行同样算边', () => {
    const violations = findViolations([
      {
        path: 'views/X.vue',
        content: `<script setup lang="ts">import '@/services/config.service'</script>`,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('规则A')
  })

  it('模板文本不参与扫描：散文里的 import 形状既不误报也不影响 script 判定', () => {
    const violations = findViolations([
      {
        path: 'views/Y.vue',
        content: `<template><p>use from '@/services/x' never</p></template>
<script setup lang="ts">
const a = 1
</script>`,
      },
    ])
    expect(violations).toEqual([])
  })

  it('模板里的撇号（Don\'t）不破坏 script 块的注释剥离与 import 判定', () => {
    const violations = findViolations([
      {
        path: 'views/Z.vue',
        content: `<template><p>Don't</p></template>
<script setup lang="ts">
/* 真实注释：from '@/services/ignored' */
import { x } from '@/services/config.service'
</script>`,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('views/Z.vue:4:')
  })

  it('规则A：闭合标签多空格（</script >）——真实解析器口径同样算边', () => {
    const violations = findViolations([
      {
        path: 'views/Aa.vue',
        content: `<script setup lang="ts">
import x from '@/services/real'
</script >`,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('规则A')
  })

  it('HTML 注释里的 script 示例不产生伪造违规', () => {
    const violations = findViolations([
      {
        path: 'views/Ab.vue',
        content: `<!-- <script>import x from '@/services/fake'</script> -->
<script setup>const ok = true</script>`,
      },
    ])
    expect(violations).toEqual([])
  })

  it('规则A：合法正则字面量含 /* 不再截断后续源码（P2-5）', () => {
    const violations = findViolations([
      {
        path: 'views/Ac.ts',
        content: `const r = /[/*]/
import { x } from '@/services/config.service'`,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('views/Ac.ts:2:')
  })

  it('规则A：控制语句闭括号后的正则字面量（if (ok) /[/*]/…）不再截断后续 import', () => {
    const violations = findViolations([
      {
        path: 'views/Ae.ts',
        content: `if (ok) /[/*]/.test(value)
import { x } from '@/services/config.service'`,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('views/Ae.ts:2:')
  })

  it('P3：多行块注释掩码保留换行——违规行号落在真实行', () => {
    const violations = findViolations([
      {
        path: 'views/Ad.vue',
        content: `<script setup lang="ts">
/* first
second */
import { x } from '@/services/config.service'
</script>`,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('views/Ad.vue:4:')
  })

  it('声明文件（.d.ts）同样在扫描面内', () => {
    const violations = findViolations([
      { path: 'views/types.d.ts', content: `import type { EditorPrefs } from '@/services/config.service'` },
    ])
    expect(violations).toHaveLength(1)
  })

  it('转义字形不绕过：说明符按运行时值解码后再判定（Unicode / hex / 身份 / 行续接）', () => {
    // 静态、副作用、动态三种入口都走解码；身份转义（\c → c、\i → i）同样是
    // 运行时等价字形——首轮的 `\\(.)` 正则多写一个反斜杠，此形态曾漏报；
    // 行续接（\ + 换行）在字符串里不产生字符，是第十一轮之后的又一漏报面
    const cases = [
      String.raw`import { configService } from '@/servi\u0063es/config.service'`,
      String.raw`import '@/servi\u0063es/config.service'`,
      String.raw`const m = await import('@/servi\u0063es/config.service')`,
      String.raw`const b = await import('@/br\u0069dge')`,
      String.raw`import { configService } from '@/servi\x63es/config.service'`,
      String.raw`import { configService } from '@/servi\ces/config.service'`,
      String.raw`const b = await import('@/br\idge')`,
      // 行续接：LF / CRLF / 动态 / bridge
      'import { x } from "@/servi\\\nces/config.service"',
      "import { x } from '@/servi\\\r\nces/config.service'",
      'const m = await import("@/servi\\\nces/config.service")',
      'const b = await import("@/br\\\nidge")',
    ]
    for (const content of cases) {
      const violations = findViolations([{ path: 'views/Esc.ts', content }])
      expect(violations, JSON.stringify(content)).toHaveLength(1)
    }
  })

  it('规则A：export default /[/*]/ 后的 import 仍被扫描（default 是正则先行词）', () => {
    const violations = findViolations([
      {
        path: 'views/Exp.ts',
        content: `export default /[/*]/
import { x } from '@/services/config.service'`,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('views/Exp.ts:2:')
  })

  it('规则A：模板表达式里的动态 import 同样算边（"imports cannot appear" 只对静态成立）', () => {
    const violations = findViolations([
      {
        path: 'views/Tpl.vue',
        content: `<template><button @click="import('@/services/config.service')">load</button></template>
<script setup lang="ts">const ready = true</script>`,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('规则A')
    // 插值里的动态 bridge import 同样命中（规则B）
    const bridge = findViolations([
      {
        path: 'views/TplB.vue',
        content: `<template><div>{{ import('@/bridge') }}</div></template>
<script setup lang="ts">const a = 1</script>`,
      },
    ])
    expect(bridge).toHaveLength(1)
    expect(bridge[0]).toContain('规则B')
  })

  it('说明符按字符串语义定界：异种引号、转义定界符都不截断（静态与动态）', () => {
    const cases = [
      // 双引号串里的单引号属于路径
      `import { x } from "@/foo'bar/services/config.service"`,
      // 单引号串里转义的同种定界符
      "import { x } from '@/foo\\'bar/services/config.service'",
      // 动态 import 的转义定界符：benign 前缀之后的 /services/ 段必须被读到
      "const m = import('@/safe\\')/services/config.service')",
    ]
    for (const content of cases) {
      const violations = findViolations([{ path: 'views/Quote.ts', content }])
      expect(violations, content).toHaveLength(1)
    }
  })

  it('未闭合或无法读取的字符串字面量按动态 import 违规 fail closed', () => {
    const violations = findViolations([
      { path: 'views/Unterminated.ts', content: "const m = import('@/services/x" },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('无法静态分析')
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
