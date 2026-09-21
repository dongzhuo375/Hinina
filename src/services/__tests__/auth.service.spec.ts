import { beforeEach, describe, expect, it, vi } from 'vitest'

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

const user: User = { id: 'u1', username: 'team01', token: 'tk' }

beforeEach(() => {
  // 静默被测代码的错误日志，保持测试输出干净（行为已由断言覆盖）
  vi.spyOn(console, 'error').mockImplementation(() => {})
})

describe('login', () => {
  it('成功后返回用户', async () => {
    bridge.login.mockResolvedValue(user)
    await expect(authService.login('team01', 'pin')).resolves.toEqual(user)
    // OJ 切换已与登录解耦（显式 switchOj）：login 只传凭据
    expect(bridge.login).toHaveBeenCalledWith('team01', 'pin')
  })

  it('失败时向上抛出', async () => {
    bridge.login.mockRejectedValue(new Error('登录失败: 密码错误'))
    await expect(authService.login('team01', 'bad')).rejects.toThrow('密码错误')
  })
})

describe('logout', () => {
  it('成功后不再做任何本地清理（前端不持有会话副本）', async () => {
    bridge.logout.mockResolvedValue(undefined)
    await expect(authService.logout()).resolves.toBeUndefined()
    expect(bridge.logout).toHaveBeenCalledTimes(1)
  })

  it('契约：后端登出失败时异常向上抛出，由调用方决定 UI 语义', async () => {
    bridge.logout.mockRejectedValue(new Error('ipc 故障'))
    await expect(authService.logout()).rejects.toThrow('ipc 故障')
  })
})

describe('checkSession', () => {
  it('后端有会话时返回用户', async () => {
    bridge.getSession.mockResolvedValue(user)
    await expect(authService.checkSession()).resolves.toEqual(user)
  })

  it('后端无会话时返回 null', async () => {
    bridge.getSession.mockResolvedValue(null)
    await expect(authService.checkSession()).resolves.toBeNull()
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
