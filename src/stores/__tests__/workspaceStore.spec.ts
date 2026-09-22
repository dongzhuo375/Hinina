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
import type { Workspace } from '@/types/workspace'

const CODE = 'int main() { return 0; }'

beforeEach(() => {
  setActivePinia(createPinia())
  vi.clearAllMocks()
  vi.spyOn(console, 'error').mockImplementation(() => {})
  // 模块级落盘修订号是副作用句柄（跨用例存活），先归零让每个用例独立
  useWorkspaceStore().cancelPendingSync()
  workspaceService.updateWorkspaceFile.mockResolvedValue(undefined)
  workspaceService.saveWorkspace.mockResolvedValue(undefined)
  workspaceService.setLanguage.mockResolvedValue({
    id: 'ws-1',
    contestId: '1',
    problemId: 'p1',
    rootPath: '',
    files: {},
    activeFile: null,
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

  it('推送在途时又敲键：不清 syncPending，且新内容仍会被下次防抖推送', async () => {
    vi.useFakeTimers()
    try {
      // 推送挂起，模拟 IPC 在途
      let releasePush: () => void = () => {}
      workspaceService.updateWorkspaceFile.mockImplementation(
        () =>
          new Promise<void>((resolve) => {
            releasePush = () => resolve()
          }),
      )

      const store = useWorkspaceStore()
      store.updateCode(CODE)
      const flushing = store.flushPendingSync()

      // 推送在途时用户继续敲键
      store.updateCode(`${CODE}\n// 又改了`)
      releasePush()
      await flushing

      // 本次推送只带走了旧内容 → 不能清标记（否则新内容既不被本次携带、
      // 又被下次防抖短路跳过，永远到不了后端）
      expect(store.syncPending).toBe(true)

      // 新内容由防抖推送补上
      workspaceService.updateWorkspaceFile.mockResolvedValue(undefined)
      vi.advanceTimersByTime(2_000)
      await vi.runOnlyPendingTimersAsync()
      expect(workspaceService.updateWorkspaceFile).toHaveBeenLastCalledWith(
        'main.cpp',
        `${CODE}\n// 又改了`,
      )
      expect(store.syncPending).toBe(false)
    } finally {
      vi.useRealTimers()
    }
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

  it('落盘 IPC 在途时又敲键：不清脏标记（避免假「已自动备份」）', async () => {
    // 落盘挂起，模拟 IPC 在途（失焦/路由切换保存与用户仍在敲键同时发生）
    let releaseSave: () => void = () => {}
    workspaceService.saveWorkspace.mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          releaseSave = () => resolve()
        }),
    )

    const store = useWorkspaceStore()
    store.updateCode(CODE)
    const saving = store.saveWorkspace()
    await vi.waitFor(() => expect(workspaceService.saveWorkspace).toHaveBeenCalled())

    // 落盘在途时用户继续敲键：新改动连后端内存都还没到
    store.updateCode(`${CODE}\n// 又改了`)
    releaseSave()
    await saving

    expect(store.syncPending).toBe(true)
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
      activeFile: null,
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

  it('同一落盘事件重复送达时幂等：不会重复生效', () => {
    const store = useWorkspaceStore()
    store.updateCode(CODE)
    store.syncPending = false

    store.markPersisted('ws-1', 3)
    expect(store.isDirty).toBe(false)

    // 再次送达同一事件（revision 相同）与更旧的 revision：都应被跳过
    store.updateCode(`${CODE}\n// 又改了`)
    store.syncPending = false
    store.markPersisted('ws-1', 3)
    store.markPersisted('ws-1', 2)

    expect(store.isDirty).toBe(true)
  })

  it('更新的 revision 仍能正常清除脏标记', () => {
    const store = useWorkspaceStore()
    store.updateCode(CODE)
    store.syncPending = false
    store.markPersisted('ws-1', 1)

    store.updateCode(`${CODE}\n// 又改了`)
    store.syncPending = false
    store.markPersisted('ws-1', 2)

    expect(store.isDirty).toBe(false)
  })

  it('事件修订号落后于已推送内容时不清脏（磁盘尚未追上编辑器）', async () => {
    // 推送被后端赋为修订号 5（编辑器最新内容在后端内存里是 revision 5）
    workspaceService.updateWorkspaceFile.mockResolvedValue(5)
    const store = useWorkspaceStore()
    store.updateCode(CODE)
    await store.flushPendingSync()
    expect(store.syncPending).toBe(false)

    // 后台 auto-save 落盘的是修订号 4 的旧快照（推送 5 之前的内存）：
    // 磁盘落后于编辑器，不能宣称「已自动备份」
    store.markPersisted('ws-1', 4)
    expect(store.isDirty).toBe(true)

    // 落盘追上（修订号 >= 已推送修订号）才清脏
    store.markPersisted('ws-1', 5)
    expect(store.isDirty).toBe(false)
  })

  it('推送返回时落盘事件尚未到达：不提前清脏（等事件到达再清）', async () => {
    // 锁定补判的方向：必须「水位 ≥ 推送」才清 —— 若误用「推送 ≥ 水位」，
    // 此处会在磁盘还落后时清脏（假「已自动备份」）
    workspaceService.updateWorkspaceFile.mockResolvedValue(7)
    const store = useWorkspaceStore()
    store.updateCode(CODE)
    await store.flushPendingSync()
    expect(store.syncPending).toBe(false)

    // 落盘事件还没到（磁盘仍在旧修订号）：不能清脏
    expect(store.isDirty).toBe(true)

    store.markPersisted('ws-1', 7)
    expect(store.isDirty).toBe(false)
  })

  it('落盘事件在推送在途到达时不卡指示器：flush 返回后补判清脏', async () => {
    // 推送挂起（模拟 IPC 在途），期间后端 auto-save 已落盘并送达事件
    let releasePush: (revision: number) => void = () => {}
    workspaceService.updateWorkspaceFile.mockImplementation(
      () =>
        new Promise<number>((resolve) => {
          releasePush = resolve
        }),
    )
    const store = useWorkspaceStore()
    store.updateCode(CODE)
    const flushing = store.flushPendingSync()

    // 事件先于推送返回到达：markPersisted 因 syncPending 推迟清脏（水位已推进）
    store.markPersisted('ws-1', 7)
    expect(store.isDirty).toBe(true)

    releasePush(7)
    await flushing

    // 后端已 clean、后续 auto-save tick 不再发事件 —— 若无补判，
    // 指示器将卡在「编辑中…」直到下一次编辑或显式保存
    expect(store.syncPending).toBe(false)
    expect(store.isDirty).toBe(false)
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
        activeFile: 'main.cpp',
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

describe('activeFile — 当前代码文件的权威源（P62）', () => {
  const loadFixture = (over: Partial<Workspace>): Workspace => ({
    id: 'ws-1',
    contestId: '1',
    problemId: 'p1',
    rootPath: '',
    files: {},
    activeFile: null,
    language: 'C++',
    isDirty: false,
    createdAt: 0,
    updatedAt: 0,
    ...over,
  })

  it('加载以 activeFile 为准，而不是按语言派生', async () => {
    // 语言说 Java、activeFile 指向 Main.java，而 files 里同时留着旧的 main.cpp：
    // 若按「语言派生名优先 + 后缀探测」（files 来自 HashMap 序列化、键序不稳定）
    // 就可能加载出旧 C++ 代码 + Java 元数据的组合 —— 提交即 CE
    workspaceService.loadWorkspace.mockResolvedValue(
      loadFixture({
        language: 'Java',
        activeFile: 'Main.java',
        files: { 'main.cpp': '// 旧 C++ 代码', 'Main.java': 'class Main {}' },
      }),
    )

    const store = useWorkspaceStore()
    await store.loadWorkspace('1', 'p1')

    expect(store.activeFile).toBe('Main.java')
    expect(store.code).toBe('class Main {}')
    expect(store.language).toBe('Java')
  })

  it('写入落到 activeFile 上（不再按语言重新派生）', async () => {
    workspaceService.loadWorkspace.mockResolvedValue(
      loadFixture({ language: 'Java', activeFile: 'Main.java', files: { 'Main.java': 'x' } }),
    )
    const store = useWorkspaceStore()
    await store.loadWorkspace('1', 'p1')

    store.updateCode('class Main { int i; }')
    await store.flushPendingSync()

    expect(workspaceService.updateWorkspaceFile).toHaveBeenCalledWith(
      'Main.java',
      'class Main { int i; }',
    )
  })

  it('历史工作区（无 activeFile）回退到「语言派生名优先」', async () => {
    workspaceService.loadWorkspace.mockResolvedValue(
      loadFixture({
        language: 'Python',
        activeFile: null,
        files: { 'main.cpp': 'old', 'main.py': 'new' },
      }),
    )
    const store = useWorkspaceStore()
    await store.loadWorkspace('1', 'p1')

    expect(store.activeFile).toBe('main.py')
    expect(store.code).toBe('new')
  })

  it('语言元数据与代码文件扩展名矛盾时以文件为准并告警', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
    workspaceService.loadWorkspace.mockResolvedValue(
      loadFixture({
        language: 'Java',
        activeFile: 'main.cpp',
        files: { 'main.cpp': 'int main() {}' },
      }),
    )
    const store = useWorkspaceStore()
    await store.loadWorkspace('1', 'p1')

    // 判题端按后缀判定语言与 limits 倍率，故以 main.cpp 为准
    expect(store.language).toBe('C++')
    expect(store.activeFile).toBe('main.cpp')
    expect(warn).toHaveBeenCalled()
    warn.mockRestore()
  })

  it('切换语言后 activeFile 跟着走，写入锚定新文件', async () => {
    workspaceService.loadWorkspace.mockResolvedValue(
      loadFixture({ language: 'C++', activeFile: 'main.cpp', files: { 'main.cpp': 'x' } }),
    )
    const store = useWorkspaceStore()
    await store.loadWorkspace('1', 'p1')

    store.changeLanguage('Python')

    expect(store.activeFile).toBe('main.py')
    await vi.waitFor(() =>
      expect(workspaceService.updateWorkspaceFile).toHaveBeenCalledWith('main.py', 'x'),
    )
  })
})
