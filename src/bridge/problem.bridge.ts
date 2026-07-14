import type { Problem } from '@/types/problem'
import { ipcInvoke } from '@/bridge'

/** 获取单个题目详情 */
export async function getProblem(contestId: string, problemId: string): Promise<Problem> {
  return ipcInvoke<Problem>('get_problem', { contestId, problemId })
}

/** 列出比赛下所有题目 */
export async function listProblems(contestId: string): Promise<Problem[]> {
  return ipcInvoke<Problem[]>('list_problems', { contestId })
}
