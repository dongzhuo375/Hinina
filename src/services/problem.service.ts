import type { Problem } from '@/types/problem'
import * as problemBridge from '@/bridge/problem.bridge'

/**
 * 题目服务 — 管理题目详情的获取。
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
}

export const problemService = new ProblemService()
