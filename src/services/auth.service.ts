import type { SessionValidity, User } from '@/types/user'
import * as authBridge from '@/bridge/auth.bridge'
import { createLogger } from '@/utils/logger'

const log = createLogger('authService')

/**
 * 认证服务 — 管理用户登录、登出与会话校验。
 *
 * 刻意**不做**前端持久化：用户信息（含 token）的唯一存放处是后端会话文件，
 * 前端每次经 `checkSession` / `validateSession` 向后端查询。localStorage 可被
 * 同源任意脚本读取，把含 token 的用户对象缓存进去是纯暴露面 —— 且该缓存
 * 没有任何生产消费方（只有测试读过它），属于「只写不读」的死代码。
 */
export class AuthService {
  /**
   * 登录，返回后端会话对应的用户。
   */
  async login(username: string, password: string): Promise<User> {
    return authBridge.login(username, password)
  }

  /**
   * 登出。
   *
   * 后端登出失败时异常向上抛出，由调用方（`authStore.logout`）决定 UI 语义；
   * 服务层不吞错误，也不维护任何需要在此清理的前端副本。
   */
  async logout(): Promise<void> {
    await authBridge.logout()
  }

  /**
   * 检查后端会话是否有效，返回当前用户或 null。
   */
  async checkSession(): Promise<User | null> {
    try {
      return await authBridge.getSession()
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
}

export const authService = new AuthService()
