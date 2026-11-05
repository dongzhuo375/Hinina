import { marked, type MarkedExtension, type Tokens } from 'marked'
import markedKatex from 'marked-katex-extension'
import DOMPurify from 'dompurify'
import { resolveExternalUrl } from '@/utils/external-links'

/**
 * 数学公式（KaTeX）配置 —— 在 tokenizer 层接管 `$...$` / `$$...$$`。
 *
 * 为什么必须在 tokenizer 层而不是「先 markdown 后找 $」：LaTeX 里的 `_`、`*`、`\`
 * 会被 markdown 先行吃掉（`$x_1$` → `$x<em>1$`），事后再补救已经无从还原。
 *
 * 各选项的理由：
 * - `nonStandard: true`：**中文题面的关键开关**。标准规则要求 `$` 前是空格/行首、
 *   后跟空白或标点，而中文行文没有空格（`保证$1 \le n \le 10^5$成立`），
 *   标准规则会整段失配、公式退化成字面文本。
 *   已知取舍：正文里成对出现的货币 `$`（如「价格 $5 和 $10」）会被误判为公式 ——
 *   ACM 题面极少出现货币符号，而中文数学式无空格是常态，故取后者；该行为有测试锁定。
 * - `throwOnError: false`：OJ 题面由组织者手写，LaTeX 写错是常态；抛错会让整段
 *   题面渲染失败（白屏级事故），改为把出错片段渲染成红色字面文本，选手仍可读。
 * - `strict: 'ignore'`：未知命令不再刷 console 警告 —— 错误已由上一项以红色文本
 *   可见地呈现，日志噪音只会淹没真实问题。
 * - `trust` 保持默认 false：禁用 `\href` / `\url` / `\includegraphics` 等可注入
 *   链接与外链的宏，公式输出因此不含任何 URL（安全前提，勿开）。
 */
marked.use(
  markedKatex({
    nonStandard: true,
    throwOnError: false,
    errorColor: '#dc2626',
    strict: 'ignore',
  }),
)

/// Vditor 排版容器（`:::` 块）的 token 结构（marked 扩展自定义类型）。
interface ContainerToken extends Tokens.Generic {
  type: 'vditorContainer'
  /// 容器名（`hljs-center` 等），直接作为输出 div 的 class
  className: string
}

/**
 * Vditor 排版容器 —— HOJ 管理端 Vditor 编辑器"居中/居右"按钮的产物：
 *
 * ```md
 * ::: hljs-center
 * $$
 * h(x) = e^{e^x}
 * $$
 * :::
 * ```
 *
 * marked 没有 `:::` 容器概念：不接管时整块按字面输出 —— 题面上 "::: hljs-center"
 * 直接印在页面里，容器内本应居中的公式连 KaTeX 都不经过（`$$` 与容器行同段时
 * 块级公式规则无法命中）。实现为块级扩展：
 *
 * - `start`：marked 的 startBlock 钩子。容器紧跟正文段落（无空行分隔）时，
 *   按此位置截断段落候选，容器才能作为独立块被识别，而不是被吞进段落文本。
 * - 类名限定 `[\w-]+`：容器名直接进 `class` 属性，字符集白名单杜绝属性注入；
 *   不合法（含引号/空格等）或不闭合的容器一律不匹配，按字面降级为文本 ——
 *   组织者手写错误不致整块内容丢失。
 * - 内部内容递归走完整块级管线（`lexer.blockTokens`）：容器内的 `$$` 公式、
 *   列表、表格照常解析，对齐语义由 global.css 的 `.hljs-center` 等类提供。
 */
const vditorContainer: MarkedExtension = {
  extensions: [
    {
      name: 'vditorContainer',
      level: 'block',
      start(src: string) {
        return src.match(/^:::[ \t]*[\w-]/m)?.index
      },
      tokenizer(
        this: { lexer: { blockTokens(src: string): Tokens.Generic[] } },
        src: string,
      ) {
        const match = src.match(
          /^:::[ \t]*([\w-]+)[ \t]*\n([\s\S]*?)\n?:::(?:[ \t]*(?=\n|$))/,
        )
        if (!match) return undefined
        const token: ContainerToken = {
          type: 'vditorContainer',
          raw: match[0],
          className: match[1],
          tokens: this.lexer.blockTokens(match[2]),
        }
        return token
      },
      renderer(
        this: { parser: { parse(tokens: Tokens.Generic[]): string } },
        token: Tokens.Generic,
      ) {
        const { className, tokens } = token as ContainerToken
        // tokenizer 恒置 tokens，?? [] 仅安抚类型（Tokens.Generic.tokens 可选）
        return `<div class="${className}">${this.parser.parse(tokens ?? [])}</div>`
      },
    },
  ],
}

marked.use(vditorContainer)

// 锚点加固（外链 fail-safe）：只有**外链**（解析后 http/https 且 host ≠ 应用
// host）才注入 `target="_blank"` + `rel="noopener noreferrer"`。
//
// 为什么必须排除站内/非 http(s)：`target="_blank"` 会把点击变成新窗口请求，而
// Tauri/wry 默认拒绝新窗口——对不会被拦截器转交的链接（同 host、`mailto:`、
// `tel:`、`#` 片段）而言，加 target 等于把原本正常的跳转变成「点了没反应」。
// 判据与拦截器（utils/external-links）**共用 `resolveExternalUrl`**：URL 解析 +
// http(s) + host 不同——只有一处定义，不会两边漂移。
//
// 相对 URL 改写也在这里做（同一钩子、**属性级**）：只遍历元素的 `src`/`href`
// 属性，天然不会误伤文本节点/代码块——`<img src=/api/…>` 写在围栏代码或正文里
// 只是文本，不会被当成属性。字符串正则做不到这个区分（会把代码样例里的
// `src=/...` 也改写、还会吞掉转义的 `&gt;`）。
//
// 钩子注册在**本模块私有的 DOMPurify 实例**上（`DOMPurify(window)`），不动全局
// 单例——应用里其它 `DOMPurify.sanitize` 调用不应继承 target/rel。
// 改写发生在消毒**之后**的 DOM 属性上，因此先校验 baseUrl scheme（只接受
// http/https 或协议相对），防止 `javascript:` 基址绕过白名单。
// Node 测试环境（无 DOM）下 isSupported 为 false：不创建实例、跳过消毒与改写
// （相对 URL 改写的用例在 markdown-dom.spec.ts）。
// （分层审计 2026-10「外链无出路」缺口的渲染半边；转交半边在 Rust open_external。）

/// 本次 sanitize 期间的相对 URL 改写基址（空串 = 不改写）。
/// `renderMarkdown` 同步调用 sanitize，钩子在同一个调用栈内读取，无并发问题。
let rewriteBase = ''

/// 校验改写基址：只接受 http/https 绝对地址或协议相对 `//host`；
/// 空串/畸形/危险 scheme（`javascript:`、`data:` 等）一律返回空串（不改写）。
function validRewriteBase(baseUrl: string): string {
  const base = baseUrl.replace(/\/+$/, '')
  if (!base) return ''
  if (base.startsWith('//')) return base
  try {
    const parsed = new URL(base)
    if (parsed.protocol === 'http:' || parsed.protocol === 'https:') return base
  } catch {
    // 非绝对 URL：拒绝改写
  }
  return ''
}

function createPurifier(): typeof DOMPurify | null {
  if (typeof window === 'undefined' || !DOMPurify.isSupported) return null
  const appDocument = window.document
  const instance = DOMPurify(window)
  instance.addHook('afterSanitizeAttributes', (node) => {
    // 空/纯空白 href（`[x]()`）解析后等于当前文档 URL：点击 = 整页重载，会丢
    // 2s 防抖内的编辑器输入与未保存的设置草稿。无目标 → 占位链接（不可点，
    // 也不显示手型），比「可点但点了没反应」更诚实。
    if (node.tagName === 'A') {
      const href = node.getAttribute('href')
      if (href !== null && href.trim() === '') node.removeAttribute('href')
    }
    // 相对 URL 改写：只碰属性，文本/代码块天然不受影响
    if (rewriteBase) {
      for (const attr of ['src', 'href'] as const) {
        const value = node.getAttribute(attr)
        if (value?.startsWith('/')) node.setAttribute(attr, resolveUrl(value, rewriteBase))
      }
    }
    if (node.tagName !== 'A') return
    // 与拦截器同一条判据：只有外链才加固（站内/非 http(s) 加 target 会变死链接）
    if (!resolveExternalUrl(node.getAttribute('href'), appDocument)) return
    node.setAttribute('target', '_blank')
    node.setAttribute('rel', 'noopener noreferrer')
  })
  return instance
}

const purifier = createPurifier()

/// 将 Markdown 文本渲染为 HTML（含 LaTeX 公式），消毒后输出，并把相对 URL 改写为绝对地址。
///
/// HOJ 返回的题目描述/题面/公告是 Markdown + LaTeX 混合语法，其中图片使用相对路径，
/// 例如 `![alt](/api/public/img/xxx.png)`。此函数：
/// 1. 用 marked 将 Markdown 转为 HTML，`$...$` / `$$...$$` 经 KaTeX 渲染为公式
/// 2. 用 DOMPurify 在**唯一出口**统一消毒（marked 默认放行内联 HTML，而内容来自
///    OJ 服务端且编辑者面较宽；出口消毒使所有 v-html 调用点自动受保护，见 P63）
/// 3. 消毒钩子里把以 `/` 开头的相对 `src`/`href` 改写为 `{baseUrl}` + 路径
///    （属性级：只碰真实元素，文本/代码块里的同形字面量原样保留）
///
/// 消毒档位必须含 `mathMl` 与 `svg`：KaTeX 输出 MathML（读屏无障碍）与 SVG
/// （可伸缩括号/根号），只开 `html` 档会把公式结构整体剥掉、只剩乱码文本。
///
/// 注意：DOMPurify 依赖 DOM，Node 测试环境（无 jsdom）下 `isSupported === false`，
/// 此时不创建消毒实例、跳过消毒与 URL 改写仅做渲染 —— 生产环境恒为 WebView，
/// 消毒必然生效。
export function renderMarkdown(markdown: string, baseUrl: string): string {
  if (!markdown) return ''

  let html: string
  try {
    // 同步渲染（marked 默认同步，返回 string）
    html = marked.parse(markdown) as string
  } catch {
    // 渲染失败时退回原文（按纯文本输出）
    html = markdown
  }

  if (purifier) {
    rewriteBase = validRewriteBase(baseUrl)
    try {
      html = purifier.sanitize(html, {
        USE_PROFILES: { html: true, mathMl: true, svg: true },
        // KaTeX 的读屏无障碍树由 <semantics>/<annotation> 承载（annotation 内是 LaTeX
        // 源码，供辅助技术与复制公式用），二者均不在 DOMPurify 预置 mathMl 白名单里，
        // 不补会被整体剥掉、LaTeX 源码反而泄漏成 <math> 内的裸文本。
        // 刻意只加这两个纯文本标签 —— mXSS 载体 <annotation-xml> 仍被拒之门外。
        //
        // 剥离原则：只为安全剥（脚本/事件属性由 DOMPurify 白名单负责），表现性标记
        // （<big>/<font size>/<center> 等老题面排版）按作者原意保真渲染，与 HOJ
        // 网页端一致 —— 浏览器 font-size:larger 的放大比例在本客户端与 HOJ 等价。
        // 唯一的安全特例是下方 FORBID_TAGS 的 <style>（mid-body 存活见该注释）
        ADD_TAGS: ['semantics', 'annotation'],
        ADD_ATTR: ['encoding'],
        // <style> 显式禁（2026-11-05 实测）：DOMPurify html 档白名单**实际含 style**，
        // mid-body（正文在前、样式块在后，题面天然形态）的 <style> 原样存活 —— 可
        // 全文档改写界面（伪造弹窗）并借属性选择器 + url(...) 外泄；置于片段开头时
        // 被解析器归入 <head> 而 WHOLE_DOCUMENT=false 只处理 body，静默丢弃只是位置
        // 伪象，不是白名单剥除。style 在 DEFAULT_FORBID_CONTENTS 里，FORBID 删除时
        // CSS 文本一并丢弃、不回填正文。inline style **属性**不受影响（保真保留）。
        // <link>/<meta>/<base> 不在档内，mid-body 也被剥，无需另禁。
        //
        // <form> 显式禁（2026-11-06 实测）：html 档白名单含 form/input/button，
        // **连外域 action 属性都原样保留**——外链拦截器只拦锚点（closest('a')），
        // Rust 侧无 on_navigation，form 提交是消毒与导航两层都不覆盖的资源面：
        // 外域 action 被 CSP form-action 'self' 拦下（webview 替换洞已关），但
        // 无/self action 的提交会整页重载，丢 2s 防抖内的编辑器输入与草稿（与
        // [x]() 占位化同类危害）。题面/公告里 form 无合法渲染语义（提交只会打
        // 到 tauri.localhost 资产服务端）；form 不在 DEFAULT_FORBID_CONTENTS，
        // FORBID 只剥标签保子内容——input/button/select 照常渲染、失去提交路径，
        // 不破坏表现性保真原则。
        FORBID_TAGS: ['style', 'form'],
      })
    } finally {
      rewriteBase = ''
    }
  }

  return html
}

/// 将相对路径解析为绝对 URL。
function resolveUrl(path: string, base: string): string {
  // 已是绝对 URL 或特殊协议，原样返回
  if (/^https?:\/\//i.test(path) || path.startsWith('data:') || path.startsWith('//')) {
    return path
  }
  return base + path
}
