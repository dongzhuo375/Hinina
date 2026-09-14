import { useContestStore } from '@/stores/contestStore'
import { useProblemStore } from '@/stores/problemStore'
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

  // 比赛 store 内含登录页匿名简报，只清理会话相关部分
  useContestStore().clearSessionData()
  useProblemStore().$reset()
  useSubmissionStore().$reset()
}
