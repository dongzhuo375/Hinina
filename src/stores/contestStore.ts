import { defineStore } from 'pinia'
import type { Contest, ContestProblem } from '@/types/contest'
import { contestService } from '@/services/contest.service'
import { errorMessage } from '@/utils/error'
import { getSessionGeneration } from '@/stores/sessionGeneration'

/**
 * 进行中的比赛加载请求（模块级，不进响应式状态）。
 *
 * 外壳 `ContestLayout` 与各视图（题目总览/榜单/解题）都可能在同一时刻发现
 * 「比赛数据还没加载」而各自触发一次加载；共享同一个 in-flight Promise 可避免
 * 重复 IPC —— 开赛瞬间全场客户端同时进场时，这类重复请求会成倍放大服务端压力。
 */
let loadInFlight: Promise<void> | null = null
/// 在途登记的归属 token：`finally` 只清理自己登记的那次请求（会话切换后
/// 新会话可能已发起新的加载，旧请求不得把它清掉 —— 评审四轮 #1）
let loadInFlightToken: object | null = null
/// 在途登记所属的会话代际：去重只复用**同代际**的在途请求（评审五轮）。
/// 旧会话的 promise 即使仍在途也不能复用——它的结果会被代际守卫丢弃，
/// 复用会让新会话拿到一个不会写入任何数据的空结果（静默空数据）。
let loadInFlightGeneration = -1

/// 匿名简报的失效世代：**只**在 OJ 切换时递增（`invalidateBrief`）。
///
/// 简报跨登出保留（同 OJ 的匿名数据），因此不能用会话代际做守卫；但切 OJ
/// 后旧 OJ 的简报必须作废——否则切换前发出的 brief 请求在 reset 之后 resolve
/// 时会把旧 OJ 的比赛写回已重置状态，LoginView 的 canEnter 会按旧时间窗
/// 误自动进赛场（评审八轮）。
let briefEpoch = 0

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
      if (loadInFlight && loadInFlightGeneration === getSessionGeneration()) return loadInFlight
      return this.loadContest()
    },

    /**
     * 加载已配置的比赛数据（并发去重）。
     *
     * 同一时刻的多次调用共享一个请求；失败时所有调用方都会收到同一个 rejection，
     * 因此调用方必须自行 catch（错误信息已写入 `error`）。
     */
    async loadContest() {
      // 会话代际：登出后旧会话的比赛数据不得写回新会话（评审四轮 #1）；
      // 去重也只复用同代际的在途请求（评审五轮：跨会话复用会得到静默空结果）
      const sessionGen = getSessionGeneration()
      if (loadInFlight && loadInFlightGeneration === sessionGen) return loadInFlight

      this.isLoading = true
      this.error = null
      const token = {}
      const task = (async () => {
        try {
          const result = await contestService.loadConfiguredContest()
          if (sessionGen !== getSessionGeneration()) return
          this.contest = result.contest
          this.problems = result.problems
        } catch (e) {
          if (sessionGen === getSessionGeneration()) this.error = errorMessage(e, '加载比赛失败')
          throw e
        } finally {
          if (sessionGen === getSessionGeneration()) this.isLoading = false
          // 只清理自己登记的在途请求
          if (loadInFlightToken === token) {
            loadInFlight = null
            loadInFlightToken = null
            loadInFlightGeneration = -1
          }
        }
      })()
      loadInFlight = task
      loadInFlightToken = token
      loadInFlightGeneration = sessionGen
      return task
    },

    /**
     * 加载登录页展示用的匿名比赛简报。
     *
     * 编排逻辑位于 `contestService`，此处只做状态映射；失败不抛出，
     * 由 `briefState` / `briefError` 驱动登录页的连接状态与重试入口。
     */
    async loadBrief() {
      // 简报的过期守卫用「OJ 失效世代」而不是会话代际：简报跨登出保留，
      // 但切 OJ 后旧请求的回写必须丢弃（评审八轮）
      const epoch = briefEpoch
      this.briefState = 'connecting'
      this.briefError = null
      try {
        const result = await contestService.loadContestBrief()
        if (epoch !== briefEpoch) return
        this.briefBaseUrl = result.baseUrl
        if (result.status === 'unconfigured') {
          this.brief = null
          this.briefState = 'unconfigured'
          return
        }
        this.brief = result.contest
        this.briefState = 'connected'
      } catch (e) {
        if (epoch !== briefEpoch) return
        this.brief = null
        this.briefError = errorMessage(e, '获取比赛信息失败')
        this.briefState = 'failed'
      }
    },

    /**
     * 作废匿名简报（**仅 OJ 切换时**由 `resetSessionForOjSwitch` 调用）。
     *
     * 递增 `briefEpoch` 使在途的旧 OJ brief 请求在 resolve 时被丢弃；
     * 再清空简报状态让登录页按新 OJ 重拉。登出不清简报（同 OJ 的匿名数据，
     * 保留可避免切换账号时右侧氛围区空白）。
     */
    invalidateBrief() {
      briefEpoch += 1
      this.brief = null
      this.briefBaseUrl = ''
      this.briefError = null
      this.briefState = 'idle'
    },

    /**
     * 清空会话相关状态（登出/切换账号时调用）。
     * 匿名比赛简报与登录态无关，予以保留。
     *
     * **同时注销在途加载登记**：`loadInFlight` 是模块级变量，`$reset()` 之外的
     * 定向清理不会触及它；不注销的话新会话的去重会拿到旧会话的 promise
     * （其结果被代际守卫丢弃 → 新会话静默空数据，评审五轮）。
     */
    clearSessionData() {
      loadInFlight = null
      loadInFlightToken = null
      loadInFlightGeneration = -1
      this.contest = null
      this.problems = []
      this.isLoading = false
      this.error = null
    },
  },
})
