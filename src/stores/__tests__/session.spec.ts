import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

/// Service 层打桩：logout / checkSession 打桩用于断言「OJ 切换不走后端登出、不做会话检查」
const { authService } = vi.hoisted(() => ({
  authService: {
    login: vi.fn(),
    logout: vi.fn(),
    checkSession: vi.fn(),
    validateSession: vi.fn(),
  },
}))
vi.mock('@/services/auth.service', () => ({ authService }))
/// 领域 store 的依赖链最终会 import Tauri API，一并打桩以保持测试环境纯净
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

import { resetSessionForOjSwitch } from '@/stores/session'
import { useAuthStore } from '@/stores/authStore'
import { useContestStore } from '@/stores/contestStore'
import { useProblemStore } from '@/stores/problemStore'
import { useSubmissionStore } from '@/stores/submissionStore'
import type { Contest } from '@/types/contest'
import type { User } from '@/types/user'

const user: User = { id: 'u1', username: 'team01', token: 'tk-old-oj' }
const contest: Contest = {
  id: '1',
  title: 'Old OJ Contest',
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

/// 预置「旧 OJ」的会话与领域状态，用于验证切换清理
function seedOldOjState() {
  const auth = useAuthStore()
  auth.user = user
  auth.sessionResolved = true

  const contestStore = useContestStore()
  contestStore.contest = contest
  contestStore.brief = contest // 匿名简报：登录页挂载时会按新 OJ 重拉

  const problem = useProblemStore()
  problem.myStatus = { p1: 1 }
  problem.myStatusStale = false
  problem.myStatusContestId = '1'

  useSubmissionStore().submissions.push({
    id: 's1',
    problemId: 'p1',
    status: 'Accepted',
    submittedAt: '2026-09-18T00:00:00Z',
  })
}

describe('resetSessionForOjSwitch — OJ 切换的会话上下文重置', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.clearAllMocks()
    seedOldOjState()
  })

  it('清空认证态并强制下次导航重新恢复会话（sessionResolved = false）', () => {
    resetSessionForOjSwitch()

    const auth = useAuthStore()
    expect(auth.user).toBeNull()
    expect(auth.error).toBeNull()
    expect(auth.sessionResolved).toBe(false)
  })

  it('清空旧 OJ 的领域状态（比赛/题目状态/提交记录）', () => {
    resetSessionForOjSwitch()

    const contestStore = useContestStore()
    expect(contestStore.contest).toBeNull()
    expect(contestStore.problems).toEqual([])
    // 匿名简报属于旧 OJ（换服务端）：一并复位 —— 否则 LoginView 的 canEnter
    // 侦听器会按旧 OJ 的时间窗计算阶段，在新 OJ 有会话时立即自动推进赛场
    expect(contestStore.brief).toBeNull()
    expect(contestStore.briefBaseUrl).toBe('')
    expect(contestStore.briefError).toBeNull()
    expect(contestStore.briefState).toBe('idle')

    const problem = useProblemStore()
    expect(problem.myStatus).toEqual({})
    expect(problem.myStatusStale).toBe(true)
    expect(problem.myStatusContestId).toBeNull()

    expect(useSubmissionStore().submissions).toEqual([])
  })

  it('不打后端 logout / checkSession：Registry 已切换，logout 会误删新 OJ 的会话文件', () => {
    resetSessionForOjSwitch()

    expect(authService.logout).not.toHaveBeenCalled()
    expect(authService.checkSession).not.toHaveBeenCalled()
    expect(authService.login).not.toHaveBeenCalled()
    expect(authService.validateSession).not.toHaveBeenCalled()
  })
})
