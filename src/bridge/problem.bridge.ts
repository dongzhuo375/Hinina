import type { Problem } from '@/types/problem'
import type { ProblemLimits, UserProblemStatus } from '@/types/rank'
import { ipcInvoke } from '@/bridge'

/** 获取单个题目详情 */
export async function getProblem(contestId: string, problemId: string): Promise<Problem> {
  return ipcInvoke<Problem>('get_problem', { contestId, problemId })
}

/**
 * 批量获取当前用户对指定题目的提交状态。
 *
 * 返回 `{ pid: 0|1|2 }`（0=未提交，1=已AC，2=尝试过）；未出现的 pid 视为未提交。
 */
export async function getUserProblemStatus(
  contestId: string,
  problemIds: string[],
): Promise<Record<string, UserProblemStatus>> {
  return ipcInvoke<Record<string, UserProblemStatus>>('get_user_problem_status', {
    contestId,
    problemIds,
  })
}

/**
 * 批量获取比赛题目的 limits（时间 ms / 内存 MB）。
 *
 * 后端带内存 + 磁盘双层缓存，命中时零网络请求。
 * **获取失败的题目不会出现在结果里**，调用方应显示占位而不是假默认值。
 */
export async function getContestProblemLimits(
  contestId: string,
  displayIds: string[],
): Promise<ProblemLimits[]> {
  return ipcInvoke<ProblemLimits[]>('get_contest_problem_limits', { contestId, displayIds })
}
