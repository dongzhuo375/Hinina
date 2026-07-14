import { invoke } from '@tauri-apps/api/core'

/**
 * 统一 IPC 调用封装，处理 Rust AppError 错误。
 * Tauri invoke 在 Rust 返回 Err 时会 throw，此处透传。
 */
export async function ipcInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(cmd, args)
}
