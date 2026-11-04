import { describe, expect, it, vi } from 'vitest'

/// 模拟非 Tauri 上下文：`getCurrentWindow()` 在取窗口元数据时**同步**抛错。
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => {
    throw new Error('tauri unavailable')
  },
}))

import { onCloseRequested } from '@/bridge/window.bridge'

describe('window.bridge.onCloseRequested', () => {
  /// 回归：裸返回 `getCurrentWindow().onCloseRequested(...)` 时同步异常会在
  /// `.catch` 挂上之前逃出组合根；async 包装必须把它变成 rejected promise。
  it('同步取窗口失败时返回 rejected promise，而不是同步抛错', async () => {
    let promise: Promise<() => void> | undefined
    expect(() => {
      promise = onCloseRequested(() => {})
    }).not.toThrow()

    await expect(promise).rejects.toThrow('tauri unavailable')
  })
})
