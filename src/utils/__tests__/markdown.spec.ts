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
