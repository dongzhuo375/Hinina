import type { Router } from 'vue-router'
import { setIpcErrorObserver } from '@/bridge'
import type { IpcError } from '@/bridge'
import { SESSION_INVALID_MESSAGE, useAuthStore } from '@/stores/authStore'
import { createLogger } from '@/utils/logger'

const log = createLogger('sessionGuard')

/// 失效处理进行中标记：多个请求同时返回认证错误时只处理一次
let invalidating = false

/**
 * 安装全局会话守卫 —— 仅应在组合根（`main.ts`）调用一次。
 *
 * 任何认证类 IPC 失败都意味着服务端已不认这份凭证（token 过期、同账号在其他
 * 设备登录被撤销、服务端重启清空 Redis 等）。若不统一处理，用户会停留在受保护
 * 页面反复重试失败；此处在会话失效时清理本地状态并回到登录页，同时给出原因。
 *
 * 依赖倒置：Bridge 层只回调注入的观察者，不感知 store 与 router，
 * 装配关系集中在组合根，避免底层反向依赖上层。
 */
export function installSessionGuard(router: Router): void {
  setIpcErrorObserver((error) => {
    void handleAuthFailure(error, router)
  })
}

/// 认证类 IPC 失败的处理：判定会话失效 → 清理 → 必要时回登录页
async function handleAuthFailure(error: IpcError, router: Router): Promise<void> {
  if (!error.isAuthError || invalidating) return

  const auth = useAuthStore()
  // 本地无会话时（如登录表单密码错误）不属于会话失效，错误由调用方自行展示
  if (!auth.isLoggedIn) return

  invalidating = true
  try {
    log.warn(`${error.cmd} 返回认证错误，判定会话失效: ${error.message}`)
    await auth.invalidateSession(SESSION_INVALID_MESSAGE)
    // 仅当停留在受保护页面时需要跳转；登录页本身就是失效后的落点
    if (router.currentRoute.value.meta.requiresAuth) {
      await router.replace({ name: 'Login' })
    }
  } finally {
    invalidating = false
  }
}
