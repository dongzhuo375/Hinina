import { defineStore } from 'pinia'
import type { ContestRankPage, ContestRankRow } from '@/types/rank'
import { rankService } from '@/services/rank.service'
import { createPoller } from '@/utils/polling'
import type { Poller } from '@/utils/polling'
import {
  dedupeRankRows,
  resolveMyRow,
  resolveParticipantCountFromPage,
} from '@/utils/rank'

/// 榜单轮询节奏：HOJ 文档 §9.9 要求间隔 ≥10s 且切后台暂停；
/// 抖动用于打散全场客户端的同步相位（同一秒进入榜单页时不会每周期都齐发）
const RANK_POLL_INTERVAL_MS = 10_000
const RANK_POLL_JITTER_MS = 2_000

/// 榜单分组筛选：official 走服务端 removeStar，star/female 服务端无对应参数故客户端过滤
export type RankGroupFilter = 'all' | 'official' | 'star' | 'female'

/**
 * 轮询器句柄。
 *
 * 副作用句柄而非渲染状态，置于模块作用域（不进响应式系统），
 * 由 `stopLive` 回收；登出时经 `stores/session` 统一停止。
 */
let poller: Poller | null = null

/// 页面是否不可见（窗口最小化/切到后台）—— 此时暂停轮询，避免无谓请求
function isPageHidden(): boolean {
  return typeof document !== 'undefined' && document.hidden
}

export const useRankStore = defineStore('rank', {
  state: () => ({
    /// 当前页榜单行（已按 uid 去重）
    rows: [] as ContestRankRow[],
    /// 我的行（服务端会前置复制一份，因此任意页都能取到）
    myRow: null as ContestRankRow | null,
    total: 0,
    size: 0,
    current: 1,
    pages: 0,
    /// 真实参与人数（已修正服务端前置副本造成的 total 偏大）
    participants: 0,
    /// 服务端搜索关键词（只匹配学校或榜单显示名）
    keyword: '',
    /// 是否移除打星队伍（服务端参数）
    removeStar: false,
    /// 客户端分组筛选
    groupFilter: 'all' as RankGroupFilter,
    isLoading: false,
    /// 实时刷新是否开启
    isLive: false,
    error: null as string | null,
    /// 最近一次成功刷新时间（ms），用于展示「x 秒前更新」
    lastUpdated: 0,

    /// 轮询上下文（供轮询任务复用；登出时随 $reset 清理）
    contestId: '',
    uid: null as string | null,
  }),

  getters: {
    /** 按分组过滤后的行（打星/女生为客户端过滤） */
    visibleRows: (state): ContestRankRow[] => {
      switch (state.groupFilter) {
        case 'star':
          return state.rows.filter((row) => row.rank === -1)
        case 'female':
          return state.rows.filter((row) => row.gender === 'female')
        default:
          return state.rows
      }
    },

    /** 是否已加载过榜单（用于区分「空榜单」与「尚未请求」） */
    hasData: (state) => state.rows.length > 0 || state.total > 0,
  },

  actions: {
    /**
     * 加载一页榜单。
     *
     * 失败时记录 error 并向上抛出，由调用方决定是否提示；
     * 认证类错误（401）已由全局会话守卫统一处理，此处不重复应对。
     */
    async loadRank(contestId: string, uid: string | null, page?: number) {
      this.isLoading = true
      this.error = null
      this.contestId = contestId
      this.uid = uid
      try {
        const result = await rankService.getRank({
          contestId,
          currentPage: page ?? this.current,
          keyword: this.keyword || null,
          removeStar: this.removeStar,
        })
        this.applyPage(result, uid)
      } catch (e) {
        this.error = e instanceof Error ? e.message : '加载榜单失败'
        throw e
      } finally {
        this.isLoading = false
      }
    },

    /**
     * 应用一页榜单数据。
     *
     * 三处归一必须在此完成，视图层不再重复处理：
     * 1. 按 uid 去重（服务端把当前用户/关注用户前置复制了一份）
     * 2. 从**未去重**的原始 records 中定位我的行
     * 3. 参与人数用 total 减去本页重复行数修正
     */
    applyPage(page: ContestRankPage, uid: string | null) {
      this.rows = dedupeRankRows(page.records)
      this.myRow = resolveMyRow(page.records, uid)
      this.total = page.total
      this.size = page.size
      this.current = page.current
      this.pages = page.pages
      this.participants = resolveParticipantCountFromPage(page)
      this.lastUpdated = Date.now()
    },

    /** 轮询用刷新：吞掉异常（错误已记录在 error），避免打断轮询节奏 */
    async refresh() {
      if (!this.contestId) return
      try {
        await this.loadRank(this.contestId, this.uid, this.current)
      } catch {
        // 瞬时失败保留上一次数据，等待下一周期
      }
    },

    /** 翻页 */
    async setPage(page: number) {
      if (page < 1 || (this.pages > 0 && page > this.pages)) return
      await this.loadRank(this.contestId, this.uid, page)
    },

    /** 设置搜索关键词并回到第 1 页（服务端会在全量排名上重新过滤，total 随之变化） */
    async setKeyword(keyword: string) {
      this.keyword = keyword.trim()
      await this.loadRank(this.contestId, this.uid, 1)
    },

    /**
     * 切换分组筛选。
     *
     * `official` 是服务端参数（removeStar），需要重新请求；
     * `star` / `female` 服务端无对应参数，走客户端过滤（visibleRows），无需请求。
     */
    async setGroupFilter(filter: RankGroupFilter) {
      this.groupFilter = filter
      const removeStar = filter === 'official'
      if (removeStar === this.removeStar) return
      this.removeStar = removeStar
      await this.loadRank(this.contestId, this.uid, 1)
    },

    /**
     * 开启实时刷新（10s ± 2s 抖动）。
     *
     * 暂停判据：页面隐藏（`document.hidden`）或调用方给出的条件（如比赛已结束）。
     * 首次数据由调用方自行 `loadRank` 拉取 —— 轮询器的第一次执行在一个完整周期之后，
     * 这样进入页面能立刻看到数据而不是等 10 秒。
     *
     * @param isPaused 额外暂停判据（比赛已结束、离线等）
     */
    startLive(isPaused?: () => boolean) {
      this.stopLive()
      poller = createPoller({
        task: () => this.refresh(),
        intervalMs: RANK_POLL_INTERVAL_MS,
        jitterMs: RANK_POLL_JITTER_MS,
        isPaused: () => isPageHidden() || (isPaused?.() ?? false),
        onError: (e) => {
          this.error = e instanceof Error ? e.message : '榜单刷新失败'
        },
      })
      poller.start()
      this.isLive = true
    },

    /** 停止实时刷新（离开榜单页、比赛结束、登出时调用） */
    stopLive() {
      poller?.stop()
      poller = null
      this.isLive = false
    },
  },
})
