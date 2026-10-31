import { ipcInvoke } from '@/bridge'

/// 前端日志级别（与 Rust 端 `frontend_log` 的白名单一致；未知级别后端按 warn 落）
export type FrontendLogLevel = 'debug' | 'info' | 'warn' | 'error'

/**
 * 把一条前端日志交给 Rust `tracing`（`frontend_log` 命令 → stderr + `logs/hinina.log`）。
 *
 * 这是前端世界的**唯一**持久化日志通道：`utils/logger` 只写 console，全局错误
 * 守卫（`guards/globalErrorGuard`）经本函数把 Vue / window / promise / router
 * 四类全局错误送进与 Rust panic 同一份日志文件。
 *
 * 安全约束与全链一致：只传 level / scope / message 三个标量，**不传 payload**；
 * 分工：scope 白名单、控制字符剥离与最终长度上限都在 Rust 端收敛；前端的
 * 2000 字符预截断不是重复实现，是 **IPC 载荷大小预算** —— 避免把超大 payload
 * （如被序列化进错误消息的对象）整段送过 IPC。
 *
 * 失败语义：返回的 Promise 可能拒绝（后端不可用、日志被 `RUST_LOG` 关闭不会导致
 * 失败但会静默）。调用方（全局守卫）必须吞掉拒绝 —— 转发是尽力而为，绝不允许
 * 「报错的通道自己抛错」。
 */
export async function forwardFrontendLog(
  level: FrontendLogLevel,
  scope: string,
  message: string,
): Promise<void> {
  return ipcInvoke<void>('frontend_log', { level, scope, message })
}
