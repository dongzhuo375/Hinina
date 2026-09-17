import { marked, type MarkedExtension, type Tokens } from 'marked'
import markedKatex from 'marked-katex-extension'
import DOMPurify from 'dompurify'

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

/// 将 Markdown 文本渲染为 HTML（含 LaTeX 公式），消毒后输出，并把相对 URL 改写为绝对地址。
///
/// HOJ 返回的题目描述/题面/公告是 Markdown + LaTeX 混合语法，其中图片使用相对路径，
/// 例如 `![alt](/api/public/img/xxx.png)`。此函数：
/// 1. 用 marked 将 Markdown 转为 HTML，`$...$` / `$$...$$` 经 KaTeX 渲染为公式
/// 2. 用 DOMPurify 在**唯一出口**统一消毒（marked 默认放行内联 HTML，而内容来自
///    OJ 服务端且编辑者面较宽；出口消毒使所有 v-html 调用点自动受保护，见 P63）
/// 3. 将以 `/` 开头的相对 `src`/`href` 改写为 `{baseUrl}` + 相对路径，
///    使图片能正确指向 HOJ 服务端而非前端 dev server
///
/// 消毒档位必须含 `mathMl` 与 `svg`：KaTeX 输出 MathML（读屏无障碍）与 SVG
/// （可伸缩括号/根号），只开 `html` 档会把公式结构整体剥掉、只剩乱码文本。
///
/// 注意：DOMPurify 依赖 DOM，Node 测试环境（无 jsdom）下 `isSupported === false`，
/// 此时跳过消毒仅做渲染 —— 生产环境恒为 WebView，消毒必然生效。
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

  if (DOMPurify.isSupported) {
    html = DOMPurify.sanitize(html, {
      USE_PROFILES: { html: true, mathMl: true, svg: true },
      // KaTeX 的读屏无障碍树由 <semantics>/<annotation> 承载（annotation 内是 LaTeX
      // 源码，供辅助技术与复制公式用），二者均不在 DOMPurify 预置 mathMl 白名单里，
      // 不补会被整体剥掉、LaTeX 源码反而泄漏成 <math> 内的裸文本。
      // 刻意只加这两个纯文本标签 —— mXSS 载体 <annotation-xml> 仍被拒之门外
      ADD_TAGS: ['semantics', 'annotation'],
      ADD_ATTR: ['encoding'],
      // CF 导入题源惯用 <big> / <font size> 抬升字号 —— 会破坏客户端排印比例
      // （题面正文被整体放大一档）。剥标签与属性、保留文字内容，字号回归
      // .prose 统一控制；作者语义性的排版（<font color>、<center>、<small>）保留
      FORBID_TAGS: ['big'],
      FORBID_ATTR: ['size'],
    })
  }

  if (baseUrl) {
    const base = baseUrl.replace(/\/+$/, '')
    html = html.replace(
      /\b(src|href)="(\/[^"]*)"/g,
      (_m, attr: string, path: string) => `${attr}="${resolveUrl(path, base)}"`,
    )
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
