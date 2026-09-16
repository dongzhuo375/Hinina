import type {
  JudgementResult,
  SubmissionCases,
  SubmissionDetail,
  SubmissionListQuery,
  SubmissionPage,
} from '@/types/submission'
import * as submissionBridge from '@/bridge/submission.bridge'

/**
 * 提交服务 — 代码提交、评测轮询、提交历史与详情查询。
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

  /**
   * 查询本人提交历史（分页；onlyMine 由后端强制）。
   */
  async listContestSubmissions(query: SubmissionListQuery): Promise<SubmissionPage> {
    return submissionBridge.listContestSubmissions(query)
  }

  /**
   * 获取提交详情（含源代码 / CE 错误信息）。
   */
  async getSubmissionDetail(submissionId: string): Promise<SubmissionDetail> {
    return submissionBridge.getSubmissionDetail(submissionId)
  }

  /**
   * 获取测试点详情（含子任务分组）。
   */
  async getSubmissionCases(submissionId: string): Promise<SubmissionCases> {
    return submissionBridge.getSubmissionCases(submissionId)
  }
}

export const submissionService = new SubmissionService()
