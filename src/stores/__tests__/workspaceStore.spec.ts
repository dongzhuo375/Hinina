import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

/// Service 层打桩：store 只依赖 service，测试不触达 IPC
const { workspaceService, configService } = vi.hoisted(() => ({
  workspaceService: {
    loadWorkspace: vi.fn(),
    saveWorkspace: vi.fn(),
    currentWorkspace: vi.fn(),
    updateWorkspaceFile: vi.fn(),
    setLanguage: vi.fn(),
    onWorkspaceSaved: vi.fn(),
  },
  configService: {
    getDefaultLanguage: vi.fn(),
  },
}))
vi.mock('@/services/workspace.service', () => ({ workspaceService }))
vi.mock('@/services/config.service', () => ({ configService }))

import { useWorkspaceStore } from '@/stores/workspaceStore'

const CODE = 'int main() { return 0; }'

beforeEach(() => {
  setActivePinia(createPinia())
  vi.clearAllMocks()
  vi.spyOn(console, 'error').mockImplementation(() => {})
  workspaceService.updateWorkspaceFile.mockResolvedValue(undefined)
  workspaceService.saveWorkspace.mockResolvedValue(undefined)
  workspaceService.setLanguage.mockResolvedValue({
    id: 'ws-1',
    contestId: '1',
    problemId: 'p1',
    rootPath: '',
    files: {},
    language: 'C++',
    isDirty: false,
    createdAt: 0,
    updatedAt: 0,
  })
})

describe('updateCode / flushPendingSync — 防抖只推内存', () => {
  it('编辑后立即标记两级状态：未推送 + 未落盘', () => {
    const store = useWorkspaceStore()
    store.updateCode(CODE)

    expect(store.code).toBe(CODE)
    expect(store.syncPending).toBe(true)
    expect(store.isDirty).toBe(true)
  })

  it('2 秒防抖后推送到后端内存（此时仍算未落盘）', async () => {
    vi.useFakeTimers()
    try {
      const store = useWorkspaceStore()
      store.updateCode(CODE)
      expect(workspaceService.updateWorkspaceFile).not.toHaveBeenCalled()

      vi.advanceTimersByTime(2_000)
      await vi.runOnlyPendingTimersAsync()

      expect(workspaceService.updateWorkspaceFile).toHaveBeenCalledWith('main.cpp', CODE)
      expect(store.syncPending).toBe(false)
      // 只推到内存 ≠ 落盘：脏标记保持，直到后端落盘事件到达
      expect(store.isDirty).toBe(true)
    } finally {
      vi.useRealTimers()
    }
  })

  it('flushPendingSync 取消防抖窗口并立即推送（切题 / 关窗用）', async () => {
    vi.useFakeTimers()
    try {
      const store = useWorkspaceStore()
      store.updateCode(CODE)

      await expect(store.flushPendingSync()).resolves.toBe(true)

      expect(workspaceService.updateWorkspaceFile).toHaveBeenCalledTimes(1)
      expect(store.syncPending).toBe(false)
      // 防抖定时器已取消：时间推进不应再推送第二次
      vi.advanceTimersByTime(5_000)
      await vi.runOnlyPendingTimersAsync()
      expect(workspaceService.updateWorkspaceFile).toHaveBeenCalledTimes(1)
    } finally {
      vi.useRealTimers()
    }
  })

  it('无在途改动时 flushPendingSync 不发请求', async () => {
    const store = useWorkspaceStore()
    await expect(store.flushPendingSync()).resolves.toBe(true)
    expect(workspaceService.updateWorkspaceFile).not.toHaveBeenCalled()
  })

  it('推送失败保留 syncPending（内容仍在编辑器，下次重试）', async () => {
    workspaceService.updateWorkspaceFile.mockRejectedValue(new Error('IPC 故障'))
    const store = useWorkspaceStore()
    store.updateCode(CODE)

    await expect(store.flushPendingSync()).resolves.toBe(false)

    expect(store.syncPending).toBe(true)
    expect(store.isDirty).toBe(true)
  })
})

describe('saveWorkspace — 先推内存再落盘', () => {
  it('在途改动先推送再落盘（顺序反了会把旧内容写进磁盘）', async () => {
    const order: string[] = []
    workspaceService.updateWorkspaceFile.mockImplementation(async () => {
      order.push('push')
    })
    workspaceService.saveWorkspace.mockImplementation(async () => {
      order.push('save')
    })

    const store = useWorkspaceStore()
    store.updateCode(CODE)
    await store.saveWorkspace()

    expect(order).toEqual(['push', 'save'])
    expect(store.isDirty).toBe(false)
  })

  it('推送失败时保留脏标记（内容未进后端，不能宣称已保存）', async () => {
    workspaceService.updateWorkspaceFile.mockRejectedValue(new Error('IPC 故障'))
    const store = useWorkspaceStore()
    store.updateCode(CODE)

    await store.saveWorkspace()

    expect(store.isDirty).toBe(true)
  })
})

describe('markPersisted — 后端落盘事件驱动指示器', () => {
  it('事件到达且无新改动时清除脏标记（含后台 auto-save）', () => {
    const store = useWorkspaceStore()
    store.updateCode(CODE)
    store.syncPending = false // 已推送、等待落盘

    store.markPersisted()

    expect(store.isDirty).toBe(false)
  })

  it('事件到达后又有新改动时不清除（最新内容尚未落盘）', () => {
    const store = useWorkspaceStore()
    store.updateCode(CODE)
    store.syncPending = false
    store.updateCode(`${CODE}\n// 又改了`) // 事件与本次改动竞争

    store.markPersisted()

    expect(store.isDirty).toBe(true)
  })

  it('过期事件（属于旧工作区）不误清新工作区的脏标记', () => {
    const store = useWorkspaceStore()
    store.workspace = {
      id: 'ws-new',
      contestId: '1',
      problemId: 'p2',
      rootPath: '',
      files: {},
      language: 'C++',
      isDirty: false,
      createdAt: 0,
      updatedAt: 0,
    }
    store.updateCode(CODE)
    store.syncPending = false

    store.markPersisted('ws-old') // 切题前保存旧工作区的事件迟到

    expect(store.isDirty).toBe(true)
  })
})

describe('loadWorkspace — 替换 store 状态前先推送在途改动', () => {
  it('先 flush 再加载：避免加载结果覆盖 code 后被旧内容回推（切视图往返丢代码）', async () => {
    const order: string[] = []
    workspaceService.updateWorkspaceFile.mockImplementation(async () => {
      order.push('push')
    })
    workspaceService.loadWorkspace.mockImplementation(async () => {
      order.push('load')
      return {
        id: 'ws-1',
        contestId: '1',
        problemId: 'p1',
        rootPath: '',
        files: { 'main.cpp': CODE },
        language: 'C++',
        isDirty: false,
        createdAt: 0,
        updatedAt: 0,
      }
    })

    const store = useWorkspaceStore()
    store.updateCode(CODE)
    await store.loadWorkspace('1', 'p1')

    expect(order).toEqual(['push', 'load'])
    expect(workspaceService.updateWorkspaceFile).toHaveBeenCalledWith('main.cpp', CODE)
    expect(store.code).toBe(CODE)
    expect(store.syncPending).toBe(false)
  })
})

describe('cancelPendingSync — 登出丢弃在途改动', () => {
  it('取消后时间推进不再推送（不向已失效会话写代码）', async () => {
    vi.useFakeTimers()
    try {
      const store = useWorkspaceStore()
      store.updateCode(CODE)
      store.cancelPendingSync()

      expect(store.syncPending).toBe(false)
      vi.advanceTimersByTime(5_000)
      await vi.runOnlyPendingTimersAsync()
      expect(workspaceService.updateWorkspaceFile).not.toHaveBeenCalled()
    } finally {
      vi.useRealTimers()
    }
  })
})

describe('changeLanguage — 语言即时落盘，不计入未落盘的代码改动', () => {
  it('切换语言不置脏标记（语言由后端立即持久化）', async () => {
    const store = useWorkspaceStore()
    store.changeLanguage('Python')

    expect(store.language).toBe('Python')
    expect(store.isDirty).toBe(false)
    expect(workspaceService.setLanguage).toHaveBeenCalledWith('Python')

    // 派生文件名变化触发的代码同步是「推送到内存」，完成后清 syncPending
    await vi.waitFor(() => expect(store.syncPending).toBe(false))
  })

  it('派生文件名变化时把代码同步到新文件名（判题端按后缀判语言）', async () => {
    const store = useWorkspaceStore()
    store.code = CODE
    store.changeLanguage('Python')

    expect(workspaceService.updateWorkspaceFile).toHaveBeenCalledWith('main.py', CODE)
  })

  it('同名短路：同一语言重复设置不触发任何请求', () => {
    const store = useWorkspaceStore()
    store.changeLanguage('C++')

    expect(workspaceService.setLanguage).not.toHaveBeenCalled()
    expect(workspaceService.updateWorkspaceFile).not.toHaveBeenCalled()
  })
})
