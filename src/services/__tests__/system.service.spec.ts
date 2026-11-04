// system.service 的启动标志管线：单飞 IPC、快照填充、失败退化。
// 守卫用例（launch-redirect.spec）把本服务整体 mock 掉，测到的只是守卫的第二道
// try/catch —— 这里锁主防线：.catch 退化、成功只发一次 IPC、失败后下次调用重试。
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

  it('IPC 失败：本次按「无参数」退化，但不缓存失败——下次调用重试并填充快照', async () => {
    // 一次瞬时失败不该让钥匙整个进程失效
    mocks.getLaunchFlags.mockRejectedValueOnce(new Error('invoke failed'))
    const svc = await fresh()
    await expect(svc.getLaunchFlags()).resolves.toEqual({ settings: false })
    expect(svc.launchFlags.value.settings).toBe(false)

    // 下一次调用重新发 IPC；成功后快照更新为真值
    mocks.getLaunchFlags.mockResolvedValueOnce({ settings: true })
    await expect(svc.getLaunchFlags()).resolves.toEqual({ settings: true })
    expect(mocks.getLaunchFlags).toHaveBeenCalledTimes(2)
    expect(svc.launchFlags.value.settings).toBe(true)
  })
})
