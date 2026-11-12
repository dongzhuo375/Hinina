import { describe, expect, it } from 'vitest'
import {
  buildLineIndex,
  lineOfIndex,
  prepareSource,
  scriptBlocksOnly,
  skipStringLiteral,
  stripComments,
  templateExpressions,
  vueScriptRanges,
} from '../../scripts/lib/source-scan.mjs'

/**
 * 共享扫描原语（`scripts/lib/source-scan.mjs`）的判定用例。
 *
 * 两个源码门禁（check-layering / check-save-gate）共用这份实现：注释剥离、
 * `.vue` 只扫 `<script>` 块、目录遍历。这里锁定原语本身的行为——门禁层的
 * 语义分别由各自 spec 覆盖。
 */

describe('source-scan — stripComments', () => {
  it('行注释与块注释被剥离，字符串字面量保留（含 // 的 URL）', () => {
    const src = `const url = 'https://x.test' // 行注释
/* 块注释 */ const a = 1`
    const out = stripComments(src)
    expect(out).toContain("'https://x.test'")
    expect(out).not.toContain('行注释')
    expect(out).not.toContain('块注释')
  })

  it('字符串内的注释记号不被误剥（Don\'t 类撇号在脚本里配对正常）', () => {
    const out = stripComments(`const s = 'a /* b */ c'; const t = 1`)
    expect(out).toContain('a /* b */ c')
  })
})

describe('source-scan — scriptBlocksOnly', () => {
  it('只保留 <script> 块内容，其余掩码为空格且换行保留（行号不变）', () => {
    const src = `<template>
  <p>Don't</p>
</template>
<script setup lang="ts">
import x from 'y'
</script>
<style>.a{}</style>`
    const out = scriptBlocksOnly(src)
    expect(out).toContain("import x from 'y'")
    expect(out).not.toContain("Don't")
    expect(out).not.toContain('.a{}')
    expect(out.split('\n')).toHaveLength(src.split('\n').length)
    // 掩码不改变列偏移：script 内的 import 仍在第四行
    expect(lineOfIndex(out, out.indexOf('import x'))).toBe(5)
  })

  it('无 script 块的 .vue 产出全空白（没有可扫描内容）', () => {
    const out = scriptBlocksOnly(`<template><div /></template>`)
    expect(out.trim()).toBe('')
  })
})

describe('source-scan — prepareSource', () => {
  it('.vue 走 script 块提取后再剥注释；.ts 直接剥注释', () => {
    const vue = prepareSource(
      'views/A.vue',
      `<template><p>x</p></template>
<script setup lang="ts">
// 注释
const a = 1
</script>`,
    )
    expect(vue).toContain('const a = 1')
    expect(vue).not.toContain('注释')

    const ts = prepareSource('views/A.ts', `// 注释\nconst a = 1`)
    expect(ts).toContain('const a = 1')
    expect(ts).not.toContain('注释')
  })

  it('偏移稳定（P3）：prepareSource 的输出长度与原始输入一致、换行逐一保留', () => {
    const src = `<template><p>Don't</p></template>\n<script>\n/* first\nsecond */\nconst a = 1\n</script>`
    const prepared = prepareSource('views/A.vue', src)
    expect(prepared).toHaveLength(src.length)
    expect(prepared.split('\n')).toHaveLength(src.split('\n').length)
    // 掩码不改变行号：多行块注释后的代码仍在真实行（第 5 行）
    expect(lineOfIndex(prepared, prepared.indexOf('const a = 1'))).toBe(5)
  })
})

describe('source-scan — SFC fail-closed 契约', () => {
  it('未闭合的 <script setup> 抛错（parse 的错误在返回值里，不是 throw）', () => {
    const unclosed = ['<template><div/></template>', '<script setup>', 'const a = 1'].join('\n')
    expect(() => vueScriptRanges(unclosed)).toThrow(/SFC 解析/)
  })

  it('templateExpressions 直接调用（不经 prepareSource）同样 fail closed', () => {
    expect(() => templateExpressions('<template><button @[a b]="go" /></template>')).toThrow(/SFC 解析/)
  })
})

describe('source-scan — 词法健壮性', () => {
  it('嵌套模板字符串：${`…`} 内部按 JS 词法扫描，字面量整体跳到正确终点', () => {
    const src = 'const s = `a${`b`}c` + 1'
    const end = skipStringLiteral(src, src.indexOf('`'))
    expect(src.slice(end)).toBe(' + 1')
  })

  it('嵌套模板里的 /* 不会把后续源码当成注释（偏移与内容不变）', () => {
    const src = `const s = \`x\${\`/*\`}y\`
import { z } from '@/stores/a'`
    const out = stripComments(src)
    expect(out).toHaveLength(src.length)
    expect(out).toContain("import { z } from '@/stores/a'")
  })

  it('模板表达式取 AST 已解码的 content（命名实体不再靠自带表）', () => {
    // `&lpar;` / `&sol;` 在完整 HTML5 表里，Vue 解析模板时解成正文字符——
    // 表达式文本必须是解码后的代码，否则动态 import 对门禁不可见（静默假绿）
    const source =
      '<template><button @click="import&lpar;\'@/services/x\'&rpar;">x</button>' +
      '<p>{{ import(\'@/services&sol;y\') }}</p></template>'
    expect(templateExpressions(source)).toEqual([
      "import('@/services/x')",
      "import('@/services/y')",
    ])
  })

  it('行索引：buildLineIndex + 二分 lineOfIndex 与逐步扫描一致', () => {
    const src = ['a', 'b', 'c', 'd'].join('\n')
    const starts = buildLineIndex(src)
    for (let i = 0; i < src.length; i += 1) {
      expect(lineOfIndex(src, i, starts)).toBe(lineOfIndex(src, i))
    }
  })
})

describe('source-scan — 正则字面量与控制括号', () => {
  it('正则字面量原样保留（字符类里的 /* 与 // 不截断源码）', () => {
    const src = `const r = /[/*]/
const u = /a\\/b/
import { x } from 'y'`
    const out = stripComments(src)
    expect(out).toHaveLength(src.length)
    expect(out).toContain('/[/*]/')
    expect(out).toContain("import { x } from 'y'")
  })

  it('成员调用的 catch 不算控制语境（promise.catch(h) 后是除法）', () => {
    const src = `promise.catch(h) / 2
import { x } from 'y'`
    const out = stripComments(src)
    expect(out).toHaveLength(src.length)
    expect(out).toContain('.catch(h) / 2')
  })

  it('控制语句的 catch 仍是正则语境（catch (e) /[/*]/…）', () => {
    const out = stripComments('try { x() } catch (e) /[/*]/.test(s)')
    expect(out).toContain('/[/*]/')
  })
})

describe('source-scan — lineOfIndex', () => {
  it('1-based 行号，跨多行正确', () => {
    const src = 'a\nb\nc'
    expect(lineOfIndex(src, 0)).toBe(1)
    expect(lineOfIndex(src, 2)).toBe(2)
    expect(lineOfIndex(src, 4)).toBe(3)
  })
})

describe('source-scan — readAngles 换行边界', () => {
  it('泛型实参只认单行：跨行扫描曾把游标推到后面无关的 `>`（旧守卫 depth===0 永不为真）', async () => {
    const { readAngles } = await import('../../scripts/lib/source-scan.mjs')
    // 单行平衡：返回闭括号后的绝对下标
    expect(readAngles('a < b > c', 2)).toBe(7)
    // 换行即放弃——`< b\nc >` 的 `>` 在下一行，属于无关表达式
    expect(readAngles('a < b\nc > d', 2)).toBeNull()
  })
})

describe('source-scan — 模板遍历器单一实现', () => {
  it('templateExpressions 与 templateExpressionEntries 是同一集合（文本/行号一致）', async () => {
    const { templateExpressionEntries } = await import('../../scripts/lib/source-scan.mjs')
    const source = [
      '<template>',
      '  <button @click="runSaveTxn(h)">{{ label }}</button>',
      '  <input :[dyn]="v" />',
      '</template>',
      '<script setup>const label = 1</script>',
    ].join('\n')
    const expressions = templateExpressions(source)
    const entries = templateExpressionEntries(source)
    expect(entries.length).toBeGreaterThan(0)
    expect(expressions).toEqual(entries.map((entry) => entry.text))
    // 行号是原文件行（:2 的指令 exp 与插值；:3 的动态参数 [dyn] 与绑定值 "v"）
    expect(entries.map((entry) => entry.line)).toEqual([2, 2, 3, 3])
  })
})
