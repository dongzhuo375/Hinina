import { describe, expect, it } from 'vitest'
import { renderMarkdown } from '@/utils/markdown'

/**
 * renderMarkdown 单元测试。
 *
 * 注意：DOMPurify 消毒依赖 DOM，Node 测试环境（手册约定不引入 jsdom）下
 * `isSupported === false`，消毒分支不生效 —— 生产环境恒为 WebView，消毒必然生效。
 * 消毒行为本身由 DOMPurify 自身测试保障，此处只锁定渲染与 URL 改写契约。
 */
describe('renderMarkdown', () => {
  it('空输入返回空串', () => {
    expect(renderMarkdown('', 'https://oj.example.com')).toBe('')
  })

  it('Markdown 基础语法渲染为 HTML', () => {
    const html = renderMarkdown('# 标题\n\n正文 **加粗**', '')
    expect(html).toContain('<h1>标题</h1>')
    expect(html).toContain('<strong>加粗</strong>')
  })

  it('相对 src/href 改写为 OJ 绝对地址', () => {
    const html = renderMarkdown('![img](/api/public/img/a.png)', 'https://oj.example.com/')
    expect(html).toContain('src="https://oj.example.com/api/public/img/a.png"')
  })

  it('绝对 URL 与 data: 协议保持原样', () => {
    const html = renderMarkdown(
      '![a](https://cdn.example.com/a.png)\n\n[b](data:text/plain,x)\n\n[c](//cdn.example.com/c)',
      'https://oj.example.com',
    )
    expect(html).toContain('https://cdn.example.com/a.png')
    expect(html).toContain('data:text/plain,x')
    expect(html).toContain('//cdn.example.com/c')
  })

  it('baseUrl 为空时不改写相对路径', () => {
    const html = renderMarkdown('![img](/api/public/img/a.png)', '')
    expect(html).toContain('src="/api/public/img/a.png"')
  })
})

/**
 * LaTeX 公式渲染（KaTeX）。
 *
 * 题面数学符号此前完全无法渲染（`$...$` 按字面输出，且下标 `_` 先被 markdown
 * 吃成 `<em>`），以下用例锁定修复后的契约。
 */
describe('renderMarkdown — 数学公式', () => {
  it('行内公式 $...$ 渲染为 KaTeX 结构', () => {
    const html = renderMarkdown('设 $n$ 为整数', '')
    expect(html).toContain('class="katex"')
    expect(html).not.toContain('$n$')
  })

  it('块级公式 $$...$$ 渲染为 display 模式', () => {
    const html = renderMarkdown('$$\n\\sum_{i=1}^{n} i = \\frac{n(n+1)}{2}\n$$', '')
    expect(html).toContain('katex-display')
    expect(html).toContain('class="katex"')
  })

  it('下标不被 markdown 吃成 <em>（tokenizer 层接管的意义）', () => {
    const html = renderMarkdown('其中 $x_1 + x_2 = y$', '')
    expect(html).toContain('class="katex"')
    expect(html).not.toContain('<em>')
  })

  it('中文紧邻 $ 无空格也能识别（nonStandard：中文题面常态）', () => {
    const html = renderMarkdown('保证$1 \\le n \\le 10^5$成立，求最小值', '')
    expect(html).toContain('class="katex"')
    // 公式前后的中文正文必须完整保留
    expect(html).toContain('保证')
    expect(html).toContain('成立，求最小值')
  })

  it('LaTeX 写错时降级为可见错误文本，绝不抛错（throwOnError:false）', () => {
    // 组织者手写公式出错是常态：抛错会让整段题面白屏
    expect(() => renderMarkdown('数据范围 $\\frac{$ 与 $\\unknownmacro{x}$', '')).not.toThrow()
    const html = renderMarkdown('$\\frac{$', '')
    expect(html).toContain('katex-error')
  })

  it('行内代码与围栏代码块中的 $ 保持字面（不当公式渲染）', () => {
    const inline = renderMarkdown('执行 `echo $HOME` 查看', '')
    expect(inline).toContain('<code>')
    expect(inline).not.toContain('class="katex"')

    const fenced = renderMarkdown('```\nprice = $5\n```\n', '')
    expect(fenced).toContain('<code>')
    expect(fenced).not.toContain('class="katex"')
  })

  it('公式与普通 Markdown 结构共存（列表/加粗/标题）', () => {
    const html = renderMarkdown(
      '## 输入\n\n- 第一行包含 **整数** $n$\n- 第二行包含 $a_i$\n',
      '',
    )
    expect(html).toContain('<h2>输入</h2>')
    expect(html).toContain('<strong>整数</strong>')
    expect(html).toContain('<li>')
    expect(html.match(/class="katex"/g)?.length).toBeGreaterThanOrEqual(2)
  })

  it('已知取舍：正文成对货币 $ 会被当作公式（ACM 题面极少出现，取中文数学式优先）', () => {
    const html = renderMarkdown('价格 $5 和 $10 之间', '')
    expect(html).toContain('class="katex"')
  })

  it('公式渲染不影响图片相对 URL 改写', () => {
    const html = renderMarkdown('![图](/api/public/img/a.png)\n\n设 $n$ 为整数', 'https://oj.example.com')
    expect(html).toContain('src="https://oj.example.com/api/public/img/a.png"')
    expect(html).toContain('class="katex"')
  })
})
