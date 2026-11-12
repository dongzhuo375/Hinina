import { describe, expect, it } from 'vitest'
import { findViolations, loadSources, GLOB_ALLOWLIST } from '../../scripts/check-layering.mjs'
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
 * 是「扫描面（含 utils/types 中转面）不 import Service、扫描面不 import
 * bridge / @tauri-apps/*（src 整树，bridge/ 与 tests 除外；stores 同禁 bridge——梯级）」两条红线在 `npm test`
 * 里的实际保障（2026-11-09
 * 分层欠账清偿后恒绿；
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

  it('规则B 口径：composable import bridge 同样违规（被 View 消费即抵达 IPC 的路径）', () => {
    // 第二十轮口径：composable 被 View 消费，放行它的 bridge 导入等于给 View
    // 留一条经 composable 抵达 IPC 的路径——规则 B 收编全部扫描面
    const violations = findViolations([
      { path: 'composables/J.ts', content: `import { IpcError } from '@/bridge'` },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('规则B')
  })

  it('规则B：直接 import @tauri-apps/api 同样违规（bridge 是唯一接触点）', () => {
    for (const content of [
      `import { invoke } from '@tauri-apps/api/core'`,
      `import { listen } from '@tauri-apps/api/event'`,
      `import api from '@tauri-apps/api'`,
    ]) {
      const violations = findViolations([
        {
          path: 'components/Invoke.vue',
          content: `<template><div/></template>\n<script setup>\n${content}\n</script>`,
        },
      ])
      expect(violations, content).toHaveLength(1)
      expect(violations[0], content).toContain('规则B')
      expect(violations[0], content).toContain('@tauri-apps/api')
    }
    // composable 里同样命中（与 bridge 同口径）
    const composable = findViolations([
      { path: 'composables/useIpc.ts', content: `import { invoke } from '@tauri-apps/api/core'` },
    ])
    expect(composable).toHaveLength(1)
    expect(composable[0]).toContain('规则B')
    // 插件包同样是 Tauri 边（第二十二轮：只匹配 @tauri-apps/api 曾放行 plugin-*）
    for (const spec of ['@tauri-apps/plugin-dialog', '@tauri-apps/plugin-opener']) {
      const plugin = findViolations([
        {
          path: 'views/Plugin.vue',
          content: `<template><div/></template>\n<script setup>\nimport { open } from '${spec}'\n</script>`,
        },
      ])
      expect(plugin, spec).toHaveLength(1)
      expect(plugin[0], spec).toContain('规则B')
    }
  })

  it('Tauri 红线整树：stores 同禁 bridge（梯级）；services/guards/router/main 只禁 Tauri 包', async () => {
    const serviceTauri = findViolations([
      { path: 'services/x.ts', content: `import { invoke } from '@tauri-apps/api/core'`, mode: 'tauriOnly' },
    ])
    expect(serviceTauri).toHaveLength(1)
    expect(serviceTauri[0]).toContain('规则B')
    // Service→Bridge 是合法梯级：services 不应用 bridge 路径规则
    const serviceBridge = findViolations([
      { path: 'services/x.ts', content: `import { IpcError } from '@/bridge'`, mode: 'tauriOnly' },
    ])
    expect(serviceBridge).toEqual([])
    // **stores 应用 bridge 半边**（第二十八轮）：Store→Bridge 破坏梯级
    const storeBridge = findViolations([
      { path: 'stores/x.ts', content: `import { IpcError } from '@/bridge'`, mode: 'store' },
    ])
    expect(storeBridge).toHaveLength(1)
    expect(storeBridge[0]).toContain('Store 不得 import bridge')
    // 扩展面不应用规则 A（Store→Service 合法）
    const storeService = findViolations([
      { path: 'stores/x.ts', content: `import { configService } from '@/services/config.service'`, mode: 'store' },
    ])
    expect(storeService).toEqual([])
    for (const [path, fileMode] of [
      ['stores/x.ts', 'store'],
      ['guards/g.ts', 'tauriOnly'],
      ['router/r.ts', 'tauriOnly'],
    ] as const) {
      const plugin = findViolations([
        { path, content: `import { open } from '@tauri-apps/plugin-dialog'`, mode: fileMode },
      ])
      expect(plugin, path).toHaveLength(1)
    }
    // loadSources 的整树标记：main.ts / env.d.ts 必在扫描面且为 tauriOnly；
    // stores 标记 'store'（bridge+Tauri），其余扩展面 'tauriOnly'
    //（评审第二十三轮：枚举目录曾漏掉根级文件，未来顶层目录由整树遍历覆盖）
    const files = loadSources(process.cwd())
    const main = files.find((f) => f.path === 'main.ts')
    expect(main, 'main.ts 缺失（扫描面 fail-closed）').toBeTruthy()
    expect(main?.mode).toBe('tauriOnly')
    const env = files.find((f) => f.path === 'env.d.ts')
    expect(env, 'env.d.ts 缺失（整树遍历应覆盖根级声明文件）').toBeTruthy()
    expect(env?.mode).toBe('tauriOnly')
    for (const file of files) {
      if (file.path.startsWith('stores/')) expect(file.mode, file.path).toBe('store')
      else if (/^(services|guards|router)\//.test(file.path)) expect(file.mode, file.path).toBe('tauriOnly')
      if (/^(views|components|composables|utils|types)\//.test(file.path) || file.path === 'App.vue') {
        expect(file.mode, file.path).toBeFalsy()
      }
    }
    // 整树承诺：除 bridge/ 与 tests 外，src 下每个脚本文件都在扫描面
    const { readdirSync } = await import('node:fs')
    const { join } = await import('node:path')
    const walked = new Set(files.map((file) => file.path))
    const walkAll = (dir, rel) => {
      for (const entry of readdirSync(dir, { withFileTypes: true })) {
        const full = join(dir, entry.name)
        const nextRel = rel === '' ? entry.name : `${rel}/${entry.name}`
        if (entry.isDirectory()) {
          if (/(^|\/)(__tests__|test-support)$/.test(nextRel) || nextRel === 'bridge') continue
          walkAll(full, nextRel)
          continue
        }
        if (!/\.(?:d\.ts|ts|tsx|mts|cts|js|jsx|mjs|cjs|vue)$/.test(entry.name)) continue
        expect(walked.has(nextRel), `未进扫描面：${nextRel}`).toBe(true)
      }
    }
    walkAll(join(process.cwd(), 'src'), '')
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

  it('import.meta.glob / globEager 默认 fail closed；allowlist 放行后按层规则判定', () => {
    // 终结式（第二十七轮）：逐轮补丁（明文通配 → 转义 → extglob → `**` 递归）
    // 无法穷尽 picomatch 语义，扫描面内 glob 调用默认 opaque；显式复核过的模式
    // 进 GLOB_ALLOWLIST 后，模式本身仍按规则 A/B 判定。
    const defaultLiteral = findViolations([
      { path: 'views/Glob.ts', content: `const m = import.meta.glob('@/services/*.service.ts')` },
    ])
    expect(defaultLiteral).toHaveLength(1)
    expect(defaultLiteral[0]).toContain('无法静态分析')
    expect(defaultLiteral[0]).toContain('GLOB_ALLOWLIST')
    GLOB_ALLOWLIST.push('@/services/*.service.ts', '@/bridge')
    try {
      const literal = findViolations([
        { path: 'views/Glob.ts', content: `const m = import.meta.glob('@/services/*.service.ts')` },
      ])
      expect(literal).toHaveLength(1)
      expect(literal[0]).toContain('规则A')
      const eager = findViolations([
        { path: 'views/Glob.ts', content: `const m = import.meta.glob('@/services/x.ts', { eager: true })` },
      ])
      expect(eager).toHaveLength(1)
      const bridge = findViolations([
        { path: 'views/Glob.ts', content: `const m = import.meta.globEager('@/bridge')` },
      ])
      expect(bridge).toHaveLength(1)
      expect(bridge[0]).toContain('规则B')
      // 非层目标放行后不误报
      GLOB_ALLOWLIST.push('@/assets/*.svg')
      const assets = findViolations([
        { path: 'views/Glob.ts', content: `const m = import.meta.glob('@/assets/*.svg')` },
      ])
      expect(assets).toEqual([])
    } finally {
      GLOB_ALLOWLIST.length = 0
    }
    for (const content of [
      `const m = import.meta.glob(['@/services/a.ts'])`,
      'const m = import.meta.glob(spec)',
    ]) {
      const violations = findViolations([{ path: 'views/Glob.ts', content }])
      expect(violations, content).toHaveLength(1)
      expect(violations[0], content).toContain('无法静态分析')
    }
  })

  it('glob 语义不可穷尽 → 一律 fail closed（明文/转义/extglob/** 递归，评审第二十四–二十七轮）', () => {
    // 同一处门禁的连续四轮修补清单，全部收敛为 opaque：Vite 对非相对/非绝对
    // 模式先 resolveId（别名生效）再展开，展开集合由 picomatch/tinyglobby 决定
    // ——`@/**` 与 `@/../**` 是**路径递归**（字面量前缀判据看不见 services/
    // bridge），转义与 extglob 表达同样集合。逐项打补丁逐轮留缺口，故终结为
    // 「glob 调用一律 fail closed + GLOB_ALLOWLIST」。
    for (const pattern of [
      '@/*/config.service.ts',
      '@/**/config.service.ts',
      '@/**/*.service.ts',
      '/src/**/config.service.ts',
      '@/**/*.bridge.ts',
      // 转义拼写：解码后才出现通配（第二十五轮）
      String.raw`@/\u002a/config.service.ts`,
      String.raw`@/\x2a/config.service.ts`,
      String.raw`@/\u003f/config.service.ts`,
      String.raw`@\u002f**\u002f*.service.ts`,
      // picomatch extglob：不含 `*?[{` 也能表达目录集合（第二十六轮）
      '@/@(services|utils)/config.service.ts',
      '@/+(services|utils)/config.service.ts',
      '@/!(utils)/config.service.ts',
      '@/(services|utils)/config.service.ts',
      // 末段 `**` 是路径递归而非文件名通配（第二十七轮）
      '@/**',
      '@/../**',
      '@/**.**',
      // 末段以目录分隔结尾的递归形态
      '@/**/',
    ]) {
      const violations = findViolations([
        { path: 'views/Glob.ts', content: `const m = import.meta.glob('${pattern}')` },
      ])
      expect(violations, pattern).toHaveLength(1)
      expect(violations[0], pattern).toContain('无法静态分析')
      expect(violations[0], pattern).toContain('GLOB_ALLOWLIST')
    }
  })

  it('模板表达式按原行号报告（不拼接、不跨表达式串味）', () => {
    const violations = findViolations([
      {
        path: 'views/Line.vue',
        content: `<template>
  <div>{{ import('@/services/config.service') }}</div>
</template>
<script setup lang="ts">const a = 1</script>`,
      },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('views/Line.vue:2:')
  })

  it('横向模块中转（utils 里再导出 service）同样算边', () => {
    const violations = findViolations([
      { path: 'utils/transit.ts', content: `export { configService } from '@/services/config.service'` },
    ])
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('规则A')
    // 规则 B 同样覆盖中转面：重导出 bridge 的 utils/types 文件本身就是违规点
    //（评审：规则 B 曾豁免中转，view → utils/ipc → bridge 全绿）
    const bridgeTransit = findViolations([
      { path: 'utils/ipc.ts', content: `import { IpcError } from '@/bridge'` },
    ])
    expect(bridgeTransit).toHaveLength(1)
    expect(bridgeTransit[0]).toContain('规则B')
    expect(bridgeTransit[0]).toContain('utils/ipc.ts:1:')
  })

  it('规则B：View 经 utils 中转摸 bridge 被斩断（中转文件本身红，评审 L1）', () => {
    const violations = findViolations([
      {
        path: 'views/Foo.vue',
        content:
          '<template><div/></template>\n<script setup>\nimport { getConfig } from "@/utils/ipc"\n</script>',
      },
      { path: 'utils/ipc.ts', content: 'export { getConfig } from "@/bridge/config"\n' },
    ])
    // View 自身的说明符（`@/utils/ipc`）不指名 bridge；红线在中转文件处收口
    expect(violations).toHaveLength(1)
    expect(violations[0]).toContain('utils/ipc.ts:1:')
    expect(violations[0]).toContain('规则B')
  })

  it('规则A/B：`<script src>` 外链脚本按说明符判定（编译期内联成真实依赖，评审 L2）', () => {
    const service = findViolations([
      {
        path: 'views/Bar.vue',
        content: '<template><div/></template>\n<script src="../services/config.service"></script>',
      },
    ])
    expect(service).toHaveLength(1)
    expect(service[0]).toContain('规则A')
    expect(service[0]).toContain('views/Bar.vue:2:')
    const bridge = findViolations([
      {
        path: 'views/Baz.vue',
        content: '<template><div/></template>\n<script src="@/bridge/config"></script>',
      },
    ])
    expect(bridge).toHaveLength(1)
    expect(bridge[0]).toContain('规则B')
    // 指向同目录非层文件的 src 不误报
    const local = findViolations([
      {
        path: 'views/Qux.vue',
        content: '<template><div/></template>\n<script src="./editor-setup"></script>',
      },
    ])
    expect(local).toEqual([])
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

  it('反斜杠路径按 / 归一化后判定（评审第二十八轮）；大小写不归一', () => {
    // `@/services\config.service` 在 Windows 上可解析（ubuntu build 会挡下，
    // 但门禁自身要闭合）；大小写不归一——Linux 上 `Services/` 可能是真实目录。
    // 源码里是 `\\`（JS 值里一个反斜杠）：单反斜杠是身份转义、解码后直接消失
    const cases = [
      String.raw`import { configService } from '@/services\\config.service'`,
      String.raw`const m = await import('@/services\\config.service')`,
      String.raw`import { ipc } from '@/bridge\\config.bridge'`,
    ]
    for (const content of cases) {
      const violations = findViolations([{ path: 'views/Back.ts', content }])
      expect(violations, JSON.stringify(content)).toHaveLength(1)
    }
    const caseSensitive = findViolations([
      { path: 'views/Case.ts', content: `import { x } from '@/Services/config.service'` },
    ])
    expect(caseSensitive).toEqual([])
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

  it('命名 HTML 实体不隐藏模板动态 import（评审第二十一轮：&lpar; / &sol; / @tauri-apps/api）', () => {
    // Vue 解析模板时按**完整 HTML5 实体表**解码，编译产物是真实动态 import；
    // 门禁曾用原文切片 + 六实体量表 → 零违规静默假绿。表达式文本改取 AST 的
    // 已解码 content 后全部命中。
    const cases = [
      `<button @click="import&lpar;'@/services/x'&rpar;">x</button>`,
      `<button @click="import('@/services&sol;x')">y</button>`,
      `<button @click="import&lpar;&apos;@tauri-apps&sol;api&sol;core&apos;&rpar;">z</button>`,
      `<button @click="import&lpar;'@/bridge/config'&rpar;">w</button>`,
      `<p>{{ import&lpar;'@/services/b'&rpar; }}</p>`,
    ]
    for (const markup of cases) {
      const violations = findViolations([
        {
          path: 'components/X.vue',
          content: `<template>${markup}</template>\n<script setup>const a = 1</script>`,
        },
      ])
      expect(violations, markup).toHaveLength(1)
      expect(violations[0], markup).toMatch(/规则[AB]/)
    }
  })

  it('模板表达式先剥注释：块注释不能拆开 import 与调用括号（评审第十九轮）', () => {
    // `import/*c*/(…)` 是合法动态 import，Vue 模板解析通过；表达式原样进入
    // 扫描面时既不像 `import(`、也不像不可读说明符，曾**零违规**静默假绿。
    // 与脚本块同口径（脚本块一直剥注释，跨行/转义等形态都已命中）。
    for (const content of [
      `<template><button @click="import/*x*/('@/services/config.service')" /></template>\n<script setup>const a = 1</script>`,
      `<template><button @click="import/*x*/('@/services/config.service', {})" /></template>\n<script setup>const a = 1</script>`,
    ]) {
      const violations = findViolations([{ path: 'views/Cmt.vue', content }])
      expect(violations, content).toHaveLength(1)
      expect(violations[0], content).toContain('规则A')
      expect(violations[0], content).toContain('views/Cmt.vue:1:')
    }
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
