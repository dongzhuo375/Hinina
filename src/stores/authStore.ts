import { defineStore } from 'pinia'
import type { User } from '@/types/user'
import { authService } from '@/services/auth.service'
import { clearDomainState } from '@/stores/session'

export const useAuthStore = defineStore('auth', {
  state: () => ({
    user: null as User | null,
    isLoading: false,
    error: null as string | null,
    /**
     * 是否已与后端确认过会话状态（登录、登出、会话恢复均视为已确认）。
     * 路由守卫据此决定是否需要发起 `get_session`，避免每次导航重复 IPC。
     */
    sessionResolved: false,
  }),

  getters: {
    isLoggedIn: (state) => state.user !== null,
    username: (state) => state.user?.username ?? null,
  },

  actions: {
    /** 登录并更新状态 */
    async login(username: string, password: string, ojType?: string) {
      this.isLoading = true
      this.error = null
      try {
        this.user = await authService.login(username, password, ojType)
        this.sessionResolved = true
      } catch (e) {
        this.error = e instanceof Error ? e.message : '登录失败'
        throw e
      } finally {
        this.isLoading = false
      }
    },

    /**
     * 登出：清理后端会话、本地认证态与所有会话级领域状态。
     *
     * 本 action 不会 reject —— 即便后端登出失败（IPC/网络异常），本地也一律视为已登出，
     * 保证 UI 不会停留在需要认证的页面。失败时 `sessionResolved` 复位为 false，
     * 使下一次进入受保护路由时重新向后端校验会话。
     */
    async logout() {
      this.isLoading = true
      this.error = null
      let backendCleared = true
      try {
        await authService.logout()
      } catch (e) {
        backendCleared = false
        this.error = e instanceof Error ? e.message : '登出失败'
        console.error('[authStore] 后端登出失败，已强制清理本地会话:', e)
      }
      this.user = null
      this.sessionResolved = backendCleared
      clearDomainState()
      this.isLoading = false
    },

    /** 检查后端会话有效性 */
    async checkSession(): Promise<boolean> {
      this.isLoading = true
      this.error = null
      try {
        const user = await authService.checkSession()
        this.user = user
        return user !== null
      } catch (e) {
        this.error = e instanceof Error ? e.message : '会话检查失败'
        return false
      } finally {
        this.sessionResolved = true
        this.isLoading = false
      }
    },
  },
})
