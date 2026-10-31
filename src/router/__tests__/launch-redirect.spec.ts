// @vitest-environment jsdom
// --settings 管理方钥匙的**守卫行为**测试（区别于 index.spec.ts 的 meta 合并矩阵）：
// firstNavigation 只触发一次、仅落登录页时直落、IPC 失败按「无参数」退化。
// 「每次导航到 Login 都重定向」会把维护人员困死在设置页（返回登录失效）——
// 这类回归在真实 router 上锁定（沿 contest-access.spec 的 mock 模式）。
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useAuthStore } from '@/stores/authStore'

vi.mock('vue-router', async (importOriginal) => {
  const actual = await importOriginal<typeof import('vue-router')>()
  return { ...actual, createWebHistory: actual.createMemoryHistory }
})
vi.mock('@/views/LoginView.vue', () => ({ default: { template: '<div />' } }))
vi.mock('@/views/ContestLayout.vue', () => ({ default: { template: '<div />' } }))
vi.mock('@/views/SettingsView.vue', () => ({ default: { template: '<div />' } }))
vi.mock('@/views/ProblemSetView.vue', () => ({ default: { template: '<div />' } }))

const launchFlags = vi.hoisted(() => ({ mock: vi.fn() }))
vi.mock('@/services/system.service', () => ({
  systemService: { getLaunchFlags: launchFlags.mock },
}))

beforeEach(() => {
  // firstNavigation 是 router 模块的模块级状态：resetModules 让每个用例拿到
  // 全新 router 实例（首次导航语义才可独立断言）
  vi.resetModules()
  launchFlags.mock.mockReset()
  setActivePinia(createPinia())
  // 会话已判定但不存在（--settings 的目标形态）；login 不受保护，守卫不拦
  const auth = useAuthStore()
  auth.sessionResolved = true
  auth.user = null
})

describe('--settings 初始直落（守卫行为）', () => {
  it('带参首次导航：登录页直落设置页', async () => {
    launchFlags.mock.mockResolvedValue({ settings: true })
    const { default: router } = await import('@/router')
    await router.push({ name: 'Login' })
    expect(router.currentRoute.value.name).toBe('Settings')
  })

  it('仅首次生效：再次前往登录页不被重定向（返回登录可用，不困死维护人员）', async () => {
    launchFlags.mock.mockResolvedValue({ settings: true })
    const { default: router } = await import('@/router')
    await router.push({ name: 'Login' })
    expect(router.currentRoute.value.name).toBe('Settings')
    await router.push({ name: 'Login' })
    expect(router.currentRoute.value.name).toBe('Login')
  })

  it('首次导航不是登录页时不直落（深链照常走会话门禁）', async () => {
    launchFlags.mock.mockResolvedValue({ settings: true })
    const { default: router } = await import('@/router')
    await router.push({ name: 'ProblemSet' })
    // 无会话：受保护路由按原守卫回登录页，而不是被钥匙带去设置页
    expect(router.currentRoute.value.name).toBe('Login')
  })

  it('flags IPC 失败按「无参数」退化：首航停留登录页，不炸守卫', async () => {
    launchFlags.mock.mockRejectedValue(new Error('invoke failed'))
    const { default: router } = await import('@/router')
    await router.push({ name: 'Login' })
    expect(router.currentRoute.value.name).toBe('Login')
  })

  it('无参启动：不重定向（正常门禁形态）', async () => {
    launchFlags.mock.mockResolvedValue({ settings: false })
    const { default: router } = await import('@/router')
    await router.push({ name: 'Login' })
    expect(router.currentRoute.value.name).toBe('Login')
  })

  it('无钥匙 + 无会话直达设置页：回登录页（meta 继承保护，豁免仅钥匙模式）', async () => {
    // 评审 P2 路径一：无条件豁免会让无钥匙访客直达设置页
    launchFlags.mock.mockResolvedValue({ settings: false })
    const { default: router } = await import('@/router')
    await router.push({ name: 'Settings' })
    expect(router.currentRoute.value.name).toBe('Login')
  })

  it('钥匙模式 + 无会话非首航直达设置页：放行（豁免不限首次）', async () => {
    // 首航去别处后，--settings 进程里设置页仍可无会话直达（返回登录后想再修
    // 一次地址的场景）
    launchFlags.mock.mockResolvedValue({ settings: true })
    const { default: router } = await import('@/router')
    await router.push({ name: 'Login' })
    expect(router.currentRoute.value.name).toBe('Settings')
    await router.push({ name: 'Login' })
    await router.push({ name: 'Settings' })
    expect(router.currentRoute.value.name).toBe('Settings')
  })
})
