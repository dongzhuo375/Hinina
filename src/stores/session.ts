import { useAnnouncementStore } from '@/stores/announcementStore'
import { useAuthStore } from '@/stores/authStore'
import { useContestStore } from '@/stores/contestStore'
import { useProblemStore } from '@/stores/problemStore'
import { useRankStore } from '@/stores/rankStore'
import { useSubmissionStore } from '@/stores/submissionStore'
import { useWorkspaceStore } from '@/stores/workspaceStore'

/**
 * 会话级领域状态清理。
 *
 * Pinia store 的生命周期与组件无关，登出/切换账号时若不主动重置，
 * 上一位选手的比赛、题面、提交记录与编辑器代码会残留到下一个会话
 * （竞赛场景下机位账号常被复用，属于数据泄露与误操作风险）。
 *
 * 认证状态由 `authStore` 自行清理，此处只负责领域状态。
 */
export function clearDomainState(): void {
  // 先取消防抖定时器，避免登出后仍向后端写入已失效会话的代码
  const workspace = useWorkspaceStore()
  workspace.cancelPendingSync()
  workspace.$reset()

  // 停止评测轮询，避免定时器脱离会话继续请求
  const submission = useSubmissionStore()
  submission.stopAllPolling()
  submission.$reset()

  // 停止榜单实时刷新并清空榜单数据（含「我的行」，属于会话数据）
  const rank = useRankStore()
  rank.stopLive()
  rank.$reset()

  // 停止公告轮询并清空列表与已读状态（已读按用户隔离，不得跨会话残留）
  const announcement = useAnnouncementStore()
  announcement.stopLive()
  announcement.$reset()

  // 比赛 store 内含登录页匿名简报，只清理会话相关部分
  useContestStore().clearSessionData()
  useProblemStore().$reset()
}

/**
 * OJ 切换的会话上下文重置（调用点：SettingsView「当前 OJ」下拉切换成功后）。
 *
 * 切换 OJ = 整个应用换了一个服务端：旧 OJ 的用户、比赛、题面、提交与编辑器
 * 代码对新 OJ 全部失效，且解题页会拿旧 OJ 的 `contest.id` 向新 OJ 发提交 ——
 * 必须与登出同款清理。区别于登出的三点：
 * - **不调用 `authStore.logout()`**：Registry 已切到新 OJ，后端 logout 会拿
 *   新 OJ 的无凭证会话打它的登出端点，还会误删新 OJ 自己的会话文件；
 * - `sessionResolved` 复位为 false，下次导航由路由守卫 `checkSession`
 *   （后端 `get_session` 读新 OJ 的 `sessions/{id}.json`）自动恢复会话 ——
 *   之前在该 OJ 登录过则无感续用，否则落回登录表单；
 * - 各 OJ 的会话文件按 id 隔离，**旧 OJ 的登录态保留**：切回去仍免登录。
 */
export function resetSessionForOjSwitch(): void {
  clearDomainState()

  // 匿名简报属于**旧 OJ**（换服务端），一并复位：clearSessionData 的保留语义
  // 是登出场景（同服务端，简报不属会话数据）；若沿用，LoginView 的
  // canEnter 侦听器（immediate）会先按旧 OJ 的时间窗计算阶段，在新 OJ 有
  // 会话时立即自动推进赛场 —— 而新 OJ 可能未配置比赛。briefState 置 idle
  // 让登录页先显示「加载中」，挂载时 loadBrief 按新 OJ 重拉。
  const contest = useContestStore()
  contest.brief = null
  contest.briefBaseUrl = ''
  contest.briefError = null
  contest.briefState = 'idle'

  const auth = useAuthStore()
  auth.user = null
  auth.error = null
  auth.sessionResolved = false
}
