import type { User } from '@/types/user'
import { ipcInvoke } from '@/bridge'

/** 登录，返回用户信息 */
export async function login(username: string, password: string, ojType?: string): Promise<User> {
  return ipcInvoke<User>('login', { username, password, ojType })
}

/** 登出 */
export async function logout(): Promise<void> {
  return ipcInvoke<void>('logout')
}

/** 获取当前会话 */
export async function getSession(): Promise<User | null> {
  return ipcInvoke<User | null>('get_session')
}
