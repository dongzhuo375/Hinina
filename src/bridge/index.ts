import { invoke } from '@tauri-apps/api/core'

/**
 * Rust `core::error::AppError` 的变体名（serde 外部标签）。
 *
 * 前端据此按错误类别分流（如 `Auth` → 会话失效处理）。
 * Rust 端新增变体时此列表会滞后，未识别的变体归为 `null`，仅退化为通用错误处理。
 */
export type AppErrorVariant =
  | 'Auth'
  | 'Contest'
  | 'Problem'
  | 'Submission'
  | 'Workspace'
  | 'Io'
  | 'Network'
  | 'Config'
  | 'ProviderNotFound'
  | 'Serialization'
  | 'Unknown'

const APP_ERROR_VARIANTS: readonly string[] = [
  'Auth',
  'Contest',
  'Problem',
  'Submission',
  'Workspace',
  'Io',
  'Network',
  'Config',
  'ProviderNotFound',
  'Serialization',
  'Unknown',
]

/**
 * IPC 调用失败时抛出的统一错误类型。
 *
 * Rust 端返回 `AppError`（serde 外部标签枚举，形如 `{ "Auth": "登录失败: ..." }`），
 * Tauri 会以该对象 reject —— 既不是 `Error` 也不是字符串。若原样透传，
 * 上层 `e instanceof Error` 判定全部落空，用户只能看到兜底文案而丢失真实原因。
 * 因此在唯一的 IPC 出口归一化为 `Error`，并保留变体与原始载荷便于分流与排查。
 */
export class IpcError extends Error {
  constructor(
    readonly cmd: string,
    message: string,
    /// AppError 变体名；非 AppError 载荷（前端异常等）为 null
    readonly variant: AppErrorVariant | null,
    readonly raw?: unknown,
  ) {
    super(message)
    this.name = 'IpcError'
  }

  /** 是否认证类错误（凭证无效 / 会话被服务端撤销） */
  get isAuthError(): boolean {
    return this.variant === 'Auth'
  }
}

/** 解析 AppError 载荷 → { 变体名, 消息 }；非 AppError 形态返回 null 变体 */
function parseAppError(raw: unknown): { variant: AppErrorVariant | null; message: string } {
  if (raw instanceof Error) return { variant: null, message: raw.message }
  if (typeof raw === 'string') return { variant: null, message: raw }
  if (raw && typeof raw === 'object') {
    const entries = Object.entries(raw as Record<string, unknown>)
    // AppError 外部标签形式：{ Variant: message }
    if (entries.length === 1 && typeof entries[0][1] === 'string') {
      const [key, message] = entries[0]
      const variant = APP_ERROR_VARIANTS.includes(key) ? (key as AppErrorVariant) : null
      return { variant, message }
    }
    const message = (raw as { message?: unknown }).message
    if (typeof message === 'string') return { variant: null, message }
    try {
      return { variant: null, message: JSON.stringify(raw) }
    } catch {
      return { variant: null, message: String(raw) }
    }
  }
  return { variant: null, message: String(raw) }
}

/**
 * IPC 错误观察者。
 *
 * 由组合根（`main.ts`）注入，用于全局会话守卫等横切处理。
 * Bridge 层只负责回调，不感知 store / router，避免底层反向依赖上层。
 */
export type IpcErrorObserver = (error: IpcError) => void

let errorObserver: IpcErrorObserver | null = null

/** 注册（或传 null 注销）IPC 错误观察者，仅应在组合根调用一次 */
export function setIpcErrorObserver(observer: IpcErrorObserver | null): void {
  errorObserver = observer
}

/**
 * 统一 IPC 调用封装：透传参数、归一化错误、单点日志、通知观察者。
 *
 * 安全约束：日志只记录命令名与错误消息，**绝不记录 args** ——
 * `login` 等命令的参数含明文密码（见开发手册 5.1 安全性要求）。
 */
export async function ipcInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args)
  } catch (raw) {
    const { variant, message } = parseAppError(raw)
    const error = new IpcError(cmd, message, variant, raw)
    console.error(`[ipc] ${cmd} 调用失败${variant ? ` (${variant})` : ''}: ${message}`)
    errorObserver?.(error)
    throw error
  }
}
