import type { SessionValidity, User } from '@/types/user'
import * as authBridge from '@/bridge/auth.bridge'
import { createLogger } from '@/utils/logger'

const log = createLogger('authService')

const STORED_USER_KEY = 'hinina_user'

/**
 * 认证服务 — 管理用户登录、登出与本地会话缓存。
 */
export class AuthService {
  /**
   * 登录并持久化用户信息到 localStorage。
   */
  async login(username: string, password: string): Promise<User> {
    const user = await authBridge.login(username, password)
    localStorage.setItem(STORED_USER_KEY, JSON.stringify(user))
    return user
  }

  /**
   * 登出并清除本地缓存。
   *
   * 后端登出失败时仍清理本地缓存：本地状态必须与"已登出"的 UI 语义保持一致。
   */
  async logout(): Promise<void> {
    try {
      await authBridge.logout()
    } finally {
      localStorage.removeItem(STORED_USER_KEY)
    }
  }

  /**
   * 检查后端会话是否有效，返回当前用户或 null。
   */
  async checkSession(): Promise<User | null> {
    try {
      const user = await authBridge.getSession()
      if (user) {
        localStorage.setItem(STORED_USER_KEY, JSON.stringify(user))
      }
      return user
    } catch {
      return null
    }
  }

  /**
   * 校验后端会话有效性（三态）。
   *
   * IPC 自身异常（序列化/通道故障）归一为 `unknown`：调用方只需面对三种业务语义，
   * 且传输层故障不会被误判为"会话失效"而把用户踢回登录页。
   */
  async validateSession(): Promise<SessionValidity> {
    try {
      return await authBridge.validateSession()
    } catch (e) {
      log.error('会话校验调用失败，按无法判定处理:', e)
      return 'unknown'
    }
  }

  /**
   * 从 localStorage 读取缓存的用户信息。
   */
  getStoredUser(): User | null {
    const raw = localStorage.getItem(STORED_USER_KEY)
    if (!raw) return null
    try {
      return JSON.parse(raw) as User
    } catch {
      localStorage.removeItem(STORED_USER_KEY)
      return null
    }
  }

  /**
   * 清除 localStorage 中的用户缓存（OJ 切换时调用）。
   *
   * 缓存里的用户属于**旧 OJ**，不得残留给新 OJ 的会话上下文；新会话由
   * `checkSession`（后端 `get_session` 读新 OJ 的会话文件）重建并回写。
   */
  clearStoredUser(): void {
    localStorage.removeItem(STORED_USER_KEY)
  }
}

export const authService = new AuthService()
