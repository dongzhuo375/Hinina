import { defineStore } from 'pinia'
import type { Contest, ContestProblem } from '@/types/contest'
import { contestService } from '@/services/contest.service'

export const useContestStore = defineStore('contest', {
  state: () => ({
    contest: null as Contest | null,
    problems: [] as ContestProblem[],
    isLoading: false,
    error: null as string | null,
  }),

  getters: {
    /** 当前比赛中所有题目的 displayId 列表 */
    currentProblemIds: (state) => state.problems.map((p) => p.displayId),

    /** 比赛题目数量 */
    problemCount: (state) => state.problems.length,
  },

  actions: {
    /** 加载已配置的比赛数据 */
    async loadContest() {
      this.isLoading = true
      this.error = null
      try {
        const result = await contestService.loadConfiguredContest()
        this.contest = result.contest
        this.problems = result.problems
      } catch (e) {
        this.error = e instanceof Error ? e.message : '加载比赛失败'
        throw e
      } finally {
        this.isLoading = false
      }
    },
  },
})
