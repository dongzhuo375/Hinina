import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

/// Service 层打桩：store 只依赖 service，测试不触达 IPC
const { authService } = vi.hoisted(() => ({
  authService: {
    login: vi.fn(),
    logout: vi.fn(),
    checkSession: vi.fn(),
    validateSession: vi.fn(),
  },
}))
vi.mock('@/services/auth.service', () => ({ authService }))
/// 工作区服务打桩：用于验证登出后防抖同步不再触达后端
const { workspaceService } = vi.hoisted(() => ({
  workspaceService: {
    loadWorkspace: vi.fn(),
    saveWorkspace: vi.fn().mockResolvedValue(undefined),
    updateWorkspaceFile: vi.fn().mockResolvedValue(undefined),
  },
}))
vi.mock('@/services/workspace.service', () => ({ workspaceService }))
/// 领域 store 的依赖链最终会 import Tauri API，一并打桩以保持测试环境纯净
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

import { SESSION_INVALID_MESSAGE, useAuthStore } from '@/stores/authStore'
import { useContestStore } from '@/stores/contestStore'
import { useSubmissionStore } from '@/stores/submissionStore'
import { useWorkspaceStore } from '@/stores/workspaceStore'
import type { Contest } from '@/types/contest'
import type { User } from '@/types/user'

const user: User = { id: 'u1', username: 'team01', token: 'tk' }
const contest: Contest = {
  id: '1',
  title: 'Test Contest',
  startTime: 1_000,
  endTime: 2_000,
  description: '',
  contestType: 0,
  status: 0,
  auth: 0,
  rankShowName: 'username',
  sealRank: false,
  sealRankTime: null,
  allowEndSubmit: false,
  oiRankScoreType: null,
}

/// 预置"上一位选手"的会话级领域状态，用于验证登出清理
function seedDomainState() {
  const contestStore = useContestStore()
  contestStore.contest = contest
  contestStore.problems = []
  // 匿名简报不属于会话数据，登出后必须保留
  contestStore.brief = contest
  useSubmissionStore().submissions.push({
    id: 's1',
    problemId: 'p1',
    status: 'Accepted',
    submittedAt: new Date().toISOString(),
  })
  useWorkspaceStore().code = '#include <bits/stdc++.h>'
}

beforeEach(() => {
  setActivePinia(createPinia())
  vi.spyOn(console, 'error').mockImplementation(() => {})
})

describe('login', () => {
  it('成功后写入用户并标记会话已确认', async () => {
    authService.login.mockResolvedValue(user)
    const auth = useAuthStore()

    await auth.login('team01', 'pin')

    expect(auth.user).toEqual(user)
    expect(auth.isLoggedIn).toBe(true)
    expect(auth.username).toBe('team01')
    expect(auth.sessionResolved).toBe(true)
    expect(auth.isLoading).toBe(false)
    expect(auth.error).toBeNull()
  })

  it('失败时向上抛出、记录原因，且不标记会话已确认', async () => {
    authService.login.mockRejectedValue(new Error('登录失败: 密码错误'))
    const auth = useAuthStore()

    await expect(auth.login('team01', 'bad')).rejects.toThrow('密码错误')
    expect(auth.user).toBeNull()
    expect(auth.error).toBe('登录失败: 密码错误')
    expect(auth.sessionResolved).toBe(false)
    expect(auth.isLoading).toBe(false)
  })
})

describe('logout', () => {
  it('成功后清理认证态与会话级领域状态，并保留匿名比赛简报', async () => {
    authService.logout.mockResolvedValue(undefined)
    const auth = useAuthStore()
    auth.user = user
    auth.sessionResolved = true
    seedDomainState()

    await auth.logout()

    expect(auth.user).toBeNull()
    expect(auth.isLoggedIn).toBe(false)
    expect(auth.sessionResolved).toBe(true)
    // 机位账号复用场景：上一位选手的数据不得残留
    expect(useContestStore().contest).toBeNull()
    expect(useSubmissionStore().submissions).toHaveLength(0)
    expect(useWorkspaceStore().code).toBe('')
    // 匿名简报与登录态无关，切换账号时右侧氛围区不应空白
    expect(useContestStore().brief).toEqual(contest)
  })

  it('契约：后端登出失败也不 reject，本地一律视为已登出', async () => {
    authService.logout.mockRejectedValue(new Error('ipc 故障'))
    const auth = useAuthStore()
    auth.user = user
    seedDomainState()

    await expect(auth.logout()).resolves.toBeUndefined()
    expect(auth.user).toBeNull()
    expect(auth.error).toBe('ipc 故障')
    // 后端状态未知 → 复位标记，下次进入受保护路由重新校验
    expect(auth.sessionResolved).toBe(false)
    expect(useSubmissionStore().submissions).toHaveLength(0)
  })

  it('清理前取消工作区防抖同步，避免登出后仍向后端写代码', async () => {
    vi.useFakeTimers()
    try {
      authService.logout.mockResolvedValue(undefined)
      const auth = useAuthStore()
      const workspace = useWorkspaceStore()
      auth.user = user

      workspace.updateCode('int main(){ return 0; }') // 排定 2s 防抖同步
      expect(workspace._syncTimer).not.toBeNull()

      await auth.logout()
      expect(workspace._syncTimer).toBeNull()

      // 即使时间推进，也不应再向后端同步任何代码
      vi.advanceTimersByTime(5_000)
      expect(workspaceService.updateWorkspaceFile).not.toHaveBeenCalled()
    } finally {
      vi.useRealTimers()
    }
  })
})

describe('checkSession', () => {
  it('后端有会话时恢复登录态', async () => {
    authService.checkSession.mockResolvedValue(user)
    const auth = useAuthStore()

    await expect(auth.checkSession()).resolves.toBe(true)
    expect(auth.isLoggedIn).toBe(true)
    expect(auth.sessionResolved).toBe(true)
  })

  it('后端无会话时返回 false 但仍标记已确认（守卫无需重复 IPC）', async () => {
    authService.checkSession.mockResolvedValue(null)
    const auth = useAuthStore()

    await expect(auth.checkSession()).resolves.toBe(false)
    expect(auth.isLoggedIn).toBe(false)
    expect(auth.sessionResolved).toBe(true)
  })

  it('异常时返回 false 且不抛出', async () => {
    authService.checkSession.mockRejectedValue(new Error('boom'))
    const auth = useAuthStore()

    await expect(auth.checkSession()).resolves.toBe(false)
    expect(auth.error).toBe('boom')
    expect(auth.sessionResolved).toBe(true)
  })
})

describe('validateSession', () => {
  it('valid：保持登录态不变', async () => {
    authService.validateSession.mockResolvedValue('valid')
    const auth = useAuthStore()
    auth.user = user
    auth.sessionResolved = true

    await expect(auth.validateSession()).resolves.toBe('valid')
    expect(auth.isLoggedIn).toBe(true)
    expect(auth.error).toBeNull()
  })

  it('契约：unknown（网络异常）必须保留登录态，不得把选手踢回登录页', async () => {
    authService.validateSession.mockResolvedValue('unknown')
    const auth = useAuthStore()
    auth.user = user
    auth.sessionResolved = true

    await expect(auth.validateSession()).resolves.toBe('unknown')
    expect(auth.isLoggedIn).toBe(true)
    expect(auth.sessionResolved).toBe(true)
    expect(auth.error).toBeNull()
  })

  it('invalid：清理会话、写入失效原因并复位 sessionResolved', async () => {
    authService.logout.mockResolvedValue(undefined)
    authService.validateSession.mockResolvedValue('invalid')
    const auth = useAuthStore()
    auth.user = user
    auth.sessionResolved = true
    seedDomainState()

    await expect(auth.validateSession()).resolves.toBe('invalid')
    expect(auth.isLoggedIn).toBe(false)
    expect(auth.error).toBe(SESSION_INVALID_MESSAGE)
    expect(auth.sessionResolved).toBe(false)
    expect(useContestStore().contest).toBeNull()
  })

  it('未登录时 invalid 不触发清理，也不覆盖已有错误信息', async () => {
    authService.validateSession.mockResolvedValue('invalid')
    const auth = useAuthStore()
    auth.error = '登录失败: 密码错误'

    await expect(auth.validateSession()).resolves.toBe('invalid')
    expect(auth.error).toBe('登录失败: 密码错误')
    expect(authService.logout).not.toHaveBeenCalled()
  })
})

describe('invalidateSession', () => {
  it('以指定原因清理会话（全局会话守卫使用）', async () => {
    authService.logout.mockResolvedValue(undefined)
    const auth = useAuthStore()
    auth.user = user
    auth.sessionResolved = true

    await auth.invalidateSession('服务端已撤销凭证')

    expect(auth.isLoggedIn).toBe(false)
    expect(auth.error).toBe('服务端已撤销凭证')
    expect(auth.sessionResolved).toBe(false)
  })
})
