import { defineStore } from 'pinia'
import type { Announcement } from '@/types/announcement'
import { announcementService } from '@/services/announcement.service'
import { createPoller } from '@/utils/polling'
import type { Poller } from '@/utils/polling'
import { errorMessage } from '@/utils/error'
import { createLogger } from '@/utils/logger'

const log = createLogger('announcementStore')

/// 公告刷新节奏：60s ± 10s。公告由裁判组低频发布，无需榜单级实时性；
/// 抖动打散全场客户端相位（见 utils/polling 头注释）
const ANNOUNCEMENT_POLL_INTERVAL_MS = 60_000
const ANNOUNCEMENT_POLL_JITTER_MS = 10_000

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

/// 窗口重新可见/聚焦时的补拉回调（模块作用域副作用句柄，与 poller 同款约定）
let visibilityRefresh: (() => void) | null = null

/// `visibilitychange` 与 `focus` 都可能触发，用时间窗去重，避免一次切回打两次请求
const VISIBILITY_REFRESH_DEDUPE_MS = 1_000
let lastVisibilityRefreshAt = 0

/**
 * 注册「窗口重新可见/聚焦 → 立即刷新」监听。
 *
 * 桌面客户端里「切回来」是最高频动作，而 `document.hidden` 从 true 变回 false
 * 时轮询器只是恢复排程，不会补发一次 —— 选手因此要等一整个周期才可能看到红点。
 * 事件触发点有两个（`visibilitychange` 与 `focus`），浏览器可能都触发，故去重。
 */
function installVisibilityRefresh(refresh: () => void): void {
  removeVisibilityRefresh()
  visibilityRefresh = () => {
    if (isPageHidden()) return
    const now = Date.now()
    if (now - lastVisibilityRefreshAt < VISIBILITY_REFRESH_DEDUPE_MS) return
    lastVisibilityRefreshAt = now
    refresh()
  }
  if (typeof document !== 'undefined') {
    document.addEventListener('visibilitychange', visibilityRefresh)
  }
  if (typeof window !== 'undefined') {
    window.addEventListener('focus', visibilityRefresh)
  }
}

/// 注销可见性补拉监听（幂等）
function removeVisibilityRefresh(): void {
  if (!visibilityRefresh) return
  if (typeof document !== 'undefined') {
    document.removeEventListener('visibilitychange', visibilityRefresh)
  }
  if (typeof window !== 'undefined') {
    window.removeEventListener('focus', visibilityRefresh)
  }
  visibilityRefresh = null
  // 复位去重时间戳：下一轮 startLive 的第一次可见性事件必须立即生效，
  // 不能被上一轮留下的时间戳挡住（也避免模块级状态在测试间泄漏）
  lastVisibilityRefreshAt = 0
}

/**
 * 公告 store — 比赛公告列表 + 客户端已读状态（HOJ 无已读概念，本地持久化）。
 *
 * 未读语义（产品决策）：
 * - ActivityBar 红点 = `unreadCount > 0`，由外壳启动的轮询在全部页面保持鲜活；
 * - 进入公告页即把当前列表全部标记已读（`markAllRead`），红点随之消失；
 * - 已读集合按「比赛 + 用户」由 Rust 端落盘，重启/重登后不复发红点。
 */
export const useAnnouncementStore = defineStore('announcement', {
  state: () => ({
    /// 公告列表（服务端时间倒序原样保留，视图不重排）
    announcements: [] as Announcement[],
    /// 已读公告 ID 集合（数组承载以保留响应式；判定走 getter）
    readIds: [] as string[],
    total: 0,
    isLoading: false,
    /// 实时刷新是否开启
    isLive: false,
    error: null as string | null,

    /// 轮询上下文（登出时随 $reset 清理）
    contestId: '',

    /// 公告页是否正处于「用户正在看」的状态（由 AnnouncementsView 挂载/卸载维护）。
    ///
    /// 语义：列表在屏幕上可见 = 用户已经看到 = 可以标为已读。
    /// 没有它，用户停留在公告页期间到达的新公告会一直挂着未读 ——
    /// 离开页面后突然冒出一个「新公告」红点，而内容其实早就看过了（假红点）。
    isWatching: false,
  }),

  getters: {
    /** 未读公告数（ActivityBar 红点徽标数据源） */
    unreadCount: (state): number => {
      const read = new Set(state.readIds)
      return state.announcements.filter((a) => !read.has(a.id)).length
    },

    /** 指定公告是否未读（列表圆点用；参数化 getter） */
    isUnread: (state) => {
      const read = new Set(state.readIds)
      return (id: string): boolean => !read.has(id)
    },

    /** 是否已加载过公告（区分「空列表」与「尚未请求」） */
    hasData: (state) => state.announcements.length > 0 || state.total > 0,
  },

  actions: {
    /**
     * 拉取公告列表与已读集合。
     *
     * 失败记录 error 并上抛，由调用方决定提示；轮询路径经 `refresh` 吞错。
     */
    async load(contestId: string) {
      this.isLoading = true
      this.error = null
      this.contestId = contestId
      try {
        // 列表与已读集合并行拉取；已读拉取失败降级为空集（宁可多显红点，不可漏报公告）
        const [page, readIds] = await Promise.all([
          announcementService.listAnnouncements(contestId),
          announcementService.getReadIds(contestId).catch((e) => {
            log.warn('读取已读状态失败，按全部未读处理:', e)
            return new Set<string>()
          }),
        ])
        this.announcements = page.records
        this.total = page.total
        this.readIds = [...readIds]
        // 用户正在看公告页 → 刚落地的这批就是「屏幕上已有的」，标为已读
        if (this.isWatching && !isPageHidden()) void this.markAllRead()
      } catch (e) {
        this.error = errorMessage(e, '加载公告失败')
        throw e
      } finally {
        this.isLoading = false
      }
    },

    /** 轮询用刷新：吞掉异常（错误已记录在 error），避免打断轮询节奏 */
    async refresh() {
      if (!this.contestId) return
      try {
        await this.load(this.contestId)
      } catch {
        // 瞬时失败保留上一次数据，等待下一周期
      }
    },

    /**
     * 把当前列表中所有未读公告标记为已读（进入公告页 / 公告页可见时刷新后调用）。
     *
     * **页面不可见时不标记**：用户切走了/最小化了，并没有「看到」这批公告，
     * 标记已读等于把红点吞掉 —— 等他切回来时既没有红点、也没意识到有新内容。
     *
     * 先乐观更新本地（红点立即消失），再持久化；持久化失败回滚本地集合并记录 ——
     * 宁可红点复发（下次进入页面会再次尝试），不可让「已读」只存在于内存造成误导。
     */
    async markAllRead() {
      const contestId = this.contestId
      if (!contestId) return
      // 不可见 = 没看到，不标记（见方法注释）
      if (isPageHidden()) return
      const read = new Set(this.readIds)
      const unreadIds = this.announcements.filter((a) => !read.has(a.id)).map((a) => a.id)
      if (unreadIds.length === 0) return

      const previous = this.readIds
      this.readIds = [...previous, ...unreadIds]
      try {
        const merged = await announcementService.markRead(contestId, unreadIds)
        this.readIds = [...merged]
      } catch (e) {
        this.readIds = previous
        log.error('已读状态持久化失败，已回滚本地标记:', e)
      }
    },

    /**
     * 开启实时刷新（60s ± 10s，页面隐藏暂停）。
     *
     * 由外壳 `ContestLayout` 在比赛数据就绪后启动，使红点在全部页面保持鲜活；
     * 首次数据由本方法立即拉取一次（红点不应等一个轮询周期才出现）。
     *
     * **窗口重新可见/聚焦时立即补拉一次**：客户端常态是「切出去看题解/记笔记，
     * 再切回来」。只靠 60s 节拍意味着切回来最多要等 70s 才可能看到红点 ——
     * 实测这被选手直接感知为「红点不出现，必须手动刷新页面」。补拉是幂等的
     * （`refresh` 内部吞错），且不改变 60s 的稳态节拍。
     *
     * @param isPaused 额外暂停判据（比赛已结束等）
     */
    startLive(contestId: string, isPaused?: () => boolean) {
      this.stopLive()
      this.contestId = contestId
      void this.refresh()
      poller = createPoller({
        task: () => this.refresh(),
        intervalMs: ANNOUNCEMENT_POLL_INTERVAL_MS,
        jitterMs: ANNOUNCEMENT_POLL_JITTER_MS,
        isPaused: () => isPageHidden() || (isPaused?.() ?? false),
        onError: (e) => {
          this.error = errorMessage(e, '公告刷新失败')
        },
      })
      poller.start()
      this.isLive = true
      installVisibilityRefresh(() => {
        if (isPaused?.() ?? false) return
        void this.refresh()
      })
    },

    /** 停止实时刷新（离开工作台、比赛结束、登出时调用） */
    stopLive() {
      poller?.stop()
      poller = null
      this.isLive = false
      removeVisibilityRefresh()
    },
  },
})
