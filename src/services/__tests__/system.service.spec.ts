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

describe('isKeyedSettings（唯一判据，评审十五 P1）', () => {
  it('钥匙模式 + Settings → true；其余组合一律 false', async () => {
    mocks.getLaunchFlags.mockResolvedValue({ settings: true })
    const svc = await fresh()
    await svc.getLaunchFlags() // 填充快照

    expect(svc.isKeyedSettings('Settings')).toBe(true)
    expect(svc.isKeyedSettings('ProblemSet')).toBe(false)
    expect(svc.isKeyedSettings('Login')).toBe(false)
    expect(svc.isKeyedSettings(undefined)).toBe(false)
    expect(svc.isKeyedSettings(null)).toBe(false)
    // 符号路由名（命名路由的 name 也可能是 symbol）不算 Settings
    expect(svc.isKeyedSettings(Symbol('Settings'))).toBe(false)
  })

  it('普通启动下即使是 Settings 也不豁免', async () => {
    mocks.getLaunchFlags.mockResolvedValue({ settings: false })
    const svc = await fresh()
    await svc.getLaunchFlags()
    expect(svc.isKeyedSettings('Settings')).toBe(false)
  })

  it('快照未填充（IPC 尚未返回）时按「无参数」处理', async () => {
    const svc = await fresh()
    expect(svc.isKeyedSettings('Settings')).toBe(false)
  })
})

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
    // 评审十九 #7：一次瞬时失败不该让钥匙整个进程失效
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
