/// 错误 → 可直接展示的文案（前端唯一入口）。
///
/// 前端错误有三种来源：IPC（`IpcError`，已由 `bridge/ipcInvoke` 在唯一出口归一化）、
/// 运行时异常（`Error`）、以及其它意外载荷（字符串等）。统一在此收敛，
/// 各处不再重复 `e instanceof Error ? e.message : '…'` 的判空与兜底。
///
/// 用法：`this.error = errorMessage(e, '加载题目失败')`
export function errorMessage(e: unknown, fallback: string): string {
  if (e instanceof Error) return e.message.trim() || fallback
  // 非 Error 载荷（Tauri 之外的异常路径）只有字符串可能带信息
  if (typeof e === 'string') return e.trim() || fallback
  return fallback
}
