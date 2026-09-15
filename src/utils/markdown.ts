import { marked } from 'marked'
import DOMPurify from 'dompurify'

/// 将 Markdown 文本渲染为 HTML，消毒后输出，并将相对 URL 改写为绝对地址。
///
/// HOJ 返回的题目描述/题面/公告是 Markdown 语法，其中图片使用相对路径，
/// 例如 `![alt](/api/public/img/xxx.png)`。此函数：
/// 1. 用 marked 将 Markdown 转为 HTML（`![alt](url)` → `<img src="url" alt="alt">`）
/// 2. 用 DOMPurify 在**唯一出口**统一消毒（marked 默认放行内联 HTML，而内容来自
///    OJ 服务端且编辑者面较宽；出口消毒使所有 v-html 调用点自动受保护，见 P63）
/// 3. 将以 `/` 开头的相对 `src`/`href` 改写为 `{baseUrl}` + 相对路径，
///    使图片能正确指向 HOJ 服务端而非前端 dev server
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
    html = DOMPurify.sanitize(html, { USE_PROFILES: { html: true } })
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
