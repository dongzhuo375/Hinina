import { beforeEach, describe, expect, it, vi } from 'vitest'
import { IpcError, parseAppError } from '@/bridge'
import type { AppConfig } from '@/types/config'

/// Bridge 层打桩：service 测试不触达 IPC（逐层隔离约定）
const { getConfig, updateConfig, reloadConfig, switchOj } = vi.hoisted(() => ({
  getConfig: vi.fn(),
  updateConfig: vi.fn(),
  reloadConfig: vi.fn(),
  switchOj: vi.fn(),
}))
vi.mock('@/bridge/config.bridge', () => ({ getConfig, updateConfig, reloadConfig, switchOj }))

const { isKeyed } = vi.hoisted(() => ({ isKeyed: vi.fn() }))
vi.mock('@/services/system.service', () => ({ systemService: { isKeyed } }))

import { ConfigService, configService } from '@/services/config.service'

function makeConfig(over: Partial<AppConfig> = {}): AppConfig {
  return {
    user: { lastUsername: 'team01' },
    oj: {
      active: 'HOJ',
      instances: [{ id: 'HOJ', baseUrl: 'https://hoj.example.com', enabled: true, options: {} }],
      contestRef: '7',
      contestPassword: null,
      timeoutSecs: 30,
      pollIntervalSecs: 2,
      pollTimeoutSecs: 300,
      cacheTtlSecs: 60,
      cacheProblemStatement: true,
    },
    editor: {
      fontSize: 14,
      tabSize: 4,
      autoSave: true,
      autoSaveIntervalSecs: 30,
      defaultLanguage: 'C++',
    },
    theme: { themeName: 'light', editorTheme: 'vs' },
    layout: { sidebarWidth: 280, splitRatio: 0.48 },
    ...over,
  }
}

beforeEach(() => {
  vi.clearAllMocks()
  // 默认普通启动（维护者层锁定 → maintainerLocked 推导为 true）
  isKeyed.mockReturnValue(false)
  // 单例服务的进程内缓存必须逐用例清空，否则相互串扰
  configService.invalidate()
  vi.spyOn(console, 'error').mockImplementation(() => {})
})

describe('getConfig — 进程内缓存', () => {
  it('并发/重复读取共享同一次 IPC', async () => {
    getConfig.mockResolvedValue(makeConfig())

    const [a, b] = await Promise.all([configService.getConfig(), configService.getConfig()])
    await configService.getConfig()

    expect(getConfig).toHaveBeenCalledTimes(1)
    expect(a).toBe(b)
  })

  it('失败时清空缓存以便下次重试', async () => {
    getConfig.mockRejectedValueOnce(new Error('IPC 失败'))
    await expect(configService.getConfig()).rejects.toThrow('IPC 失败')

    getConfig.mockResolvedValue(makeConfig())
    await expect(configService.getConfig()).resolves.toBeDefined()
    expect(getConfig).toHaveBeenCalledTimes(2)
  })
})

describe('getEditorPrefs / getDefaultLanguage / getSplitRatio — 派生参数与兜底', () => {
  it('合法配置原样透传', async () => {
    const config = makeConfig()
    config.editor.fontSize = 16
    config.editor.tabSize = 2
    config.editor.defaultLanguage = 'Java'
    config.theme.editorTheme = 'vs-dark'
    config.layout.splitRatio = 0.55
    getConfig.mockResolvedValue(config)

    expect(await configService.getEditorPrefs()).toEqual({
      fontSize: 16,
      tabSize: 2,
      editorTheme: 'vs-dark',
    })
    expect(await configService.getDefaultLanguage()).toBe('Java')
    expect(await configService.getSplitRatio()).toBe(0.55)
  })

  it('历史配置遗留 Monaco id "cpp" 归一为 HOJ 显示名', async () => {
    const config = makeConfig()
    config.editor.defaultLanguage = 'cpp'
    getConfig.mockResolvedValue(config)

    expect(await configService.getDefaultLanguage()).toBe('C++')
  })

  it('越界/非法值回退兜底（字号 14、Tab 4、主题 vs、分栏 0.48）', async () => {
    const config = makeConfig()
    config.editor.fontSize = 999
    config.editor.tabSize = -1
    config.theme.editorTheme = 'dracula'
    config.layout.splitRatio = 5
    getConfig.mockResolvedValue(config)

    expect(await configService.getEditorPrefs()).toEqual({
      fontSize: 14,
      tabSize: 4,
      editorTheme: 'vs',
    })
    expect(await configService.getSplitRatio()).toBe(0.48)
  })

  it('读取失败全部回退兜底值，不向上抛错', async () => {
    getConfig.mockRejectedValue(new Error('IPC 失败'))

    expect(await configService.getEditorPrefs()).toEqual({
      fontSize: 14,
      tabSize: 4,
      editorTheme: 'vs',
    })
    expect(await configService.getDefaultLanguage()).toBe('C++')
    expect(await configService.getSplitRatio()).toBe(0.48)
  })
})

describe('updateEditorPrefs — 解题页编辑器设置落盘入口', () => {
  it('只写传入字段，其余配置整体保留（读-改-写）', async () => {
    getConfig.mockResolvedValue(makeConfig())
    updateConfig.mockResolvedValue(undefined)

    await configService.updateEditorPrefs({ fontSize: 18 })

    const written = updateConfig.mock.calls[0][0] as AppConfig
    expect(written.editor.fontSize).toBe(18)
    // 未传入的偏好与其它分组不得被冲掉
    expect(written.editor.tabSize).toBe(4)
    expect(written.editor.autoSaveIntervalSecs).toBe(30)
    expect(written.theme.editorTheme).toBe('vs')
    expect(written.oj.contestRef).toBe('7')
    expect(written.oj.instances[0].baseUrl).toBe('https://hoj.example.com')
  })

  it('编辑器主题落在 theme.editorTheme，且不触碰 themeName（界面仍只有浅色）', async () => {
    getConfig.mockResolvedValue(makeConfig())
    updateConfig.mockResolvedValue(undefined)

    await configService.updateEditorPrefs({ editorTheme: 'vs-dark', tabSize: 2 })

    const written = updateConfig.mock.calls[0][0] as AppConfig
    expect(written.theme.editorTheme).toBe('vs-dark')
    expect(written.theme.themeName).toBe('light')
    expect(written.editor.tabSize).toBe(2)
  })

  it('非法主题名归一为默认值后再落盘（不让未知主题名进配置文件）', async () => {
    getConfig.mockResolvedValue(makeConfig())
    updateConfig.mockResolvedValue(undefined)

    await configService.updateEditorPrefs({ editorTheme: 'dracula' })

    const written = updateConfig.mock.calls[0][0] as AppConfig
    expect(written.theme.editorTheme).toBe('vs')
  })
})

describe('getOjBaseUrl / activeOjBaseUrl — 当前实例地址解析', () => {
  it('取 active 实例的 baseUrl；active 未命中时回退第一个启用实例', async () => {
    const config = makeConfig()
    config.oj.instances.push({ id: 'X', baseUrl: 'https://x.example.com', enabled: true, options: {} })
    config.oj.active = 'X'
    getConfig.mockResolvedValue(config)
    expect(await configService.getOjBaseUrl()).toBe('https://x.example.com')

    config.oj.active = 'MISSING'
    configService.invalidate()
    getConfig.mockResolvedValue(config)
    expect(await configService.getOjBaseUrl()).toBe('https://hoj.example.com')
  })

  it('禁用实例不参与 active 匹配与回退（enabled = 后端不注册）', async () => {
    const config = makeConfig()
    config.oj.instances = [
      { id: 'HOJ', baseUrl: 'https://hoj.example.com', enabled: false, options: {} },
      { id: 'X', baseUrl: 'https://x.example.com', enabled: true, options: {} },
    ]
    // active 指向禁用实例：回退第一个启用实例的地址
    config.oj.active = 'HOJ'
    getConfig.mockResolvedValue(config)
    expect(await configService.getOjBaseUrl()).toBe('https://x.example.com')
  })

  it('读取失败返回空串（缺基址只影响图片改写，不抛出）', async () => {
    getConfig.mockRejectedValue(new Error('IPC 失败'))
    await expect(configService.getOjBaseUrl()).resolves.toBe('')
  })
})

describe('switchOj — 显式切换当前 OJ', () => {
  it('调用 bridge 且切换后本地缓存失效（下次读取拿新 active）', async () => {
    switchOj.mockResolvedValue(undefined)
    getConfig.mockResolvedValue(makeConfig())
    await configService.getConfig() // 预热缓存
    expect(getConfig).toHaveBeenCalledTimes(1)

    await configService.switchOj('X')

    expect(switchOj).toHaveBeenCalledTimes(1)
    expect(switchOj).toHaveBeenCalledWith('X')
    // 缓存已失效：下次读取重新拉后端真值
    await configService.getConfig()
    expect(getConfig).toHaveBeenCalledTimes(2)
  })

  it('评审二十六 P1：切换已生效但持久化失败也失效缓存（否则旧 active 会被写回）', async () => {
    getConfig.mockResolvedValue(makeConfig())
    await configService.getConfig() // 预热缓存（旧 active）
    // 用真实 AppError 载荷经 parseAppError 构造 IpcError（与生产 IPC 出口同源）：
    // 手写 variant 会绕过白名单，生产上 variant 为 null、分支永不进入
    const payload = { OjSwitchPersistFailed: 'OJ 切换已生效但保存配置失败: 磁盘满' }
    const { variant, message } = parseAppError(payload)
    switchOj.mockRejectedValue(new IpcError('switch_oj', message, variant, payload))

    await expect(configService.switchOj('X')).rejects.toThrow('保存配置失败')

    // 缓存必须已失效：下一次读取重新拉后端（内存中 active 已是新 OJ）
    await configService.getConfig()
    expect(getConfig).toHaveBeenCalledTimes(2)
  })

  it('bridge 失败时原样上抛（设置页据此回滚下拉显示）', async () => {
    switchOj.mockRejectedValue(new Error('OJ X 未注册'))
    await expect(configService.switchOj('X')).rejects.toThrow('未注册')
  })
})

/// 并发基座的真实服务测试（评审三十六 P2）：useSettingsForm 的 mock 复刻了
/// epoch/pending，若只测 mock，删掉真实排队/代际仍会全绿。这里用独立实例与
/// 延迟 bridge promise 直接锁定 ConfigService 的行为。
describe('switchOj — 真实排队与代际', () => {
  function deferred<T = void>() {
    let resolve!: (value: T) => void
    let reject!: (reason?: unknown) => void
    const promise = new Promise<T>((res, rej) => {
      resolve = res
      reject = rej
    })
    return { promise, resolve, reject }
  }

  it('并发切换串行：第二次在第一次落地后才发 bridge；代际开始/结束各 +1', async () => {
    const svc = new ConfigService()
    const first = deferred()
    const second = deferred()
    switchOj.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)

    expect(svc.ojSwitchEpoch).toBe(0)
    const p1 = svc.switchOj('A')
    const p2 = svc.switchOj('B')
    expect(svc.ojSwitchEpoch).toBe(2) // 两次「开始」各 +1
    await Promise.resolve()
    expect(switchOj).toHaveBeenCalledTimes(1)
    expect(switchOj).toHaveBeenNthCalledWith(1, 'A')

    first.resolve()
    await p1
    expect(svc.ojSwitchEpoch).toBe(3)
    await Promise.resolve()
    expect(switchOj).toHaveBeenCalledTimes(2)
    expect(switchOj).toHaveBeenNthCalledWith(2, 'B')
    expect(svc.ojSwitchInFlight).toBe(true)

    second.resolve()
    await p2
    expect(svc.ojSwitchEpoch).toBe(4)
    expect(svc.ojSwitchInFlight).toBe(false)
  })

  it('whenOjSwitchSettled 覆盖队尾；队尾失败也解除等待', async () => {
    const svc = new ConfigService()
    const first = deferred()
    const second = deferred()
    switchOj.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)

    const p1 = svc.switchOj('A')
    const p2 = svc.switchOj('B')
    let settled = false
    const waiting = svc.whenOjSwitchSettled().then(() => {
      settled = true
    })

    first.resolve()
    await p1
    await Promise.resolve()
    expect(settled).toBe(false) // 队尾 B 仍在途

    second.reject(new Error('第二次失败'))
    await expect(p2).rejects.toThrow('第二次失败')
    await waiting
    expect(settled).toBe(true)
    expect(svc.ojSwitchInFlight).toBe(false)
  })

  it('记录最近一次生效结果；普通失败不覆盖 persist-failed', async () => {
    const svc = new ConfigService()
    switchOj.mockResolvedValueOnce(undefined)
    await svc.switchOj('A')
    expect(svc.latestOjSwitchResult()).toEqual({ target: 'A', outcome: 'success' })

    const payload = { OjSwitchPersistFailed: 'OJ 切换已生效但保存配置失败: 磁盘满' }
    const { variant, message } = parseAppError(payload)
    switchOj.mockRejectedValueOnce(new IpcError('switch_oj', message, variant, payload))
    await expect(svc.switchOj('B')).rejects.toThrow('保存配置失败')
    expect(svc.latestOjSwitchResult()).toEqual({ target: 'B', outcome: 'persist-failed' })

    switchOj.mockRejectedValueOnce(new Error('普通失败'))
    await expect(svc.switchOj('C')).rejects.toThrow('普通失败')
    expect(svc.latestOjSwitchResult()).toEqual({ target: 'B', outcome: 'persist-failed' })
  })

  it('任意 updateConfig 成功且 active 匹配时解除 persist-failed', async () => {
    const svc = new ConfigService()
    const payload = { OjSwitchPersistFailed: 'OJ 切换已生效但保存配置失败: 磁盘满' }
    const { variant, message } = parseAppError(payload)
    switchOj.mockRejectedValueOnce(new IpcError('switch_oj', message, variant, payload))
    await expect(svc.switchOj('X')).rejects.toThrow('保存配置失败')

    const config = makeConfig()
    config.oj.active = 'X'
    getConfig.mockResolvedValue(config)
    updateConfig.mockResolvedValue(undefined)
    await svc.updateConfig(() => {})

    expect(svc.latestOjSwitchResult()).toEqual({ target: 'X', outcome: 'success' })
  })

  it('保存的 active 不匹配时保留 persist-failed', async () => {
    const svc = new ConfigService()
    const payload = { OjSwitchPersistFailed: 'OJ 切换已生效但保存配置失败: 磁盘满' }
    const { variant, message } = parseAppError(payload)
    switchOj.mockRejectedValueOnce(new IpcError('switch_oj', message, variant, payload))
    await expect(svc.switchOj('X')).rejects.toThrow('保存配置失败')

    getConfig.mockResolvedValue(makeConfig()) // active=HOJ
    updateConfig.mockResolvedValue(undefined)
    await svc.updateConfig(() => {})

    expect(svc.latestOjSwitchResult()).toEqual({ target: 'X', outcome: 'persist-failed' })
  })

  it('以后端实际提交的 active 为准（可能与保存前快照不同）', async () => {
    const svc = new ConfigService()
    const payload = { OjSwitchPersistFailed: 'OJ 切换已生效但保存配置失败: 磁盘满' }
    const { variant, message } = parseAppError(payload)
    switchOj.mockRejectedValueOnce(new IpcError('switch_oj', message, variant, payload))
    await expect(svc.switchOj('Hydro')).rejects.toThrow('保存配置失败')

    getConfig.mockResolvedValue(makeConfig()) // 快照 active=HOJ
    const persisted = makeConfig()
    persisted.oj.active = 'Hydro'
    updateConfig.mockResolvedValue(persisted)
    const saved = await svc.updateConfig(() => {})

    // 后端写锁内实际落盘的是 Hydro：返回值与警告解除都按它判定
    expect(saved.oj.active).toBe('Hydro')
    expect(svc.latestOjSwitchResult()).toEqual({ target: 'Hydro', outcome: 'success' })
  })
})

describe('updateConfig — 设置页保存唯一入口', () => {
  it('读取当前配置 → 应用变更 → 整体写回 → 失效缓存', async () => {
    getConfig.mockResolvedValue(makeConfig())
    updateConfig.mockResolvedValue(undefined)

    const saved = await configService.updateConfig((draft) => {
      draft.editor.fontSize = 18
    })

    // 整体替换语义：未变更字段必须原样保留
    expect(updateConfig).toHaveBeenCalledTimes(1)
    const written = updateConfig.mock.calls[0][0] as AppConfig
    expect(written.editor.fontSize).toBe(18)
    expect(written.oj.contestRef).toBe('7')
    expect(saved.editor.fontSize).toBe(18)

    // 缓存已失效：下次读取重新拉后端真值
    getConfig.mockResolvedValue(makeConfig())
    await configService.getConfig()
    expect(getConfig).toHaveBeenCalledTimes(2)
  })

  it('评审二十七：未显式传参时按钥匙模式推导 maintainerLocked（普通启动=true）', async () => {
    getConfig.mockResolvedValue(makeConfig())
    updateConfig.mockResolvedValue(undefined)

    // 设置页之外的保存入口（编辑器偏好 / 分栏比例）都不传第二参数
    await configService.updateEditorPrefs({ fontSize: 18 })
    expect(updateConfig).toHaveBeenCalledWith(expect.any(Object), true)

    // 钥匙模式：维护者层可编辑，锁定语义关闭
    isKeyed.mockReturnValue(true)
    configService.invalidate()
    getConfig.mockResolvedValue(makeConfig())
    await configService.updateConfig((draft) => {
      draft.layout.splitRatio = 0.5
    })
    expect(updateConfig).toHaveBeenLastCalledWith(expect.any(Object), false)
  })

  it('评审二十六 P2：维护者层锁定把 maintainerLocked 透传给 bridge', async () => {
    getConfig.mockResolvedValue(makeConfig())
    updateConfig.mockResolvedValue(undefined)

    await configService.updateConfig((draft) => {
      draft.editor.fontSize = 18
    }, { maintainerLocked: true })

    expect(updateConfig).toHaveBeenCalledWith(expect.any(Object), true)
  })

  it('写回失败同样失效缓存（避免缓存与磁盘漂移）并向上抛错', async () => {
    getConfig.mockResolvedValue(makeConfig())
    updateConfig.mockRejectedValue(new Error('磁盘写入失败'))

    await expect(
      configService.updateConfig((draft) => {
        draft.editor.fontSize = 18
      }),
    ).rejects.toThrow('磁盘写入失败')

    getConfig.mockResolvedValue(makeConfig())
    await configService.getConfig()
    expect(getConfig).toHaveBeenCalledTimes(2)
  })

  it('变更作用于副本，不污染缓存中的当前配置', async () => {
    const current = makeConfig()
    getConfig.mockResolvedValue(current)
    updateConfig.mockResolvedValue(undefined)

    await configService.updateConfig((draft) => {
      draft.layout.splitRatio = 0.66
    })

    expect(current.layout.splitRatio).toBe(0.48)
  })
})
