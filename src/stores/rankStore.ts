import { defineStore } from 'pinia'
import type { ContestRankPage, ContestRankRow } from '@/types/rank'
import { DEFAULT_RANK_PAGE_SIZE, rankService } from '@/services/rank.service'
import { createPoller } from '@/utils/polling'
import type { Poller } from '@/utils/polling'
import {
  dedupeRankRows,
  filterRankRowsByGroup,
  mergeRankPages,
  paginateRankRows,
  resolveMyRow,
  resolveParticipantCountFromPage,
} from '@/utils/rank'
import type { RankGroupFilter } from '@/utils/rank'

/// 榜单轮询节奏：HOJ 文档 §9.9 要求间隔 ≥10s 且切后台暂停；
/// 抖动用于打散全场客户端的同步相位（同一秒进入榜单页时不会每周期都齐发）
const RANK_POLL_INTERVAL_MS = 10_000
const RANK_POLL_JITTER_MS = 2_000

/// 全量快照拉取上限：40 页 × 50 行 = 2000 行。超限说明比赛规模远超客户端筛选
/// 的合理承载，截断并在 UI 明示「结果可能不完整」，避免赛场上无限拉页打爆 OJ
const FULL_FETCH_MAX_PAGES = 40
const FULL_FETCH_MAX_ROWS = 2_000

/// 榜单分组筛选（判据与过滤纯函数见 `utils/rank`）
export type { RankGroupFilter }

/// 是否属于需要全量快照的客户端筛选（star/female 服务端无对应参数）
function isFullSnapshotFilter(filter: RankGroupFilter): boolean {
  return filter === 'star' || filter === 'female'
}

/// 全量快照拉取状态：idle 未拉取 / loading 拉取中 / done 完整 /
/// truncated 达到上限被截断 / error 失败（无可用快照）
export type FullFetchState = 'idle' | 'loading' | 'done' | 'truncated' | 'error'

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

    // ── 全量快照模式（star/female 跨页过滤）──
    /// 全量榜单快照（跨页按 uid 去重合并）；非全量模式为 null
    fullRows: null as ContestRankRow[] | null,
    /// 快照拉取状态
    fullFetchState: 'idle' as FullFetchState,
    /// 快照模式下客户端分页的当前页（映射到 `current` 供视图无感消费）
    fullPage: 1,
    /// 快照已加载行数（去重后），用于状态行「已加载 N 行」
    fullLoadedRows: 0,
  }),

  getters: {
    /** 是否处于全量快照模式（star/female 筛选且快照已就位） */
    isFullMode: (state): boolean =>
      isFullSnapshotFilter(state.groupFilter) && state.fullRows !== null,

    /** 快照按当前分组过滤后的全部行（未分页） */
    fullFilteredRows: (state): ContestRankRow[] =>
      state.fullRows ? filterRankRowsByGroup(state.fullRows, state.groupFilter) : [],

    /**
     * 按分组过滤后的当前页可见行。
     *
     * 全量模式：过滤快照后做客户端分页（`pages`/`current` 已由 `syncFullPaging`
     * 同步为等效值，视图分页逻辑无需感知模式差异）；
     * 常规模式：`rows` 即服务端页数据，all/official 原样返回。
     * star/female 但快照未就位（拉取中/失败）时退化为筛当前页，聊胜于无。
     */
    visibleRows(): ContestRankRow[] {
      if (this.isFullMode) {
        return paginateRankRows(this.fullFilteredRows, this.fullPage, DEFAULT_RANK_PAGE_SIZE)
      }
      return filterRankRowsByGroup(this.rows, this.groupFilter)
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
     * 3. 参与人数按「total − 本页重复数 − 未被去重捕获的我的前置副本」修正
     */
    applyPage(page: ContestRankPage, uid: string | null) {
      this.rows = dedupeRankRows(page.records)
      this.myRow = resolveMyRow(page.records, uid)
      this.total = page.total
      this.size = page.size
      this.current = page.current
      this.pages = page.pages
      this.participants = resolveParticipantCountFromPage(page, uid)
      this.lastUpdated = Date.now()
    },

    /**
     * 全量快照拉取（star/female 跨页过滤的数据源）。
     *
     * 从第 1 页起顺序拉取并按 uid 跨页去重合并（服务端每页都会前置复制
     * 当前用户/关注用户），直到末页或触达上限（40 页 / 2000 行 → `truncated`）。
     * keyword 保持生效（每页请求都带上）；removeStar 恒为 false —— 快照必须
     * 包含打星行，否则「打星队」筛选恒为空。
     *
     * 失败时保留旧快照（若有）供继续浏览，仅在毫无快照时标记 `error`；
     * 异常向上抛出，由调用方决定提示方式（与 loadRank 一致）。
     */
    async fetchAllRows() {
      if (!this.contestId || this.fullFetchState === 'loading') return
      // 记住拉取前状态：失败且仍有旧快照时回退到它（不能停在 loading，否则重入保护会永久锁死）
      const previousState = this.fullFetchState
      this.fullFetchState = 'loading'
      this.isLoading = true
      this.error = null
      try {
        let merged: ContestRankRow[] = []
        let truncated = false
        for (let page = 1; ; page++) {
          const result = await rankService.getRank({
            contestId: this.contestId,
            currentPage: page,
            limit: DEFAULT_RANK_PAGE_SIZE,
            keyword: this.keyword || null,
            removeStar: false,
          })
          merged = mergeRankPages(merged, result.records)
          if (page >= result.pages) break
          if (page >= FULL_FETCH_MAX_PAGES || merged.length >= FULL_FETCH_MAX_ROWS) {
            truncated = true
            break
          }
        }
        this.fullRows = merged
        this.fullLoadedRows = merged.length
        this.fullFetchState = truncated ? 'truncated' : 'done'
        this.lastUpdated = Date.now()
        this.syncFullPaging()
      } catch (e) {
        // 有旧快照则保留并回退到拉取前状态供继续浏览；毫无快照才标记 error
        this.fullFetchState = this.fullRows === null ? 'error' : previousState
        this.error = e instanceof Error ? e.message : '拉取全量榜单失败'
        throw e
      } finally {
        this.isLoading = false
      }
    },

    /**
     * 全量模式下把客户端分页状态同步到 `pages`/`current`。
     *
     * RankView 的分页条直接读这两个字段，同步等效值后视图无需感知模式差异。
     */
    syncFullPaging() {
      const count = this.fullFilteredRows.length
      this.pages = Math.max(1, Math.ceil(count / DEFAULT_RANK_PAGE_SIZE))
      this.fullPage = Math.min(Math.max(1, this.fullPage), this.pages)
      this.current = this.fullPage
    },

    /** 轮询用刷新：吞掉异常（错误已记录在 error），避免打断轮询节奏 */
    async refresh() {
      if (!this.contestId) return
      try {
        if (isFullSnapshotFilter(this.groupFilter)) {
          // 全量快照太重，不进 10s 轮询（轮询器在全量模式下暂停）；
          // 这里只会被「手动刷新」触发，重新拉取整个快照
          await this.fetchAllRows()
        } else {
          await this.loadRank(this.contestId, this.uid, this.current)
        }
      } catch {
        // 瞬时失败保留上一次数据，等待下一周期
      }
    },

    /** 翻页（全量模式为纯客户端切片，不发请求） */
    async setPage(page: number) {
      if (page < 1 || (this.pages > 0 && page > this.pages)) return
      if (this.isFullMode) {
        this.fullPage = page
        this.current = page
        return
      }
      await this.loadRank(this.contestId, this.uid, page)
    },

    /** 设置搜索关键词并回到第 1 页（服务端会在全量排名上重新过滤，total 随之变化） */
    async setKeyword(keyword: string) {
      this.keyword = keyword.trim()
      if (isFullSnapshotFilter(this.groupFilter)) {
        // 全量模式：keyword 是快照拉取的请求参数，须重拉快照
        this.fullPage = 1
        await this.fetchAllRows()
        return
      }
      await this.loadRank(this.contestId, this.uid, 1)
    },

    /**
     * 切换分组筛选。
     *
     * - `official`：服务端参数（removeStar），重新请求第 1 页；
     * - `star` / `female`：服务端无对应参数，只筛当前页会跨页漏行，
     *   进入**全量快照模式**（fetchAllRows），star↔female 互切也重拉以获得新数据；
     * - 切回 `all` / `official`：清空快照，恢复服务端分页（轮询随之恢复）。
     */
    async setGroupFilter(filter: RankGroupFilter) {
      const previous = this.groupFilter
      this.groupFilter = filter

      if (isFullSnapshotFilter(filter)) {
        this.removeStar = false
        this.fullPage = 1
        await this.fetchAllRows()
        return
      }

      const wasFullMode = isFullSnapshotFilter(previous)
      this.fullRows = null
      this.fullFetchState = 'idle'
      this.fullPage = 1
      this.fullLoadedRows = 0
      const removeStar = filter === 'official'
      // 退出全量模式时 pages/current 已被客户端分页覆写，即使 removeStar 未变也须重载
      if (removeStar === this.removeStar && !wasFullMode) return
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
        isPaused: () =>
          isPageHidden() ||
          // 全量快照模式暂停自动轮询（整榜重拉太重），只保留手动刷新；
          // 切回 all/official 后轮询自动恢复
          isFullSnapshotFilter(this.groupFilter) ||
          (isPaused?.() ?? false),
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
