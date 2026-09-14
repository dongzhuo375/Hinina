import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

/// Bridge 层打桩：service 只依赖 bridge，不需要真实 Tauri 运行时
const { bridge } = vi.hoisted(() => ({
  bridge: {
    login: vi.fn(),
    logout: vi.fn(),
    getSession: vi.fn(),
    validateSession: vi.fn(),
  },
}))
vi.mock('@/bridge/auth.bridge', () => bridge)

import { authService } from '@/services/auth.service'
import type { User } from '@/types/user'

const STORED_KEY = 'hinina_user'
const user: User = { id: 'u1', username: 'team01', token: 'tk' }

/// 内存版 localStorage：node 环境无 DOM，手写桩即可，不必引入 jsdom
let backing: Map<string, string>

beforeEach(() => {
  backing = new Map()
  vi.stubGlobal('localStorage', {
    getItem: (key: string) => backing.get(key) ?? null,
    setItem: (key: string, value: string) => void backing.set(key, value),
    removeItem: (key: string) => void backing.delete(key),
    clear: () => backing.clear(),
  })
  // 静默被测代码的错误日志，保持测试输出干净（行为已由断言覆盖）
  vi.spyOn(console, 'error').mockImplementation(() => {})
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('login', () => {
  it('成功后返回用户并写入本地缓存', async () => {
    bridge.login.mockResolvedValue(user)
    await expect(authService.login('team01', 'pin')).resolves.toEqual(user)
    expect(bridge.login).toHaveBeenCalledWith('team01', 'pin', undefined)
    expect(backing.get(STORED_KEY)).toBe(JSON.stringify(user))
  })

  it('失败时不写缓存并向上抛出', async () => {
    bridge.login.mockRejectedValue(new Error('登录失败: 密码错误'))
    await expect(authService.login('team01', 'bad')).rejects.toThrow('密码错误')
    expect(backing.has(STORED_KEY)).toBe(false)
  })
})

describe('logout', () => {
  it('成功后清除本地缓存', async () => {
    backing.set(STORED_KEY, JSON.stringify(user))
    bridge.logout.mockResolvedValue(undefined)
    await authService.logout()
    expect(backing.has(STORED_KEY)).toBe(false)
  })

  it('契约：后端登出失败也必须清除本地缓存，且异常向上抛出', async () => {
    // 本地状态必须与"已登出"的 UI 语义一致，否则重启后仍会回注死 token
    backing.set(STORED_KEY, JSON.stringify(user))
    bridge.logout.mockRejectedValue(new Error('ipc 故障'))
    await expect(authService.logout()).rejects.toThrow('ipc 故障')
    expect(backing.has(STORED_KEY)).toBe(false)
  })
})

describe('checkSession', () => {
  it('后端有会话时返回用户并刷新缓存', async () => {
    bridge.getSession.mockResolvedValue(user)
    await expect(authService.checkSession()).resolves.toEqual(user)
    expect(backing.get(STORED_KEY)).toBe(JSON.stringify(user))
  })

  it('后端无会话时返回 null 且不写缓存', async () => {
    bridge.getSession.mockResolvedValue(null)
    await expect(authService.checkSession()).resolves.toBeNull()
    expect(backing.has(STORED_KEY)).toBe(false)
  })

  it('IPC 异常时吞掉错误返回 null（启动路径不应因会话查询失败而中断）', async () => {
    bridge.getSession.mockRejectedValue(new Error('boom'))
    await expect(authService.checkSession()).resolves.toBeNull()
  })
})

describe('validateSession', () => {
  it('透传后端三态结果', async () => {
    for (const validity of ['valid', 'invalid', 'unknown'] as const) {
      bridge.validateSession.mockResolvedValue(validity)
      await expect(authService.validateSession()).resolves.toBe(validity)
    }
  })

  it('契约：IPC 自身异常归一为 unknown，而不是 invalid', async () => {
    // 传输层故障若被当成"会话失效"，会在赛前把选手踢回登录页
    bridge.validateSession.mockRejectedValue(new Error('ipc 故障'))
    await expect(authService.validateSession()).resolves.toBe('unknown')
  })
})

describe('getStoredUser', () => {
  it('解析缓存的用户信息', () => {
    backing.set(STORED_KEY, JSON.stringify(user))
    expect(authService.getStoredUser()).toEqual(user)
  })

  it('缓存损坏时清除脏数据并返回 null', () => {
    backing.set(STORED_KEY, '{ not json')
    expect(authService.getStoredUser()).toBeNull()
    expect(backing.has(STORED_KEY)).toBe(false)
  })

  it('无缓存时返回 null', () => {
    expect(authService.getStoredUser()).toBeNull()
  })
})
