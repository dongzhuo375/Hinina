import { defineStore } from 'pinia'
import type { Problem } from '@/types/problem'
import { problemService } from '@/services/problem.service'

export const useProblemStore = defineStore('problem', {
  state: () => ({
    currentProblem: null as Problem | null,
    problems: [] as Problem[],
    isLoading: false,
    error: null as string | null,
  }),

  getters: {
    /** 当前打开的题目 ID */
    currentProblemId: (state) => state.currentProblem?.id ?? null,
  },

  actions: {
    /** 打开指定题目详情并设为当前题目 */
    async openProblem(contestId: string, problemId: string) {
      this.isLoading = true
      this.error = null
      try {
        this.currentProblem = await problemService.getProblem(contestId, problemId)
      } catch (e) {
        this.error = e instanceof Error ? e.message : '加载题目失败'
        throw e
      } finally {
        this.isLoading = false
      }
    },

    /** 加载比赛下所有题目列表 */
    async loadProblems(contestId: string) {
      this.isLoading = true
      this.error = null
      try {
        this.problems = await problemService.listProblems(contestId)
      } catch (e) {
        this.error = e instanceof Error ? e.message : '加载题目列表失败'
        throw e
      } finally {
        this.isLoading = false
      }
    },
  },
})
