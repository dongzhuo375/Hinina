import type { SessionValidity, User } from '@/types/user'
import { ipcInvoke } from '@/bridge'

/** 登录，返回用户信息（OJ 切换走显式 `switchOj`，与登录解耦） */
export async function login(username: string, password: string): Promise<User> {
  return ipcInvoke<User>('login', { username, password })
}

/** 登出 */
export async function logout(): Promise<void> {
  return ipcInvoke<void>('logout')
}

/** 获取当前会话 */
export async function getSession(): Promise<User | null> {
  return ipcInvoke<User | null>('get_session')
}

/** 校验当前会话是否仍然有效（三态：valid / invalid / unknown） */
export async function validateSession(): Promise<SessionValidity> {
  return ipcInvoke<SessionValidity>('validate_session')
}
