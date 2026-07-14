import type { JudgementResult } from '@/types/submission'
import * as submissionBridge from '@/bridge/submission.bridge'

/**
 * 提交服务 — 管理代码提交与评测结果轮询。
 */
export class SubmissionService {
  /**
   * 提交代码，返回 submissionId。
   */
  async submitCode(
    contestId: string,
    problemId: string,
    language: string,
    sourceCode: string,
  ): Promise<string> {
    return submissionBridge.submitCode(contestId, problemId, language, sourceCode)
  }

  /**
   * 轮询获取评测结果。
   */
  async pollJudgement(submissionId: string): Promise<JudgementResult> {
    return submissionBridge.getJudgement(submissionId)
  }
}

export const submissionService = new SubmissionService()
