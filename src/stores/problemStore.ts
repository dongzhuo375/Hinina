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
    problems: [] as Problem[],
    isLoading: false,
    error: null as string | null,

    /// displayId → limits（渐进填充：卡片先渲染，limits 到达后补齐）
    limits: {} as Record<string, ProblemLimits>,
    isLimitsLoading: false,
    /// pid → 我的提交状态（0=未提交 / 1=已AC / 2=尝试过）
    myStatus: {} as Record<string, UserProblemStatus>,
    isStatusLoading: false,
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

    /** 加载比赛下所有题目列表 */
    async loadProblems(contestId: string) {
      this.isLoading = true
      this.error = null
      try {
        this.problems = await problemService.listProblems(contestId)
      } catch (e) {
        this.error = errorMessage(e, '加载题目列表失败')
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
     */
    async loadMyStatus(contestId: string, problemIds: string[]) {
      if (problemIds.length === 0) return
      this.isStatusLoading = true
      try {
        this.myStatus = await problemService.getUserProblemStatus(contestId, problemIds)
      } catch (e) {
        log.error('我的题目状态加载失败:', e)
      } finally {
        this.isStatusLoading = false
      }
    },
  },
})
