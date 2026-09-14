import { defineStore } from 'pinia'
import type { Contest, ContestProblem } from '@/types/contest'
import { contestService } from '@/services/contest.service'

export const useContestStore = defineStore('contest', {
  state: () => ({
    contest: null as Contest | null,
    problems: [] as ContestProblem[],
    isLoading: false,
    error: null as string | null,

    // ── 登录页匿名比赛简报（不依赖会话，登出后保留，避免切换账号时右侧氛围区空白） ──
    brief: null as Contest | null,
    briefBaseUrl: '',
    briefState: 'idle' as 'idle' | 'connecting' | 'connected' | 'failed' | 'unconfigured',
    briefError: null as string | null,
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

    /**
     * 加载登录页展示用的匿名比赛简报。
     *
     * 编排逻辑位于 `contestService`，此处只做状态映射；失败不抛出，
     * 由 `briefState` / `briefError` 驱动登录页的连接状态与重试入口。
     */
    async loadBrief() {
      this.briefState = 'connecting'
      this.briefError = null
      try {
        const result = await contestService.loadContestBrief()
        this.briefBaseUrl = result.baseUrl
        if (result.status === 'unconfigured') {
          this.brief = null
          this.briefState = 'unconfigured'
          return
        }
        this.brief = result.contest
        this.briefState = 'connected'
      } catch (e) {
        this.brief = null
        this.briefError = e instanceof Error ? e.message : '获取比赛信息失败'
        this.briefState = 'failed'
      }
    },

    /**
     * 清空会话相关状态（登出/切换账号时调用）。
     * 匿名比赛简报与登录态无关，予以保留。
     */
    clearSessionData() {
      this.contest = null
      this.problems = []
      this.isLoading = false
      this.error = null
    },
  },
})
