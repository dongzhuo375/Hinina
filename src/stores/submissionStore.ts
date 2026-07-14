import { defineStore } from 'pinia'
import type { JudgementStatus } from '@/types/submission'
import { submissionService } from '@/services/submission.service'

interface SubmissionEntry {
  id: string
  problemId: string
  status: JudgementStatus
  time?: number
  memory?: number
  submittedAt: string
}

export const useSubmissionStore = defineStore('submission', {
  state: () => ({
    submissions: [] as SubmissionEntry[],
    isSubmitting: false,
    error: null as string | null,
  }),

  actions: {
    /** 提交代码并记录提交条目 */
    async submitCode(
      contestId: string,
      problemId: string,
      language: string,
      sourceCode: string,
    ): Promise<string> {
      this.isSubmitting = true
      this.error = null
      try {
        const submissionId = await submissionService.submitCode(contestId, problemId, language, sourceCode)
        this.submissions.push({
          id: submissionId,
          problemId,
          status: 'Pending',
          submittedAt: new Date().toISOString(),
        })
        return submissionId
      } catch (e) {
        this.error = e instanceof Error ? e.message : '提交失败'
        throw e
      } finally {
        this.isSubmitting = false
      }
    },

    /** 轮询评测结果并更新对应提交的状态 */
    async pollResult(submissionId: string) {
      try {
        const result = await submissionService.pollJudgement(submissionId)
        const entry = this.submissions.find((s) => s.id === submissionId)
        if (entry) {
          entry.status = result.status
          entry.time = result.timeMs
        }
        return result
      } catch (e) {
        this.error = e instanceof Error ? e.message : '获取评测结果失败'
        throw e
      }
    },
  },
})
