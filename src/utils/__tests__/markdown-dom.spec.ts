// @vitest-environment jsdom
import { describe, expect, it } from 'vitest'
import DOMPurify from 'dompurify'
import { renderMarkdown } from '@/utils/markdown'

/**
 * 生产路径（真实 DOM）下的公式渲染契约。
 *
 * markdown.spec.ts 运行在 Node（无 DOM）——DOMPurify.isSupported === false，
 * 消毒被跳过，只锁「marked + KaTeX」渲染段。本文件运行在 jsdom，消毒必然生效，
 * 完整走「KaTeX 渲染 → DOMPurify 消毒 → URL 改写」链路，锁定 WebView 中的实际输出。
 * 消毒是历史上最易出错的环节（白名单差一个标签，公式就碎成乱码），必须独立锁定。
 */
describe('renderMarkdown — 生产路径（DOMPurify 消毒激活）', () => {
  it('前置：本环境 DOMPurify 生效（否则本文件失去意义）', () => {
    expect(DOMPurify.isSupported).toBe(true)
  })

  it('行内公式消毒后完整存活（视觉树 katex-html 保留）', () => {
    const html = renderMarkdown('保证$1 \\le n \\le 10^5$成立', '')
    expect(html).toContain('class="katex"')
    // 视觉渲染依赖的定位内联样式必须保留（strut/vlist 撑起上下标）
    expect(html).toMatch(/style="[^"]*height:/)
  })

  it('块级公式消毒后保留 katex-display（可横向滚动的全局样式锚点）', () => {
    const html = renderMarkdown('$$\n\\sum_{i=1}^{n} i = \\frac{n(n+1)}{2}\n$$', '')
    expect(html).toContain('katex-display')
    expect(html).toContain('class="katex"')
  })

  it('MathML 无障碍树完整保留（semantics/annotation 及 encoding 属性）', () => {
    const html = renderMarkdown('设 $x_1$ 为变量', '')
    expect(html).toContain('<semantics>')
    expect(html).toContain('<annotation encoding="application/x-tex">x_1</annotation>')
  })

  it('可伸缩根号/括号走 svg 档，消毒后 svg 路径保留', () => {
    const html = renderMarkdown('$$\\sqrt{\\frac{n(n+1)}{2}}$$', '')
    expect(html).toContain('<svg')
    expect(html).toContain('katex-display')
  })

  it('公式与恶意内联 HTML 共存时消毒不失效（script/onerror 必须被剥）', () => {
    const html = renderMarkdown(
      '<img src=x onerror=alert(1)>\n\n$\\frac{a}{b}$\n\n<script>evil()</script>',
      '',
    )
    expect(html).not.toContain('<script')
    expect(html).not.toContain('onerror')
    expect(html).toContain('class="katex"')
  })

  it('消毒与相对图片 URL 改写共存（生产组合行为）', () => {
    const html = renderMarkdown(
      '![图](/api/public/img/a.png)\n\n其中 $a_i \\ge 0$',
      'https://oj.example.com',
    )
    expect(html).toContain('src="https://oj.example.com/api/public/img/a.png"')
    expect(html).toContain('class="katex"')
  })

  it('出错公式消毒后错误文本与 errorColor 内联样式保留（可见降级）', () => {
    const html = renderMarkdown('$\\frac{$', '')
    expect(html).toContain('katex-error')
    expect(html).toMatch(/style="color:#dc2626"/)
  })

  it('消毒后容器 + 恶意内联 HTML 共存时 script 仍被剥', () => {
    const html = renderMarkdown(
      '::: hljs-center\n$$\ne^{e^x}\n$$\n:::\n\n<script>evil()</script>',
      '',
    )
    expect(html).toContain('<div class="hljs-center">')
    expect(html).not.toContain('<script')
  })

  it('Vditor 容器 div 与类名消毒后保留（居中公式完整链路）', () => {
    const html = renderMarkdown('::: hljs-center\n$$\nh(x)=e^{e^x}\n$$\n:::', '')
    expect(html).toContain('<div class="hljs-center">')
    expect(html).toContain('katex-display')
    expect(html).toContain('class="katex"')
  })
})

/**
 * CF 导入题源的废弃字号标签 —— <big> / <font size> 是"后续文字放大一圈"的根因：
 * DOMPurify 的 html 档默认放行它们，浏览器按 +1 档字号渲染，破坏客户端排印比例。
 */
describe('renderMarkdown — 废弃字号标签剥离', () => {
  it('<big> 剥离、内容保留（修复后续文字放大一圈）', () => {
    const html = renderMarkdown('<big>后续文字保持正文字号</big>。', '')
    expect(html).toContain('后续文字保持正文字号')
    expect(html).not.toContain('<big')
  })

  it('<font size> 属性剥离，font 标签与 color 强调保留', () => {
    const html = renderMarkdown('<font size="5" color="red">强调文字</font>', '')
    expect(html).not.toContain('size=')
    expect(html).toContain('<font color="red">')
    expect(html).toContain('强调文字')
  })

  it('<small> / <center> 保留（合法排版语义，不在剥离范围）', () => {
    const html = renderMarkdown('<small>注释</small>', '')
    expect(html).toContain('<small>注释</small>')
  })

  it('真实 CF 风格数据端到端：big 剥离与公式渲染互不干扰', () => {
    // 来自实际题面的结构：容器内 \huge 公式 + <big> 包裹的后续段落（含 \large 行内公式）
    const html = renderMarkdown(
      '::: hljs-center\n\n' + '${\\huge h\\left ( x \\right ) = e^{e^{x}  }}$' + '\n:::\n\n' +
        '<big>输出结果至少有 ${\\large n}$ 位数字。</big>',
      '',
    )
    expect(html).toContain('<div class="hljs-center">')
    expect(html).toContain('class="katex"')
    expect(html).not.toContain('<big')
    expect(html).toContain('位数字')
  })
})
