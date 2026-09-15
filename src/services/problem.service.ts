import type { Problem } from '@/types/problem'
import type { ProblemLimits, UserProblemStatus } from '@/types/rank'
import * as problemBridge from '@/bridge/problem.bridge'

/**
 * 题目服务 — 管理题目详情、limits 与我的提交状态的获取。
 */
export class ProblemService {
  /**
   * 获取单个题目详情。
   */
  async getProblem(contestId: string, problemId: string): Promise<Problem> {
    return problemBridge.getProblem(contestId, problemId)
  }

  /**
   * 列出比赛下所有题目。
   */
  async listProblems(contestId: string): Promise<Problem[]> {
    return problemBridge.listProblems(contestId)
  }

  /**
   * 批量获取我的题目提交状态（0=未提交 / 1=已AC / 2=尝试过）。
   *
   * 空列表直接返回空对象，不发无意义的 IPC。
   */
  async getUserProblemStatus(
    contestId: string,
    problemIds: string[],
  ): Promise<Record<string, UserProblemStatus>> {
    if (problemIds.length === 0) return {}
    return problemBridge.getUserProblemStatus(contestId, problemIds)
  }

  /**
   * 批量获取题目 limits（后端带双层缓存）。
   *
   * 结果按 displayId 索引；**缺失的题表示后端获取失败**（如 403 私有题不可访问），
   * 调用方应显示占位而不是回退成假默认值。
   */
  async getProblemLimits(
    contestId: string,
    displayIds: string[],
  ): Promise<Record<string, ProblemLimits>> {
    if (displayIds.length === 0) return {}
    const list = await problemBridge.getContestProblemLimits(contestId, displayIds)
    return Object.fromEntries(list.map((item) => [item.displayId, item]))
  }
}

export const problemService = new ProblemService()
