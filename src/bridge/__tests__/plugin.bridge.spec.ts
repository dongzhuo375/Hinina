import { describe, expect, it, vi } from 'vitest'
import { listen } from '@tauri-apps/api/event'

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}))

import { pluginBridge } from '@/bridge/plugin.bridge'
import type { PluginNotification } from '@/types/plugin'

const listenMock = vi.mocked(listen)

/// 捕获 listen 注册的回调，模拟事件桥下发；返回触达 handler 的载荷记录
function installSubscriber(): (payload: unknown) => PluginNotification[] {
  let callback: ((event: { payload: unknown }) => void) | null = null
  const unlisten = vi.fn()
  const received: PluginNotification[] = []
  listenMock.mockImplementation(async (channel, cb) => {
    expect(channel).toBe('plugin-notification')
    callback = cb as (event: { payload: unknown }) => void
    return unlisten as unknown as () => void
  })
  void pluginBridge.onNotification((notification) => {
    received.push(notification)
  })
  return (payload: unknown) => {
    if (!callback) throw new Error('listen 未被调用')
    callback({ payload })
    return received
  }
}

describe('pluginBridge.onNotification — bridge 层载荷形状校验', () => {
  it('合法载荷透传给 handler', () => {
    const emit = installSubscriber()
    const valid = { pluginId: 'balloon', level: 'info', message: 'hello' }

    const received = emit(valid)

    expect(received).toEqual([valid])
  })

  it('payload 非对象（null / 字符串）整事件丢弃', () => {
    const emit = installSubscriber()

    expect(emit(null)).toEqual([])
    expect(emit('plugin-notification')).toEqual([])
  })

  it('pluginId / message / level 任一字段缺失或非 string 时丢弃', () => {
    const emit = installSubscriber()
    const valid = { pluginId: 'balloon', level: 'info', message: 'hello' }

    expect(emit({ ...valid, pluginId: undefined })).toEqual([])
    expect(emit({ ...valid, pluginId: 7 })).toEqual([])
    expect(emit({ ...valid, message: undefined })).toEqual([])
    expect(emit({ ...valid, level: undefined })).toEqual([])
  })
})
