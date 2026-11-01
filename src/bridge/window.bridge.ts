import { getCurrentWindow } from '@tauri-apps/api/window'

/**
 * 无边框窗口控制 IPC 薄封装（最小化 / 最大化 / 关闭）。
 *
 * 分层约定（CLAUDE.md）：`bridge/*.bridge.ts` 是唯一接触 `@tauri-apps/api` 的
 * 地方——组件只 import 本模块（评审十九 #8；`App.vue` / `main.ts` 的同类调用
 * 是更早期遗留，未在本次一并迁移）。
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
