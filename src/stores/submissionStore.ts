import { defineStore } from 'pinia'
import type { JudgementStatus } from '@/types/submission'
import { submissionService } from '@/services/submission.service'
import { configService } from '@/services/config.service'
import { isTerminalStatus } from '@/utils/submission'

interface SubmissionEntry {
  id: string
  problemId: string
  status: JudgementStatus
  time?: number
  memory?: number
  submittedAt: string
}

/**
 * submissionId → 轮询定时器句柄。
 *
 * 定时器属于副作用句柄而非渲染状态，故放在模块作用域（不进入响应式系统），
 * 由 `stopPolling` / `stopAllPolling` 统一回收；登出时经 `stores/session` 调用后者。
 */
const pollTimers = new Map<string, ReturnType<typeof setInterval>>()

export const useSubmissionStore = defineStore('submission', {
  state: () => ({
    submissions: [] as SubmissionEntry[],
    isSubmitting: false,
    error: null as string | null,
  }),

  actions: {
    /** 提交代码、记录提交条目并启动评测轮询 */
    async submitCode(
      contestId: string,
      problemId: string,
      language: string,
      sourceCode: string,
    ): Promise<string> {
      this.isSubmitting = true
      this.error = null
      let submissionId: string
      try {
        submissionId = await submissionService.submitCode(contestId, problemId, language, sourceCode)
        this.submissions.push({
          id: submissionId,
          problemId,
          status: 'Pending',
          submittedAt: new Date().toISOString(),
        })
      } catch (e) {
        this.error = e instanceof Error ? e.message : '提交失败'
        throw e
      } finally {
        this.isSubmitting = false
      }
      // 提交已成功，轮询启动与提交结果解耦（轮询参数读取失败也有兜底值）
      await this.startPolling(submissionId)
      return submissionId
    },

    /** 轮询评测结果并更新对应提交的状态 */
    async pollResult(submissionId: string) {
      try {
        const result = await submissionService.pollJudgement(submissionId)
        const entry = this.submissions.find((s) => s.id === submissionId)
        if (entry) {
          entry.status = result.status
          entry.time = result.timeMs
          // 内存占用同样回填，否则控制台条/提交列表只能显示耗时
          entry.memory = result.memoryKb
        }
        return result
      } catch (e) {
        this.error = e instanceof Error ? e.message : '获取评测结果失败'
        throw e
      }
    },

    /**
     * 启动指定提交的评测轮询：命中终态或超过总超时后自动停止。
     *
     * 幂等 —— 重复调用会先停掉该提交已有的定时器，避免并发轮询。
     */
    async startPolling(submissionId: string) {
      this.stopPolling(submissionId)
      const { intervalMs, timeoutMs } = await configService.getPollSchedule()
      // 读取配置期间会话可能已被清理（登出/切换账号），此时不再注册定时器
      if (!this.submissions.some((s) => s.id === submissionId)) return

      const deadline = Date.now() + timeoutMs
      const timer = setInterval(() => {
        void this.pollOnce(submissionId, deadline)
      }, intervalMs)
      pollTimers.set(submissionId, timer)
    },

    /** 单次轮询：终态或超时即停止；瞬时失败不中断循环，由总超时兜底 */
    async pollOnce(submissionId: string, deadline: number) {
      if (Date.now() >= deadline) {
        console.warn(`[submissionStore] 提交 ${submissionId} 评测轮询超时，已停止`)
        this.stopPolling(submissionId)
        return
      }
      try {
        const result = await this.pollResult(submissionId)
        if (isTerminalStatus(result.status)) this.stopPolling(submissionId)
      } catch {
        // 网络抖动等瞬时错误：等待下一次轮询
      }
    },

    /** 停止指定提交的轮询 */
    stopPolling(submissionId: string) {
      const timer = pollTimers.get(submissionId)
      if (timer) {
        clearInterval(timer)
        pollTimers.delete(submissionId)
      }
    },

    /** 停止全部轮询（登出/切换账号时调用，防止定时器脱离会话继续请求） */
    stopAllPolling() {
      pollTimers.forEach((timer) => clearInterval(timer))
      pollTimers.clear()
    },
  },
})
