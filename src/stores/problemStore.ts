import { defineStore } from 'pinia'
import type { Problem } from '@/types/problem'
import type { ProblemLimits, UserProblemStatus } from '@/types/rank'
import { problemService } from '@/services/problem.service'
import { errorMessage } from '@/utils/error'
import { createLogger } from '@/utils/logger'

const log = createLogger('problemStore')

export const useProblemStore = defineStore('problem', {
  state: () => ({
    currentProblem: null as Problem | null,
    isLoading: false,
    error: null as string | null,

    /// displayId → limits（渐进填充：卡片先渲染，limits 到达后补齐）
    limits: {} as Record<string, ProblemLimits>,
    isLimitsLoading: false,
    /// pid → 我的提交状态（0=未提交 / 1=已AC / 2=尝试过）
    myStatus: {} as Record<string, UserProblemStatus>,
    isStatusLoading: false,
    /// 我的题目状态是否已过期。
    ///
    /// 该数据**只由我自己的提交**改变（他人 AC 不影响它），因此不必按轮询周期
    /// 整表重拉：提交到达终态（或轮询超时、终态未知）时由 `submissionStore` 调
    /// `invalidateMyStatus()` 置位，总览页在下次可见刷新时重拉一次即清位。
    /// 初始为 true（首屏必拉一次）。
    myStatusStale: true,
    /// `myStatus` 中的数据属于哪场比赛（null = 尚无数据）。
    ///
    /// 状态表以 **pid** 为键，而同一题可能出现在多场比赛里 —— 切换比赛（设置页改
    /// `contestId`）后必须重拉，否则旧比赛的状态会命中新比赛的卡片。
    myStatusContestId: null as string | null,
  }),

  getters: {
    /** 当前打开的题目 ID */
    currentProblemId: (state) => state.currentProblem?.id ?? null,

    /** 取某题的 limits；缺失表示后端获取失败，视图应显示占位而非假默认值 */
    limitsOf: (state) => (displayId: string): ProblemLimits | null =>
      state.limits[displayId] ?? null,

    /** 取某题我的提交状态；未出现在 map 中视为未提交 */
    statusOf: (state) => (problemId: string): UserProblemStatus =>
      state.myStatus[problemId] ?? 0,

    /**
     * 指定比赛是否需要重拉「我的题目状态」。
     *
     * 判据两条：① 标记为过期（我的提交到达终态 / 轮询超时终态未知）；
     * ② **数据属于另一场比赛** —— 状态表以 pid 为键，同一题可能出现在多场比赛里，
     * 切换比赛（设置页改 `contestId`）后沿用旧数据会让卡片显示别的比赛的判定。
     */
    myStatusNeedsReloadFor: (state) => (contestId: string): boolean =>
      state.myStatusStale || state.myStatusContestId !== contestId,
  },

  actions: {
    /** 打开指定题目详情并设为当前题目 */
    async openProblem(contestId: string, problemId: string) {
      this.isLoading = true
      this.error = null
      try {
        this.currentProblem = await problemService.getProblem(contestId, problemId)
      } catch (e) {
        this.error = errorMessage(e, '加载题目失败')
        throw e
      } finally {
        this.isLoading = false
      }
    },

    /**
     * 批量加载题目 limits（后端带内存 + 磁盘双层缓存，命中时零网络请求）。
     *
     * 失败**不抛出**：limits 只影响卡片上的一行信息，不应打断整页渲染；
     * 缺失的题在 `limitsOf` 中返回 null，视图显示占位。
     * 认证类错误（401/403）已由全局会话守卫统一处理，此处只记录日志。
     */
    async loadLimits(contestId: string, displayIds: string[]) {
      if (displayIds.length === 0) return
      this.isLimitsLoading = true
      try {
        const fetched = await problemService.getProblemLimits(contestId, displayIds)
        this.limits = { ...this.limits, ...fetched }
      } catch (e) {
        log.error('题目 limits 加载失败:', e)
      } finally {
        this.isLimitsLoading = false
      }
    },

    /**
     * 批量加载我的题目提交状态（驱动卡片状态标记与解题进度）。
     *
     * 同 `loadLimits`：失败不抛出，状态缺失时按「未提交」展示。
     * 成功后清除 `myStatusStale`；失败则置为过期（数据仍是旧的），下一周期重试。
     */
    async loadMyStatus(contestId: string, problemIds: string[]) {
      if (problemIds.length === 0) return
      this.isStatusLoading = true
      try {
        this.myStatus = await problemService.getUserProblemStatus(contestId, problemIds)
        this.myStatusStale = false
        // 记录数据归属：切比赛后据此判定必须重拉
        this.myStatusContestId = contestId
      } catch (e) {
        // 拉取失败：数据仍是旧的 → 置为过期，下一周期重试（与改造前「每周期重拉」
        // 的容错等价，只是不再在成功路径上白打请求）。归属保持不变 —— 表里装的
        // 仍是上一次成功拉取的那场比赛的数据。
        this.myStatusStale = true
        log.error('我的题目状态加载失败:', e)
      } finally {
        this.isStatusLoading = false
      }
    },

    /**
     * 标记「我的题目状态」已过期（提交到达终态时由 `submissionStore` 调用）。
     *
     * 只置位、不发请求 —— 真正的重拉交给总览页在下次可见刷新时执行，
     * 避免在解题页后台凭空多打一次请求。
     */
    invalidateMyStatus() {
      this.myStatusStale = true
    },
  },
})
