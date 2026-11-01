import { ref } from 'vue'

/**
 * 钥匙模式的阶段闸门提示（评审十五 B）。
 *
 * 赛前/赛后从设置页点题目/榜单会被守卫拦回设置页——vue-router 只 resolve 一个
 * NavigationFailure，用户看不到任何反馈（导航又不能禁用：那正是评审六修的
 * inert）。守卫写入这里，SettingsView 渲染一行提示并自动清除。
 *
 * 模块级单例（与 settings-access 同先例）：没有领域数据、不走 IPC，不值得一个
 * store。
 */
export const settingsGateNotice = ref<string | null>(null)

/** 提示停留时长（毫秒）：足够读到，又不长期占位 */
const NOTICE_MS = 4_000

let timer: ReturnType<typeof setTimeout> | null = null

/// 记录一次被闸门拦回的导航（自动清除）。
export function markSettingsGateBlocked(message: string): void {
  settingsGateNotice.value = message
  if (timer) clearTimeout(timer)
  timer = setTimeout(() => {
    settingsGateNotice.value = null
    timer = null
  }, NOTICE_MS)
}

/// 测试/卸载用：清空提示与定时器。
export function clearSettingsGateNotice(): void {
  if (timer) clearTimeout(timer)
  timer = null
  settingsGateNotice.value = null
}
