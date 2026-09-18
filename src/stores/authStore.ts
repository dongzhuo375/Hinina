import { defineStore } from 'pinia'
import type { SessionValidity, User } from '@/types/user'
import { authService } from '@/services/auth.service'
import { clearDomainState } from '@/stores/session'
import { errorMessage } from '@/utils/error'
import { createLogger } from '@/utils/logger'

const log = createLogger('authStore')

/**
 * 会话失效的统一提示文案。
 *
 * 竞赛场景下最常见的失效原因是同一账号在其他设备登录导致服务端撤销凭证，
 * 文案需给出可执行的下一步（重新登录），而不是只报"未授权"。
 */
export const SESSION_INVALID_MESSAGE = '登录状态已失效（该账号可能已在其他设备登录），请重新登录'

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
    async login(username: string, password: string) {
      this.isLoading = true
      this.error = null
      try {
        this.user = await authService.login(username, password)
        this.sessionResolved = true
      } catch (e) {
        this.error = errorMessage(e, '登录失败')
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
        this.error = errorMessage(e, '登出失败')
        log.error('后端登出失败，已强制清理本地会话:', e)
      }
      this.user = null
      this.sessionResolved = backendCleared
      clearDomainState()
      this.isLoading = false
    },

    /**
     * 校验后端会话有效性（三态），并在确认失效时就地清理。
     *
     * `unknown`（网络异常等）**保持登录态不变**，由调用方决定重试 ——
     * 赛前把选手误踢回登录页的代价，远大于多等一轮校验。
     */
    async validateSession(): Promise<SessionValidity> {
      const validity = await authService.validateSession()
      if (validity === 'invalid' && this.isLoggedIn) {
        await this.invalidateSession(SESSION_INVALID_MESSAGE)
      }
      return validity
    },

    /**
     * 会话被判定失效（服务端撤销凭证 / 认证接口 401）：清理后端与本地状态并记录原因。
     *
     * 与主动登出共用清理链路，区别在于：
     * - `sessionResolved` 复位为 false，强制下次进入受保护路由重新向后端校验
     * - `error` 携带失效原因，供登录页展示
     */
    async invalidateSession(reason: string) {
      await this.logout()
      this.error = reason
      this.sessionResolved = false
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
        this.error = errorMessage(e, '会话检查失败')
        return false
      } finally {
        this.sessionResolved = true
        this.isLoading = false
      }
    },
  },
})
