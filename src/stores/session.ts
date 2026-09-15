import { useAnnouncementStore } from '@/stores/announcementStore'
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
