import { getCurrentWindow } from '@tauri-apps/api/window'

/**
 * 无边框窗口控制 IPC 薄封装（最小化 / 最大化 / 关闭 / 销毁 / 关闭请求订阅）。
 *
 * 分层约定：`bridge/*.bridge.ts` 是唯一接触 `@tauri-apps/api` 的地方——任何
 * 层都不直接 import 本模块，经 `systemService` 消费、由服务转发。
 *
 * 窗口操作失败没有用户可执行的补救动作，调用方一律忽略结果。
 */
export async function minimizeWindow(): Promise<void> {
  await getCurrentWindow().minimize()
}

export async function toggleMaximizeWindow(): Promise<void> {
  await getCurrentWindow().toggleMaximize()
}

export async function closeWindow(): Promise<void> {
  await getCurrentWindow().close()
}

/** 销毁窗口（close-guard 落盘完成后的收尾动作，不走 close-requested 往返） */
export async function destroyWindow(): Promise<void> {
  await getCurrentWindow().destroy()
}

/**
 * 订阅关闭请求（close-guard 关窗握手的事件源）。
 *
 * 返回取消订阅的 promise（失败即「钩子没挂上」，调用方记日志）。
 * **async 是契约的一部分**：`getCurrentWindow()` 可能同步抛错（非 Tauri 上下文），
 * 只有 async 包装才能把它变成 rejected promise，让调用方的 `.catch` 接住；
 * 裸返回 promise 的写法会让同步异常绕过 `.catch` 逃出组合根。
 */
export async function onCloseRequested(
  handler: (event: { preventDefault(): void }) => void,
): Promise<() => void> {
  return getCurrentWindow().onCloseRequested(handler)
}
