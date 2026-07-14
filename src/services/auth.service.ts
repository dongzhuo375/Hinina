import type { User } from '@/types/user'
import * as authBridge from '@/bridge/auth.bridge'

const STORED_USER_KEY = 'hinina_user'

/**
 * 认证服务 — 管理用户登录、登出与本地会话缓存。
 */
export class AuthService {
  /**
   * 登录并持久化用户信息到 localStorage。
   */
  async login(username: string, password: string, ojType?: string): Promise<User> {
    const user = await authBridge.login(username, password, ojType)
    localStorage.setItem(STORED_USER_KEY, JSON.stringify(user))
    return user
  }

  /**
   * 登出并清除本地缓存。
   */
  async logout(): Promise<void> {
    await authBridge.logout()
    localStorage.removeItem(STORED_USER_KEY)
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
}

export const authService = new AuthService()
