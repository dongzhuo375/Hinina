import { defineStore } from 'pinia'
import type { User } from '@/types/user'
import { authService } from '@/services/auth.service'

export const useAuthStore = defineStore('auth', {
  state: () => ({
    user: null as User | null,
    isLoading: false,
    error: null as string | null,
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
      } catch (e) {
        this.error = e instanceof Error ? e.message : '登录失败'
        throw e
      } finally {
        this.isLoading = false
      }
    },

    /** 登出并清除状态 */
    async logout() {
      this.isLoading = true
      this.error = null
      try {
        await authService.logout()
        this.user = null
      } catch (e) {
        this.error = e instanceof Error ? e.message : '登出失败'
        throw e
      } finally {
        this.isLoading = false
      }
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
        this.isLoading = false
      }
    },
  },
})
