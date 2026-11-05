/**
 * 外链拦截：把题面/公告等 markdown 渲染面里的 http(s) 锚点转交系统默认浏览器，
 * WebView 永不导航离开应用（点外链把整个客户端替换掉，是分层审计 2026-10
 * 「外链无出路」缺口的危害形态）。
 *
 * 拦截规则（document 级委托，一次装配覆盖全部 v-html 消费面）：
 * - **左键（含 Ctrl/Meta/Shift/Alt 修饰）与中键**都转交：WebView 没有
 *   「新标签」语义，修饰键/中键的默认策略就是新窗口请求被 Tauri 拒绝——
 *   放过等于「点了没反应」，所以一并转交（`click` + `auxclick`）。
 * - 右键不拦（上下文菜单交给默认策略）。
 * - 只拦 `href` 解析后为 **http/https 且 host ≠ 当前 host** 的锚点——应用内
 *   路由（同 host）与 `#` 片段（解析后同 host，或非 http(s) 协议）自然落到
 *   「非外链」分支，不转交；规则只有一份，由 URL 解析统一判定。
 * - 已被前置处理器拦下的点击（`defaultPrevented`）不再重复处理。
 *
 * 转交通道由装配方注入（`main.ts` → `systemService.openExternal` → Rust
 * `open_external` 命令，scheme 白名单在 Rust 侧守卫）；本模块不做策略判断
 * 之外的事，注入即测试点。
 */

/**
 * 判定一个 href 解析后是否为待转交的外链，返回解析后的绝对 URL（非外链返回 null）。
 *
 * 判据只有这一条：`new URL(href, doc.location.href)` 后为 **http/https 且
 * host ≠ 文档 host**。拦截器（click/auxclick）与 markdown 加固钩子共用它，
 * 避免「什么算外链」两处实现漂移；`#` 片段解析后同 host（或非 http(s)）自然落选。
 */
export function resolveExternalUrl(href: string | null, doc: Document): URL | null {
  if (!href) return null;
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
  const intercept = (event: MouseEvent): void => {
    if (event.defaultPrevented) return;
    const target = event.target instanceof Element ? event.target : null;
    const anchor = target?.closest('a');
    if (!anchor) return;
    const resolved = resolveExternalUrl(anchor.getAttribute('href'), doc);
    if (!resolved) return;
    event.preventDefault();
    open(resolved.href);
  };
  const onClick = (event: MouseEvent): void => {
    // click 只对主键触发；中键/右键走 auxclick
    if (event.button !== 0) return;
    intercept(event);
  };
  const onAuxClick = (event: MouseEvent): void => {
    // 中键 = 在 WebView 里本应「开新标签」，没有该能力，转交系统浏览器；
    // 右键留给上下文菜单
    if (event.button !== 1) return;
    intercept(event);
  };
  doc.addEventListener('click', onClick);
  doc.addEventListener('auxclick', onAuxClick);
  return () => {
    doc.removeEventListener('click', onClick);
    doc.removeEventListener('auxclick', onAuxClick);
  };
}
