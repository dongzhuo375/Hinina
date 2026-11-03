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

/**
 * 认证代际：登录 / 登出 / OJ 切换 / 发起会话检查都会递增。
 *
 * `checkSession` 在 await 前捕获代际、写回前比对——`resetForOjSwitch()` 只清
 * 状态并不取消已发出的 IPC，旧 OJ 的迟到响应若直接写回，会把旧用户标成
 * 「已在新 OJ 上解析」，路由守卫与登录页随后都跳过检查，界面一直显示旧用户，
 * 直到某个业务请求 401（评审：OJ 切换认证竞态）。登录/登出同样递增，避免
 * 在途检查覆盖刚完成的状态转换；每个新检查也递增，并发检查以最后发起的为准。
 */
let authEpoch = 0

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
      // 作废在途的会话检查；成功后再递增一次，避免登录期间发起的检查覆盖结果
      authEpoch += 1
      this.isLoading = true
      this.error = null
      try {
        const user = await authService.login(username, password)
        authEpoch += 1
        this.user = user
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
      // 作废在途的会话检查：登出期间的迟到响应不得把用户重新写回来
      authEpoch += 1
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
      // 清理提交前再递增一次：登出期间发起的检查同样作废
      authEpoch += 1
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
      const epoch = authEpoch
      const validity = await authService.validateSession()
      // 期间发生登录/登出/OJ 切换/新检查：这是旧上下文的结果，不得据此清理
      // 新会话（invalidateSession 会走 logout，可能误删新 OJ 的会话文件）
      if (epoch !== authEpoch) return 'unknown'
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

    /**
     * OJ 切换的认证态复位（调用点：useSettingsForm 切换成功后，紧随
     * `resetSessionForOjSwitch()`）。区别于 `logout` 的三点：**不调后端**
     * （Registry 已切到新 OJ，后端 logout 会拿新 OJ 的无凭证会话打它的
     * 登出端点，还会误删新 OJ 自己的会话文件）；`sessionResolved` 复位为
     * false，下次导航由路由守卫 `checkSession`（后端读新 OJ 的会话文件）
     * 自动恢复 —— 之前在该 OJ 登录过则无感续用，否则落回登录表单；
     * 各 OJ 的会话文件按 id 隔离，**旧 OJ 的登录态保留**：切回去仍免登录。
     */
    resetForOjSwitch() {
      // 作废在途的 checkSession：旧 OJ 的迟到响应不得写回旧用户（认证竞态）
      authEpoch += 1
      this.user = null
      this.error = null
      this.sessionResolved = false
      this.isLoading = false
    },

    /**
     * 检查后端会话有效性。
     *
     * 只有成功取得用户或后端明确返回 null 才算已确认；传输异常保留原用户并
     * 保持 unresolved，后续路由守卫/登录页挂载可重试。调用方仍只收到 boolean。
     */
    async checkSession(): Promise<boolean> {
      // 新检查成为「最新」：更早发起的检查在写回前会见代际不符而放弃
      const epoch = ++authEpoch
      this.isLoading = true
      this.error = null
      try {
        const user = await authService.checkSession()
        // 登录/登出/OJ 切换/更新的检查已发生：这份结果属于旧上下文，丢弃
        if (epoch !== authEpoch) return false
        this.user = user
        this.sessionResolved = true
        return user !== null
      } catch (e) {
        if (epoch !== authEpoch) return false
        this.sessionResolved = false
        this.error = errorMessage(e, '会话检查失败')
        return false
      } finally {
        if (epoch === authEpoch) this.isLoading = false
      }
    },
  },
})
