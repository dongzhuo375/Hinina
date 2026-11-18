import { describe, expect, it, vi } from 'vitest'
import { listen } from '@tauri-apps/api/event'

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}))

import {
  ANNOUNCEMENTS_PUBLISHED_EVENT,
  onAnnouncementsPublished,
} from '@/bridge/announcement.bridge'
import type { AnnouncementsPublishedPayload } from '@/bridge/announcement.bridge'

const listenMock = vi.mocked(listen)

/// 捕获 listen 注册的回调，模拟事件桥下发；返回触达 handler 的载荷记录
function installSubscriber(): (payload: unknown) => AnnouncementsPublishedPayload[] {
  let callback: ((event: { payload: unknown }) => void) | null = null
  const unlisten = vi.fn()
  const received: AnnouncementsPublishedPayload[] = []
  listenMock.mockImplementation(async (channel, cb) => {
    expect(channel).toBe(ANNOUNCEMENTS_PUBLISHED_EVENT)
    callback = cb as (event: { payload: unknown }) => void
    return unlisten as unknown as () => void
  })
  void onAnnouncementsPublished((payload) => {
    received.push(payload)
  })
  return (payload: unknown) => {
    if (!callback) throw new Error('listen 未被调用')
    callback({ payload })
    return received
  }
}

describe('announcementBridge.onAnnouncementsPublished — bridge 层载荷形状校验', () => {
  it('合法载荷透传给 handler', () => {
    const emit = installSubscriber()

    const received = emit({ contestId: '1001', newIds: ['702'] })

    expect(received).toEqual([{ contestId: '1001', newIds: ['702'] }])
  })

  it('payload 非对象（null / 字符串）或 contestId 非 string 时整事件丢弃', () => {
    const emit = installSubscriber()

    expect(emit(null)).toEqual([])
    expect(emit('announcements-published')).toEqual([])
    expect(emit({ contestId: 7, newIds: [] })).toEqual([])
    expect(emit({ newIds: ['702'] })).toEqual([])
  })

  it('newIds 缺省或非数组归一为空数组（不透传畸形形态）', () => {
    expect(installSubscriber()({ contestId: '1001' })).toEqual([{ contestId: '1001', newIds: [] }])
    expect(installSubscriber()({ contestId: '1001', newIds: '702' })).toEqual([
      { contestId: '1001', newIds: [] },
    ])
  })
})
