import { systemService } from '@/services/system.service'

/**
 * 「管理方钥匙」（`--settings`）豁免策略的**唯一判定点**（评审二十四 #4）。
 *
 * 钥匙模式有两条互不相同的判据，消费方一律经这里取，不要各自读
 * `launchFlags` 或重复拼 `route.name === 'Settings'`：
 *
 * - `isKeyedProcess()` —— 进程级：本次启动是否带 `--settings`。用于进程级行为
 *   （设置页分层锁、登录页「返回设置」入口、ActivityBar 恒显入口）。
 * - `isKeyedSettingsRoute(routeName)` —— 路由级：钥匙模式 **且** 目标/当前路由
 *   是设置页。这是设置页豁免（免登录进入、会话失效不跳登录、阶段闸门拦回、
 *   OJ 切换停留、导航落地不变量）的共同判据。
 *
 * 协议：**豁免只属于设置页**。任何「钥匙模式下某处也照做」的新逻辑，先在这里
 * 加一个具名函数或复用已有函数，并在注释里写清与豁免的关系——避免同一规则
 * 散落在路由/守卫/视图里各自漂移（历史上有 7 处内联，靠多轮评审才收敛）。
 */
export function isKeyedProcess(): boolean {
  return systemService.isKeyed()
}

export function isKeyedSettingsRoute(routeName: string | symbol | null | undefined): boolean {
  return systemService.isKeyedSettings(routeName)
}
