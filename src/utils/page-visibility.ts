/**
 * 页面可见性判据（榜单/公告/评测列表轮询共用的暂停条件）。
 *
 * 无 `document` 的环境（jsdom 部分用例/SSR）按可见处理——不因环境缺失
 * 而永久暂停轮询。
 */
export function isPageHidden(): boolean {
  return typeof document !== 'undefined' && document.hidden
}
