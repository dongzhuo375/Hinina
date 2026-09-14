import { invoke } from '@tauri-apps/api/core'

/**
 * IPC 调用失败时抛出的统一错误类型。
 *
 * Rust 端返回 `AppError`（serde 外部标签枚举，形如 `{ "Auth": "登录失败: ..." }`），
 * Tauri 会以该对象 reject —— 既不是 `Error` 也不是字符串。若原样透传，
 * 上层 `e instanceof Error` 判定全部落空，用户只能看到兜底文案而丢失真实原因。
 * 因此在唯一的 IPC 出口归一化为 `Error`，并保留原始载荷便于排查。
 */
export class IpcError extends Error {
  constructor(
    readonly cmd: string,
    message: string,
    readonly raw?: unknown,
  ) {
    super(message)
    this.name = 'IpcError'
  }
}

/** 将后端错误载荷归一化为可读消息 */
function toMessage(raw: unknown): string {
  if (raw instanceof Error) return raw.message
  if (typeof raw === 'string') return raw
  if (raw && typeof raw === 'object') {
    const entries = Object.entries(raw as Record<string, unknown>)
    // AppError 外部标签形式：{ Variant: message }
    if (entries.length === 1 && typeof entries[0][1] === 'string') return entries[0][1]
    const message = (raw as { message?: unknown }).message
    if (typeof message === 'string') return message
    try {
      return JSON.stringify(raw)
    } catch {
      return String(raw)
    }
  }
  return String(raw)
}

/**
 * 统一 IPC 调用封装：透传参数、归一化错误、单点日志。
 *
 * 安全约束：日志只记录命令名与错误消息，**绝不记录 args** ——
 * `login` 等命令的参数含明文密码（见开发手册 5.1 安全性要求）。
 */
export async function ipcInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args)
  } catch (raw) {
    const message = toMessage(raw)
    console.error(`[ipc] ${cmd} 调用失败: ${message}`)
    throw new IpcError(cmd, message, raw)
  }
}
