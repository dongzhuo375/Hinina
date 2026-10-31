// system.service 的启动标志管线：单飞 IPC、快照填充、失败退化。
// 守卫用例（launch-redirect.spec）把本服务整体 mock 掉，测到的只是守卫的第二道
// try/catch —— 这里锁主防线：.catch 退化与「只发一次 IPC」的单飞语义。
import { beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({ getLaunchFlags: vi.fn() }))
vi.mock('@/bridge/system.bridge', () => ({ getLaunchFlags: mocks.getLaunchFlags }))

// 模块级 memoize + 快照是进程状态：resetModules 让每个用例拿到全新实例
beforeEach(() => {
  vi.resetModules()
  mocks.getLaunchFlags.mockReset()
})

async function fresh() {
  return (await import('@/services/system.service')).systemService
}

describe('getLaunchFlags 单飞与快照', () => {
  it('并发与先后多次调用只发一次 IPC，并填充同步快照', async () => {
    mocks.getLaunchFlags.mockResolvedValue({ settings: true })
    const svc = await fresh()
    const [a, b] = await Promise.all([svc.getLaunchFlags(), svc.getLaunchFlags()])
    expect(a).toEqual({ settings: true })
    expect(b).toEqual({ settings: true })
    expect(mocks.getLaunchFlags).toHaveBeenCalledTimes(1)
    expect(svc.launchFlags.value.settings).toBe(true)
    // 之后的调用走 memo：不再发 IPC
    await svc.getLaunchFlags()
    expect(mocks.getLaunchFlags).toHaveBeenCalledTimes(1)
  })

  it('IPC 失败：按「无参数」退化，快照保持安全侧默认，且不再重试', async () => {
    mocks.getLaunchFlags.mockRejectedValue(new Error('invoke failed'))
    const svc = await fresh()
    await expect(svc.getLaunchFlags()).resolves.toEqual({ settings: false })
    expect(svc.launchFlags.value.settings).toBe(false)
    // 失败同样被 memoize：后续调用不重发 IPC（本进程内退化即定局，
    // 不做「失败后重试翻转锁态」——中途解锁比保持锁定更危险）
    await svc.getLaunchFlags()
    expect(mocks.getLaunchFlags).toHaveBeenCalledTimes(1)
  })
})
