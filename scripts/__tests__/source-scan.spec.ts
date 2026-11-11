import { describe, expect, it } from 'vitest'
import {
  lineOfIndex,
  prepareSource,
  scriptBlocksOnly,
  stripComments,
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
