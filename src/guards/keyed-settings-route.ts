import { systemService } from '@/services/system.service'

/**
 * 钥匙模式 + 指定路由是否为设置页：**路由级唯一判据**。
 *
 * 判断需要两个来源——进程级钥匙状态（`systemService.isKeyed()`）与**路由身份**
 * （`routeName === 'Settings'`）。服务层不认识路由，所以组合放在 guards/：
 * 这是名副其实的跨层装配（services 不 import 本模块，无反向边）。
 *
 * 各消费方（阶段闸门/最小壳、sessionGuard 旁路、OJ 切换停留、路由落地不变量）
 * 都走这里，避免注释声称一致、实现各自内联的漂移。进程级判断请直接用
 * `systemService.isKeyed()`。
 */
export function isKeyedSettingsRoute(
  routeName: string | symbol | null | undefined,
): boolean {
  return systemService.isKeyed() && routeName === 'Settings'
}
