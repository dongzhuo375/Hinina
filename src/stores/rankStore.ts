import { defineStore } from 'pinia'
import type { ContestRankPage, ContestRankRow } from '@/types/rank'
import { DEFAULT_RANK_PAGE_SIZE, rankService } from '@/services/rank.service'
import { getSessionGeneration } from '@/stores/sessionGeneration'
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
import { errorMessage } from '@/utils/error'

/// 榜单轮询节奏：HOJ 文档 §9.9 要求间隔 ≥10s 且切后台暂停；
/// 抖动用于打散全场客户端的同步相位（同一秒进入榜单页时不会每周期都齐发）
const RANK_POLL_INTERVAL_MS = 10_000
const RANK_POLL_JITTER_MS = 2_000

/// 全量快照拉取上限：40 页 × 50 行 = 2000 行。超限说明比赛规模远超客户端筛选
/// 的合理承载，截断并在 UI 明示「结果可能不完整」，避免赛场上无限拉页打爆 OJ
const FULL_FETCH_MAX_PAGES = 40
const FULL_FETCH_MAX_ROWS = 2_000

/// 全量快照页间节流：HOJ 榜单页是「整榜全量重算后分页」，背靠背连发会让服务端
/// 连续做最多 40 次整榜计算 —— 这是全应用最重的突发路径，必须摊开：
/// 页间 400ms（打满上限也只多 ~16s，手动刷新场景无感知）
const FULL_FETCH_PAGE_DELAY_MS = 400

/// 用户操作路径（翻页 / 搜索 / 切筛选）的榜单查询 memo 有效期。
///
/// 只用于**去抖**：连点筛选或分页会让服务端背靠背重算整榜，而同一查询在数秒内
/// 重复发起的第二次结果必然相同。**轮询与手动刷新不走 memo** —— 榜单的实时性
/// 是公平性要求（`get_rank` 在后端也刻意不缓存）。
const RANK_QUERY_MEMO_TTL_MS = 3_000

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

/// 用户操作路径的查询 memo 与在途合并（模块级：非渲染状态，不进响应式系统）。
/// seq 是发起时的 loadSeq：出现更新的用户意图后，旧 memo/在途请求均不得再被复用
let queryMemo: { key: string; at: number; seq: number } | null = null
/// 在途合并记录代际与归属 token：跨会话不复用旧 promise，finally 只清自己的登记
/// （评审六轮：与 contestStore 的在途去重同类问题）
let queryInFlight: {
  key: string
  promise: Promise<void>
  generation: number
  token: object
  seq: number
} | null = null

/// 查询指纹：同一比赛 + 同一用户 + 同一页 + 同一关键词 + 同一 removeStar
/// 才算「同一查询」。含 uid 是为了避免换账号后误命中上一位用户的查询。
function rankQueryKey(
  contestId: string,
  uid: string | null,
  page: number,
  keyword: string,
  removeStar: boolean,
): string {
  return [contestId, uid ?? '', page, keyword, removeStar ? '1' : '0'].join('|')
}

/// 全量拉取代际令牌：每次 fetchAllRows 自增。切筛选/登出后旧循环经令牌与
/// groupFilter 双重校验作废，防止过期循环把旧快照写回（复活）或并发双写
let fullFetchGeneration = 0

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
    /// loadRank 请求序号：同会话内只认最新一次请求（含 dedup/memo 的复用归属）
    loadSeq: 0,
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
     * 加载一页榜单（带过期响应保护：同会话内只认最新一次请求，参数页码同步落地）。
     *
     * 最新请求失败时记录 error 并向上抛出，由调用方决定是否提示；
     * 认证类错误（401）已由全局会话守卫统一处理，此处不重复应对。
     */
    async loadRank(contestId: string, uid: string | null, page?: number) {
      // 跨比赛复用时 store 不会重建：旧榜单/快照/分页全属于上一场，必须在发起
      // 新比赛请求前清空并作废旧普通/全量请求，否则全量分支只会沿用旧 contestId
      const contestChanged = this.contestId !== '' && this.contestId !== contestId
      this.contestId = contestId
      this.uid = uid
      if (contestChanged) {
        fullFetchGeneration++
        this.loadSeq++
        this.rows = []
        this.myRow = null
        this.total = 0
        this.size = 0
        this.current = 1
        this.pages = 0
        this.participants = 0
        this.error = null
        this.lastUpdated = 0
        this.fullRows = null
        this.fullFetchState = 'idle'
        this.fullPage = 1
        this.fullLoadedRows = 0
        this.isLoading = false
      }
      // 重新进入榜单页时 RankView.bootstrap 也会直调本方法。仍处于全量筛选就必须
      // 继续走快照链路，否则普通分页会覆写全量分页，并与在途快照共享 loading 收尾
      if (isFullSnapshotFilter(this.groupFilter)) {
        await this.fetchAllRows()
        return
      }
      // 轮询 refresh 绕过 dedup 直调本方法，与用户翻页并发时，后完成的旧请求会
      // 把榜单整体拉回旧页且不自愈 —— 过期响应一律丢弃（与 openProblemSeq 同款范式）
      const token = ++this.loadSeq
      // 会话代际：登出后旧会话的榜单响应不得写回新会话（评审四轮 #1）
      const sessionGen = getSessionGeneration()
      this.isLoading = true
      this.error = null
      // 页码同步落地：refresh 轮询读 this.current 发请求；若等响应落地才更新，
      // 翻页在途期间触发的轮询会带着旧页号回来（序号更新反而把新页响应判过期）
      const previousPage = this.current
      if (page !== undefined) this.current = page
      try {
        const result = await rankService.getRank({
          contestId,
          currentPage: page ?? this.current,
          keyword: this.keyword || null,
          removeStar: this.removeStar,
        })
        if (token !== this.loadSeq || sessionGen !== getSessionGeneration()) return
        this.applyPage(result, uid)
      } catch (e) {
        // 调用方对过期加载早已放弃，重抛只会让视图把旧失败当成新失败提示
        if (token !== this.loadSeq || sessionGen !== getSessionGeneration()) return
        // 目标页请求失败时页面仍展示上一次成功数据，页码必须同步回退，
        // 否则会出现「第 1 页的行 + 第 2 页的页码」且没有后续请求自愈
        this.current = previousPage
        this.error = errorMessage(e, '加载榜单失败')
        throw e
      } finally {
        // 旧请求不得提前关掉新请求的加载态
        if (token === this.loadSeq && sessionGen === getSessionGeneration()) this.isLoading = false
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
      // 固定本次拉取的比赛上下文：循环中途换比赛时即使代际守卫失效，也不能逐页
      // 读取可变的 this.contestId 而拼出 A/B 混合快照
      const contestId = this.contestId
      // 记住拉取前状态：失败且仍有旧快照时回退到它（不能停在 loading，否则重入保护会永久锁死）
      const previousState = this.fullFetchState
      const generation = ++fullFetchGeneration
      // 普通分页与全量拉取共享 isLoading：进入全量模式同时作废旧普通请求，
      // 其迟到响应/finally 不得覆盖快照状态或提前结束全量加载
      this.loadSeq++
      // 会话代际：登出后旧会话的全量快照不得写回新会话（评审四轮 #1）
      const sessionGen = getSessionGeneration()
      this.fullFetchState = 'loading'
      this.isLoading = true
      this.error = null
      try {
        let merged: ContestRankRow[] = []
        let truncated = false
        for (let page = 1; ; page++) {
          const result = await rankService.getRank({
            contestId,
            currentPage: page,
            limit: DEFAULT_RANK_PAGE_SIZE,
            keyword: this.keyword || null,
            removeStar: false,
          })
          // 拉取期间已被更新的拉取取代、用户已退出全量模式（切筛选/登出 $reset），
          // 或会话已切换：中止且不写快照 —— 过期数据复活比缺一次刷新更糟
          if (
            generation !== fullFetchGeneration ||
            !isFullSnapshotFilter(this.groupFilter) ||
            sessionGen !== getSessionGeneration()
          ) {
            return
          }
          merged = mergeRankPages(merged, result.records)
          if (page >= result.pages) break
          if (page >= FULL_FETCH_MAX_PAGES || merged.length >= FULL_FETCH_MAX_ROWS) {
            truncated = true
            break
          }
          // 页间节流（见 FULL_FETCH_PAGE_DELAY_MS 注释）；节流窗口同样是中止检查点
          await sleep(FULL_FETCH_PAGE_DELAY_MS)
          if (
            generation !== fullFetchGeneration ||
            !isFullSnapshotFilter(this.groupFilter) ||
            sessionGen !== getSessionGeneration()
          ) {
            return
          }
        }
        this.fullRows = merged
        this.fullLoadedRows = merged.length
        this.fullFetchState = truncated ? 'truncated' : 'done'
        this.lastUpdated = Date.now()
        this.syncFullPaging()
      } catch (e) {
        // 已被新拉取取代或已退出全量模式：旧失败不得覆盖新状态，也不得重抛给
        // 早已放弃这次加载的调用方；否则会把新拉取的 loading 覆盖成 error
        if (generation !== fullFetchGeneration || sessionGen !== getSessionGeneration()) return
        // 有旧快照则保留并回退到拉取前状态供继续浏览；毫无快照才标记 error
        this.fullFetchState = this.fullRows === null ? 'error' : previousState
        this.error = errorMessage(e, '拉取全量榜单失败')
        throw e
      } finally {
        // 只有当前代际负责收尾共享 loading（旧拉取不得关掉新普通/全量请求）
        if (generation === fullFetchGeneration && sessionGen === getSessionGeneration()) {
          this.isLoading = false
        }
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
      await this.loadRankDeduped(page)
    },

    /**
     * 用户操作路径的榜单加载：同查询 in-flight 合并 + 短 TTL memo。
     *
     * 轮询（`refresh`）与手动刷新**不经过这里**：前者每 10s±2s 必须真打服务端，
     * 后者是用户显式要求真值。这里只处理「同一查询在数秒内被重复发起」——
     * 连点分页/筛选/搜索会命中合并或 memo，避免服务端背靠背重算整榜。
     */
    async loadRankDeduped(page: number) {
      // 快照尚未就位时 isFullMode=false，翻页可能落到这里；同样不得改走普通分页，
      // 更不能让一次被重入保护短路的调用写入普通查询 memo
      if (isFullSnapshotFilter(this.groupFilter)) {
        await this.fetchAllRows()
        return
      }
      const key = rankQueryKey(this.contestId, this.uid, page, this.keyword, this.removeStar)
      const generation = getSessionGeneration()

      // 只复用同会话、且仍是最新意图的在途查询；直接 loadRank 也会推进 loadSeq，
      // 因此旧登记即使在 key 相同的情况下也不能截获更新后的用户操作
      if (
        queryInFlight?.key === key &&
        queryInFlight.generation === generation &&
        queryInFlight.seq === this.loadSeq
      ) {
        return queryInFlight.promise
      }

      if (
        queryMemo?.key === key &&
        queryMemo.seq === this.loadSeq &&
        Date.now() - queryMemo.at < RANK_QUERY_MEMO_TTL_MS
      ) {
        return
      }

      const token = {}
      const request = this.loadRank(this.contestId, this.uid, page)
      const seq = this.loadSeq
      const promise = request
        .then(() => {
          // 过期失败会被 loadRank 静默吞掉，因此 memo 必须以序号确认这次确实是最新成功；
          // 会话已切换时同样不得给新会话留下「已命中」
          if (seq === this.loadSeq && generation === getSessionGeneration()) {
            queryMemo = { key, at: Date.now(), seq }
          }
        })
        .finally(() => {
          if (queryInFlight?.token === token) queryInFlight = null
        })
      queryInFlight = { key, promise, generation, token, seq }
      return promise
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
      await this.loadRankDeduped(1)
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
      // 退出本身也是一次“取代”：即使旧循环尚未走到下一个中止检查点，
      // 其 catch/finally 也已失去写回资格，不能污染恢复后的普通分页
      if (wasFullMode) fullFetchGeneration++
      this.fullRows = null
      this.fullFetchState = 'idle'
      this.fullPage = 1
      this.fullLoadedRows = 0
      const removeStar = filter === 'official'
      // 退出全量模式时 pages/current 已被客户端分页覆写，即使 removeStar 未变也须重载
      if (removeStar === this.removeStar && !wasFullMode) return
      this.removeStar = removeStar
      await this.loadRankDeduped(1)
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
      // 进入榜单页 = 新的一次会话上下文：清掉上一次的查询 memo/在途记录，
      // 避免「登出 → 重新登录 → 首次翻页」因 memo 命中而跳过请求（界面停空）
      queryMemo = null
      queryInFlight = null
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
          this.error = errorMessage(e, '榜单刷新失败')
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
      // 查询去抖 memo 只在「停留榜单页期间」有意义：离开页面后下一次进入是新上下文，
      // 保留 memo 会让「登出 → 重新登录 → 首次翻页」跳过请求（界面停空）
      queryMemo = null
      queryInFlight = null
    },
  },
})
