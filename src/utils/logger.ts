/// 前端作用域日志（唯一入口）。
///
/// 只做两件事：统一作用域前缀、按级别分流（`debug`/`info` 仅开发环境输出，
/// `warn`/`error` 始终输出）。**不做上报与落盘** —— 客户端的持久化日志由 Rust 侧
/// `tracing` 负责，前端 console 仅供开发与现场 DevTools 排障。
///
/// 安全约束与 `bridge/ipcInvoke` 一致：日志**绝不记录敏感参数**（登录密码等），
/// 只记录命令名、错误消息与业务标识。
///
/// 用法：`const log = createLogger('workspaceStore')` → `log.error('代码同步失败:', e)`

export interface Logger {
  debug(...args: unknown[]): void
  info(...args: unknown[]): void
  warn(...args: unknown[]): void
  error(...args: unknown[]): void
}

/// 开发环境判定（`vite build` 产物为 false，vitest 为 true）
const IS_DEV = import.meta.env.DEV

function noop(): void {}

/// 创建带作用域前缀的日志器。
///
/// 作用域名沿用模块名（`configService` / `workspaceStore` / `ipc` …），
/// 与既有日志文案保持一致，便于按模块过滤现场日志。
export function createLogger(scope: string): Logger {
  const prefix = `[${scope}]`
  return {
    debug: IS_DEV ? (...args: unknown[]) => console.debug(prefix, ...args) : noop,
    info: IS_DEV ? (...args: unknown[]) => console.info(prefix, ...args) : noop,
    // 保底信息：warn/error 是现场排障的唯一线索（用户不会开控制台），不可按环境关闭
    warn: (...args: unknown[]) => console.warn(prefix, ...args),
    error: (...args: unknown[]) => console.error(prefix, ...args),
  }
}
