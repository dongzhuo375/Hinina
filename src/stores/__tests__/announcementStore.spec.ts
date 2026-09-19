// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import type { Announcement, AnnouncementPage } from '@/types/announcement'

/// Service 层打桩：store 只依赖 service，测试不触达 IPC
const { announcementService } = vi.hoisted(() => ({
  announcementService: {
    listAnnouncements: vi.fn(),
    getReadIds: vi.fn(),
    markRead: vi.fn(),
  },
}))
vi.mock('@/services/announcement.service', () => ({ announcementService }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

import { useAnnouncementStore } from '@/stores/announcementStore'

const CONTEST_ID = '1'

function makeAnnouncement(id: string, over: Partial<Announcement> = {}): Announcement {
  return {
    id,
    title: `公告 ${id}`,
    content: '内容',
    author: 'admin',
    createdAt: 1_700_000_000,
    updatedAt: 1_700_000_000,
    ...over,
  }
}

function makePage(records: Announcement[]): AnnouncementPage {
  return { records, total: records.length, size: 100, current: 1, pages: 1 }
}

/// 覆盖 `document.hidden`（jsdom 默认 false = 可见）
function setPageHidden(hidden: boolean): void {
  Object.defineProperty(document, 'hidden', { configurable: true, get: () => hidden })
}

/// 复位为「可见」。刻意不用 `delete`：jsdom 把 `hidden` 定义在原型上，
/// ESM 严格模式下删除失败会抛 TypeError，反而污染所有用例
function restorePageVisibility(): void {
  setPageHidden(false)
}

beforeEach(() => {
  setActivePinia(createPinia())
  vi.clearAllMocks()
  restorePageVisibility()
  vi.spyOn(console, 'error').mockImplementation(() => {})
  vi.spyOn(console, 'warn').mockImplementation(() => {})
})

afterEach(() => {
  restorePageVisibility()
})

describe('load — 公告列表与已读集合', () => {
  it('成功时写入列表、总数与已读 ID', async () => {
    announcementService.listAnnouncements.mockResolvedValue(
      makePage([makeAnnouncement('a1'), makeAnnouncement('a2')]),
    )
    announcementService.getReadIds.mockResolvedValue(new Set(['a1']))
    const store = useAnnouncementStore()

    await store.load(CONTEST_ID)

    expect(store.announcements).toHaveLength(2)
    expect(store.total).toBe(2)
    expect(store.readIds).toEqual(['a1'])
    expect(store.isLoading).toBe(false)
    expect(store.error).toBeNull()
  })

  it('已读集合拉取失败降级为全部未读（宁可多显红点，不可漏报公告）', async () => {
    announcementService.listAnnouncements.mockResolvedValue(makePage([makeAnnouncement('a1')]))
    announcementService.getReadIds.mockRejectedValue(new Error('磁盘读取失败'))
    const store = useAnnouncementStore()

    await store.load(CONTEST_ID)

    expect(store.readIds).toEqual([])
    expect(store.unreadCount).toBe(1)
  })

  it('列表拉取失败写入 error 并抛出', async () => {
    announcementService.listAnnouncements.mockRejectedValue(new Error('网络异常'))
    announcementService.getReadIds.mockResolvedValue(new Set())
    const store = useAnnouncementStore()

    await expect(store.load(CONTEST_ID)).rejects.toThrow('网络异常')
    expect(store.error).toBe('网络异常')
    expect(store.isLoading).toBe(false)
  })
})

describe('unreadCount / isUnread — 未读判定（ActivityBar 红点数据源）', () => {
  it('未读数 = 列表中不在已读集合的条数', async () => {
    announcementService.listAnnouncements.mockResolvedValue(
      makePage([makeAnnouncement('a1'), makeAnnouncement('a2'), makeAnnouncement('a3')]),
    )
    announcementService.getReadIds.mockResolvedValue(new Set(['a2']))
    const store = useAnnouncementStore()

    await store.load(CONTEST_ID)

    expect(store.unreadCount).toBe(2)
    expect(store.isUnread('a1')).toBe(true)
    expect(store.isUnread('a2')).toBe(false)
  })
})

describe('markAllRead — 进入公告页全部已读', () => {
  it('乐观更新本地集合，持久化成功后以后端合并结果为准', async () => {
    announcementService.listAnnouncements.mockResolvedValue(
      makePage([makeAnnouncement('a1'), makeAnnouncement('a2')]),
    )
    announcementService.getReadIds.mockResolvedValue(new Set())
    announcementService.markRead.mockResolvedValue(new Set(['a1', 'a2']))
    const store = useAnnouncementStore()
    await store.load(CONTEST_ID)

    await store.markAllRead()

    expect(announcementService.markRead).toHaveBeenCalledWith(CONTEST_ID, ['a1', 'a2'])
    expect(store.readIds.sort()).toEqual(['a1', 'a2'])
    expect(store.unreadCount).toBe(0)
  })

  it('持久化失败回滚本地标记（红点复发优于假已读）', async () => {
    announcementService.listAnnouncements.mockResolvedValue(makePage([makeAnnouncement('a1')]))
    announcementService.getReadIds.mockResolvedValue(new Set())
    announcementService.markRead.mockRejectedValue(new Error('磁盘写入失败'))
    const store = useAnnouncementStore()
    await store.load(CONTEST_ID)

    await store.markAllRead()

    expect(store.readIds).toEqual([])
    expect(store.unreadCount).toBe(1)
  })

  it('无未读时不调用后端', async () => {
    announcementService.listAnnouncements.mockResolvedValue(makePage([makeAnnouncement('a1')]))
    announcementService.getReadIds.mockResolvedValue(new Set(['a1']))
    const store = useAnnouncementStore()
    await store.load(CONTEST_ID)

    await store.markAllRead()

    expect(announcementService.markRead).not.toHaveBeenCalled()
  })

  it('未加载比赛（contestId 为空）时静默返回', async () => {
    const store = useAnnouncementStore()

    await store.markAllRead()

    expect(announcementService.markRead).not.toHaveBeenCalled()
  })

  it('页面不可见时不标记已读（切走了 = 没看到，标记等于吞掉红点）', async () => {
    announcementService.listAnnouncements.mockResolvedValue(makePage([makeAnnouncement('a1')]))
    announcementService.getReadIds.mockResolvedValue(new Set())
    const store = useAnnouncementStore()
    await store.load(CONTEST_ID)

    setPageHidden(true)
    await store.markAllRead()

    expect(announcementService.markRead).not.toHaveBeenCalled()
    expect(store.unreadCount).toBe(1)
  })
})

describe('isWatching — 用户正在看公告页时的已读语义', () => {
  it('正在看时，load 落地的新公告自动标为已读（避免离开页面后冒出假红点）', async () => {
    announcementService.listAnnouncements.mockResolvedValue(makePage([makeAnnouncement('a1')]))
    announcementService.getReadIds.mockResolvedValue(new Set())
    announcementService.markRead.mockResolvedValue(new Set(['a1']))
    const store = useAnnouncementStore()
    store.isWatching = true

    await store.load(CONTEST_ID)
    // markAllRead 由 load 内部触发（异步），等它落地
    await vi.waitFor(() => expect(announcementService.markRead).toHaveBeenCalledWith(CONTEST_ID, ['a1']))
    expect(store.unreadCount).toBe(0)
  })

  it('不在公告页时不标记：新公告必须保持未读以点亮红点', async () => {
    announcementService.listAnnouncements.mockResolvedValue(makePage([makeAnnouncement('a1')]))
    announcementService.getReadIds.mockResolvedValue(new Set())
    const store = useAnnouncementStore()
    store.isWatching = false

    await store.load(CONTEST_ID)

    expect(announcementService.markRead).not.toHaveBeenCalled()
    expect(store.unreadCount).toBe(1)
  })

  it('正在看但页面不可见时不标记（切走期间落地的不算看过）', async () => {
    announcementService.listAnnouncements.mockResolvedValue(makePage([makeAnnouncement('a1')]))
    announcementService.getReadIds.mockResolvedValue(new Set())
    const store = useAnnouncementStore()
    store.isWatching = true

    setPageHidden(true)
    await store.load(CONTEST_ID)

    expect(announcementService.markRead).not.toHaveBeenCalled()
  })
})

describe('refresh / startLive / stopLive — 轮询编排', () => {
  it('refresh 吞掉异常（错误已记录），不打断轮询', async () => {
    announcementService.listAnnouncements.mockRejectedValue(new Error('网络异常'))
    announcementService.getReadIds.mockResolvedValue(new Set())
    const store = useAnnouncementStore()
    store.contestId = CONTEST_ID

    await expect(store.refresh()).resolves.toBeUndefined()
    expect(store.error).toBe('网络异常')
  })

  it('startLive 立即拉取一次并按周期刷新；stopLive 后不再请求', async () => {
    vi.useFakeTimers()
    try {
      announcementService.listAnnouncements.mockResolvedValue(makePage([]))
      announcementService.getReadIds.mockResolvedValue(new Set())
      const store = useAnnouncementStore()

      store.startLive(CONTEST_ID)
      expect(store.isLive).toBe(true)
      // 首次数据立即拉取（红点不应等一个轮询周期才出现）
      await vi.advanceTimersByTimeAsync(0)
      expect(announcementService.listAnnouncements).toHaveBeenCalledTimes(1)

      // 轮询周期 60s±10s：推进 71s 必然已触发
      await vi.advanceTimersByTimeAsync(71_000)
      expect(announcementService.listAnnouncements.mock.calls.length).toBeGreaterThanOrEqual(2)

      const callsAfterStop = announcementService.listAnnouncements.mock.calls.length
      store.stopLive()
      expect(store.isLive).toBe(false)
      await vi.advanceTimersByTimeAsync(300_000)
      expect(announcementService.listAnnouncements).toHaveBeenCalledTimes(callsAfterStop)
    } finally {
      vi.useRealTimers()
    }
  })

  it('重复 startLive 不会产生多个轮询器', async () => {
    vi.useFakeTimers()
    try {
      announcementService.listAnnouncements.mockResolvedValue(makePage([]))
      announcementService.getReadIds.mockResolvedValue(new Set())
      const store = useAnnouncementStore()

      store.startLive(CONTEST_ID)
      store.startLive(CONTEST_ID)
      store.startLive(CONTEST_ID)
      const initialCalls = announcementService.listAnnouncements.mock.calls.length

      await vi.advanceTimersByTimeAsync(71_000)
      // 三个轮询器并存时同一周期会发出三倍请求
      expect(announcementService.listAnnouncements.mock.calls.length).toBeLessThanOrEqual(
        initialCalls + 1,
      )
      store.stopLive()
    } finally {
      vi.useRealTimers()
    }
  })

  it('窗口重新可见/聚焦时立即补拉一次（切回窗口不该等一整个周期）', async () => {
    // 桌面客户端的常态是「切出去看题解，再切回来」；只靠 60s 节拍意味着
    // 切回来最多要等 70s 才可能看到红点，被选手直接感知为「红点不出现」
    vi.useFakeTimers()
    try {
      announcementService.listAnnouncements.mockResolvedValue(makePage([]))
      announcementService.getReadIds.mockResolvedValue(new Set())
      const store = useAnnouncementStore()

      store.startLive(CONTEST_ID)
      await vi.advanceTimersByTimeAsync(0)
      const before = announcementService.listAnnouncements.mock.calls.length

      // 越过模块级去重窗口（同一秒内 visibilitychange + focus 只补拉一次）。
      // 窗口状态是模块级的、跨用例保留，故推进量要明显大于窗口而非刚好越过
      await vi.advanceTimersByTimeAsync(5_000)
      window.dispatchEvent(new Event('focus'))
      expect(announcementService.listAnnouncements.mock.calls.length).toBe(before + 1)

      store.stopLive()
    } finally {
      vi.useRealTimers()
    }
  })

  it('visibilitychange 与 focus 同时到达时只补拉一次（去重）', async () => {
    vi.useFakeTimers()
    try {
      announcementService.listAnnouncements.mockResolvedValue(makePage([]))
      announcementService.getReadIds.mockResolvedValue(new Set())
      const store = useAnnouncementStore()

      store.startLive(CONTEST_ID)
      await vi.advanceTimersByTimeAsync(0)
      const before = announcementService.listAnnouncements.mock.calls.length

      await vi.advanceTimersByTimeAsync(5_000)
      window.dispatchEvent(new Event('focus'))
      document.dispatchEvent(new Event('visibilitychange'))

      expect(announcementService.listAnnouncements.mock.calls.length).toBe(before + 1)
      store.stopLive()
    } finally {
      vi.useRealTimers()
    }
  })

  it('页面仍不可见时不补拉（后台不产生无谓请求）', async () => {
    vi.useFakeTimers()
    try {
      announcementService.listAnnouncements.mockResolvedValue(makePage([]))
      announcementService.getReadIds.mockResolvedValue(new Set())
      const store = useAnnouncementStore()

      store.startLive(CONTEST_ID)
      await vi.advanceTimersByTimeAsync(0)
      const before = announcementService.listAnnouncements.mock.calls.length

      setPageHidden(true)
      await vi.advanceTimersByTimeAsync(5_000)
      window.dispatchEvent(new Event('focus'))

      expect(announcementService.listAnnouncements.mock.calls.length).toBe(before)
      store.stopLive()
    } finally {
      vi.useRealTimers()
    }
  })

  it('stopLive 后不再响应可见性补拉（离开工作台必须回收监听）', async () => {
    vi.useFakeTimers()
    try {
      announcementService.listAnnouncements.mockResolvedValue(makePage([]))
      announcementService.getReadIds.mockResolvedValue(new Set())
      const store = useAnnouncementStore()

      store.startLive(CONTEST_ID)
      await vi.advanceTimersByTimeAsync(0)
      store.stopLive()
      const before = announcementService.listAnnouncements.mock.calls.length

      await vi.advanceTimersByTimeAsync(5_000)
      window.dispatchEvent(new Event('focus'))

      expect(announcementService.listAnnouncements.mock.calls.length).toBe(before)
    } finally {
      vi.useRealTimers()
    }
  })
})
