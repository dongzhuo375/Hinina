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
     * 把当前列表中所有未读公告标记为已读（进入公告页时调用）。
     *
     * 先乐观更新本地（红点立即消失），再持久化；持久化失败回滚本地集合并记录 ——
     * 宁可红点复发（下次进入页面会再次尝试），不可让「已读」只存在于内存造成误导。
     */
    async markAllRead() {
      const contestId = this.contestId
      if (!contestId) return
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
    },

    /** 停止实时刷新（离开工作台、比赛结束、登出时调用） */
    stopLive() {
      poller?.stop()
      poller = null
      this.isLive = false
    },
  },
})
