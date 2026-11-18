import { describe, expect, it, vi } from 'vitest'
import { listen } from '@tauri-apps/api/event'

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}))

import { onWorkspaceSaved } from '@/bridge/workspace.bridge'
import type { WorkspaceSavedPayload } from '@/bridge/workspace.bridge'

const listenMock = vi.mocked(listen)

/// 捕获 listen 注册的回调，模拟事件桥下发；返回触达 handler 的载荷记录
function installSubscriber(): (payload: unknown) => WorkspaceSavedPayload[] {
  let callback: ((event: { payload: unknown }) => void) | null = null
  const unlisten = vi.fn()
  const received: WorkspaceSavedPayload[] = []
  listenMock.mockImplementation(async (channel, cb) => {
    expect(channel).toBe('workspace-saved')
    callback = cb as (event: { payload: unknown }) => void
    return unlisten as unknown as () => void
  })
  void onWorkspaceSaved((payload) => {
    received.push(payload)
  })
  return (payload: unknown) => {
    if (!callback) throw new Error('listen 未被调用')
    callback({ payload })
    return received
  }
}

describe('workspaceBridge.onWorkspaceSaved — bridge 层载荷形状校验', () => {
  it('合法载荷（revision 恒携带）透传给 handler', () => {
    const emit = installSubscriber()
    const valid = { workspaceId: 'ws-1', revision: 3, auto: false }

    const received = emit(valid)

    expect(received).toEqual([valid])
  })

  it('payload 非对象（null / 字符串）整事件丢弃', () => {
    const emit = installSubscriber()

    expect(emit(null)).toEqual([])
    expect(emit('workspace-saved')).toEqual([])
  })

  it('workspaceId / revision / auto 任一字段形态不符时丢弃', () => {
    const emit = installSubscriber()
    const valid = { workspaceId: 'ws-1', revision: 3, auto: false }

    expect(emit({ ...valid, revision: undefined })).toEqual([])
    expect(emit({ ...valid, revision: '3' })).toEqual([])
    expect(emit({ ...valid, auto: 'false' })).toEqual([])
    expect(emit({ ...valid, workspaceId: 7 })).toEqual([])
  })
})
