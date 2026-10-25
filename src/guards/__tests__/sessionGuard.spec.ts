// sessionGuard 并发会话失效处理（M2 工作包 C）：
// 并发认证错误只清理一次、未登录不跳转、非认证错误不误处理、
// 受保护页面才回登录页。此前该守卫零测试（M0 盘点：guards 0/1）。
import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { useAuthStore, SESSION_INVALID_MESSAGE } from '@/stores/authStore'

const mocks = vi.hoisted(() => ({
  setIpcErrorObserver: vi.fn(),
  logout: vi.fn(),
  replace: vi.fn().mockResolvedValue(undefined),
}))

vi.mock('@/bridge', () => {
  class IpcError extends Error {
    isAuthError: boolean
    constructor(message: string, isAuthError: boolean) {
      super(message)
      this.isAuthError = isAuthError
    }
  }
  return { setIpcErrorObserver: mocks.setIpcErrorObserver, IpcError }
})
vi.mock('@/services/auth.service', () => ({ authService: { logout: mocks.logout } }))

import { installSessionGuard } from '@/guards/sessionGuard'

interface GuardHarness {
  auth: ReturnType<typeof useAuthStore>
  observer: (error: { isAuthError: boolean; message: string }) => void
}

/// 装配守卫并取回注入 bridge 的观察者。`requiresAuth` 决定当前路由形态。
function install(requiresAuth: boolean, loggedIn = true): GuardHarness {
  setActivePinia(createPinia())
  const auth = useAuthStore()
  if (loggedIn) auth.user = { id: 'u1', username: 'alice', token: 't' } as never
  const router = {
    currentRoute: { value: { meta: { requiresAuth } } },
    replace: mocks.replace,
  }
  installSessionGuard(router as never)
  const observer = mocks.setIpcErrorObserver.mock.calls.at(-1)?.[0] as GuardHarness['observer']
  expect(observer).toBeTypeOf('function')
  return { auth, observer }
}

const authError = () => ({ isAuthError: true, message: '401' })
const otherError = () => ({ isAuthError: false, message: '500' })

/// 观察者是 fire-and-forget（内部 void handleAuthFailure）；
/// 落盘链路要跨多个微任务，统一沉淀后再断言。
const settle = async () => {
  for (let i = 0; i < 12; i += 1) await Promise.resolve()
}

beforeEach(() => {
  vi.clearAllMocks()
  mocks.replace.mockResolvedValue(undefined)
})

describe('sessionGuard 并发会话失效', () => {
  it('并发认证错误只清理一次（防重入）', async () => {
    let releaseLogout!: () => void
    mocks.logout.mockReturnValue(
      new Promise<void>(resolve => { releaseLogout = resolve }),
    )
    const { auth, observer } = install(true)

    observer(authError())
    observer(authError())
    observer(authError())
    await Promise.resolve()
    expect(mocks.logout).toHaveBeenCalledTimes(1) // in-flight 期间的重复错误必须被 invalidating 挡下

    releaseLogout()
    await settle()
    expect(auth.user).toBeNull()
    expect(auth.error).toBe(SESSION_INVALID_MESSAGE)
    expect(mocks.replace).toHaveBeenCalledTimes(1)
  })

  it('未登录时的认证错误（如登录表单密码错误）不触发清理与跳转', async () => {
    const { observer } = install(true, false)

    observer(authError())
    await settle()

    expect(mocks.logout).not.toHaveBeenCalled()
    expect(mocks.replace).not.toHaveBeenCalled()
  })

  it('非认证错误不误处理', async () => {
    const { auth, observer } = install(true)

    observer(otherError())
    await settle()

    expect(auth.user).not.toBeNull() // 网络错误不得把选手踢回登录页
    expect(mocks.replace).not.toHaveBeenCalled()
  })

  it('仅当前路由要求认证时才跳登录页', async () => {
    mocks.logout.mockResolvedValue(undefined)
    const onPublic = install(false)
    onPublic.observer(authError())
    await settle()
    expect(mocks.replace).not.toHaveBeenCalled() // 公开页不是失效落点，无需跳转

    const onProtected = install(true)
    onProtected.observer(authError())
    await settle()
    expect(mocks.replace).toHaveBeenCalledWith({ name: 'Login' })
  })

  it('失效完成后的再次认证错误：已登出，直接短路', async () => {
    mocks.logout.mockResolvedValue(undefined)
    const { observer } = install(true)

    observer(authError())
    await settle()
    observer(authError())
    await settle()

    expect(mocks.logout).toHaveBeenCalledTimes(1) // 第二波到达时 isLoggedIn 已为 false
  })
})
