import { defineStore } from 'pinia'
import type { Contest, ContestProblem } from '@/types/contest'
import { contestService } from '@/services/contest.service'

/**
 * 进行中的比赛加载请求（模块级，不进响应式状态）。
 *
 * 外壳 `ContestLayout` 与各视图（题目总览/榜单/解题）都可能在同一时刻发现
 * 「比赛数据还没加载」而各自触发一次加载；共享同一个 in-flight Promise 可避免
 * 重复 IPC —— 开赛瞬间全场客户端同时进场时，这类重复请求会成倍放大服务端压力。
 */
let loadInFlight: Promise<void> | null = null

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
    /**
     * 等待比赛数据就绪（P59 统一入口）。
     *
     * 外壳 `ContestLayout` 与各子视图都可能发现「比赛还没加载」：
     * - 已有数据 → 立即返回；
     * - 加载在途 → 复用同一个 in-flight Promise（不重复请求）；
     * - 无人加载 → 由本调用发起。
     *
     * 失败时 rejection 透传给调用方（错误已写入 `error`），调用方自行 catch。
     */
    async whenLoaded(): Promise<void> {
      if (this.contest) return
      if (loadInFlight) return loadInFlight
      return this.loadContest()
    },

    /**
     * 加载已配置的比赛数据（并发去重）。
     *
     * 同一时刻的多次调用共享一个请求；失败时所有调用方都会收到同一个 rejection，
     * 因此调用方必须自行 catch（错误信息已写入 `error`）。
     */
    async loadContest() {
      if (loadInFlight) return loadInFlight

      this.isLoading = true
      this.error = null
      const task = (async () => {
        try {
          const result = await contestService.loadConfiguredContest()
          this.contest = result.contest
          this.problems = result.problems
        } catch (e) {
          this.error = e instanceof Error ? e.message : '加载比赛失败'
          throw e
        } finally {
          this.isLoading = false
          loadInFlight = null
        }
      })()
      loadInFlight = task
      return task
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
