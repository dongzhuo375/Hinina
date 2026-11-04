/**
 * 外链拦截：把题面/公告等 markdown 渲染面里的 http(s) 锚点转交系统默认浏览器，
 * WebView 永不导航离开应用（点外链把整个客户端替换掉，是分层审计 2026-10
 * 「外链无出路」缺口的危害形态）。
 *
 * 拦截规则（document 级委托，一次装配覆盖全部 v-html 消费面）：
 * - 只拦 `href` 解析后为 **http/https 且 host ≠ 当前 host** 的锚点——应用内
 *   路由（RouterLink，同 host）与 `#` 片段天然不受影响；
 * - 已被前置处理器拦下的点击（`defaultPrevented`）不再重复处理；
 * - 只拦左键无修饰键点击——Ctrl/中键等浏览器行为交给默认策略（Tauri 默认
 *   拒绝新窗口，结果是无动作而非劫持）。
 *
 * 转交通道由装配方注入（`main.ts` → `systemService.openExternal` → Rust
 * `open_external` 命令，scheme 白名单在 Rust 侧守卫）；本模块不做策略判断
 * 之外的事，注入即测试点。
 */

/** 判定一个锚点解析后是否为待转交的外链，返回解析后的绝对 URL（非外链返回 null）。 */
function resolveExternalHref(anchor: HTMLAnchorElement, doc: Document): URL | null {
  const href = anchor.getAttribute('href');
  if (!href || href.startsWith('#')) return null;
  let resolved: URL;
  try {
    resolved = new URL(href, doc.location.href);
  } catch {
    return null;
  }
  if (resolved.protocol !== 'http:' && resolved.protocol !== 'https:') return null;
  if (resolved.host === doc.location.host) return null;
  return resolved;
}

/**
 * 装配全局外链拦截。返回解绑函数（应用生命周期内不解除；测试用）。
 *
 * `open` 由调用方注入——失败时调用方自行决定兜底（`main.ts` 仅 log.warn，
 * 沿窗口控制「无补救动作」先例）。
 */
export function installExternalLinkOpener(
  open: (url: string) => void,
  doc: Document = document,
): () => void {
  const onClick = (event: MouseEvent): void => {
    if (event.defaultPrevented) return;
    if (event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) {
      return;
    }
    const target = event.target instanceof Element ? event.target : null;
    const anchor = target?.closest('a');
    if (!anchor) return;
    const resolved = resolveExternalHref(anchor, doc);
    if (!resolved) return;
    event.preventDefault();
    open(resolved.href);
  };
  doc.addEventListener('click', onClick);
  return () => doc.removeEventListener('click', onClick);
}
