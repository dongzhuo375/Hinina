import { beforeEach, describe, expect, it, vi } from 'vitest'

/// vi.mock 会被提升到文件顶部，工厂内引用的变量必须经 vi.hoisted 声明
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke }))

import { forwardFrontendLog } from '@/bridge/diagnostics.bridge'

describe('forwardFrontendLog', () => {
  beforeEach(() => {
    invoke.mockReset()
  })

  it('经 ipcInvoke 透传 frontend_log 与三标量参数', async () => {
    invoke.mockResolvedValue(undefined)
    await expect(forwardFrontendLog('error', 'vue', 'boom')).resolves.toBeUndefined()
    expect(invoke).toHaveBeenCalledWith('frontend_log', {
      level: 'error',
      scope: 'vue',
      message: 'boom',
    })
  })

  it('后端失败时按约定拒绝（由调用方守卫吞掉，不在此处吞）', async () => {
    invoke.mockRejectedValue({ Unknown: 'frontend_log 不可用' })
    await expect(forwardFrontendLog('warn', 'window', 'x')).rejects.toMatchObject({
      cmd: 'frontend_log',
    })
  })
})
