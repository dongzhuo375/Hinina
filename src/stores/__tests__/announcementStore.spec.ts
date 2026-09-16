import { beforeEach, describe, expect, it, vi } from 'vitest'
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

beforeEach(() => {
  setActivePinia(createPinia())
  vi.clearAllMocks()
  vi.spyOn(console, 'error').mockImplementation(() => {})
  vi.spyOn(console, 'warn').mockImplementation(() => {})
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
})
