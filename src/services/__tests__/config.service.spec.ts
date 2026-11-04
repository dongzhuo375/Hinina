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

import { ConfigService, configService, isOjSwitchPersistFailed } from '@/services/config.service'

function makeConfig(over: Partial<AppConfig> = {}): AppConfig {
  return {
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
    theme: { editorTheme: 'vs' },
    layout: { sidebarWidth: 280, splitRatio: 0.48 },
    ...over,
  }
}

/// 保存入口的测试便捷封装：生产只有 `runSaveTxn` 一个写入通道（不暴露
/// 队列化的 `updateConfig`），spec 统一从事务句柄进入，避免各写一遍箭头。
function save(
  svc: ConfigService,
  mutate: (draft: AppConfig) => void,
): Promise<AppConfig> {
  return svc.runSaveTxn((tx) => tx.updateConfig(mutate))
}

beforeEach(() => {
  vi.clearAllMocks()
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

  it('编辑器主题落在 theme.editorTheme（界面仍只有浅色）', async () => {
    getConfig.mockResolvedValue(makeConfig())
    updateConfig.mockResolvedValue(undefined)

    await configService.updateEditorPrefs({ editorTheme: 'vs-dark', tabSize: 2 })

    const written = updateConfig.mock.calls[0][0] as AppConfig
    expect(written.theme.editorTheme).toBe('vs-dark')
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

  it('切换已生效但持久化失败也失效缓存（否则旧 active 会被写回）', async () => {
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

describe('isOjSwitchPersistFailed — 类型化判据', () => {
  it('只认 IpcError 的持久化失败变体，裸 variant 对象不算', () => {
    const payload = { OjSwitchPersistFailed: 'OJ 切换已生效但保存配置失败: 磁盘满' }
    const { variant, message } = parseAppError(payload)
    expect(isOjSwitchPersistFailed(new IpcError('switch_oj', message, variant, payload))).toBe(true)
    expect(isOjSwitchPersistFailed(new Error('普通失败'))).toBe(false)
    // 旧判据 `(e as { variant? }).variant` 会把这种手写对象误判为「已生效」
    expect(isOjSwitchPersistFailed({ variant: 'OjSwitchPersistFailed' })).toBe(false)
  })

  it('裸 variant 对象按普通失败处理：不记录持久化失败、不失效缓存', async () => {
    const svc = new ConfigService()
    getConfig.mockResolvedValue(makeConfig())
    await svc.getConfig() // 预热缓存
    const fake = { variant: 'OjSwitchPersistFailed', message: '伪装' }
    switchOj.mockRejectedValueOnce(fake)

    await expect(svc.switchOj('X')).rejects.toEqual(fake)

    expect(svc.latestOjSwitchResult()).toBeNull()
    // 未走 persist-failed 分支的 invalidate：缓存仍命中
    await svc.getConfig()
    expect(getConfig).toHaveBeenCalledTimes(1)
  })
})

/// 并发基座的真实服务测试：useSettingsForm 的 mock 复刻了
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

  it('任意保存成功且 active 匹配时解除 persist-failed', async () => {
    const svc = new ConfigService()
    const payload = { OjSwitchPersistFailed: 'OJ 切换已生效但保存配置失败: 磁盘满' }
    const { variant, message } = parseAppError(payload)
    switchOj.mockRejectedValueOnce(new IpcError('switch_oj', message, variant, payload))
    await expect(svc.switchOj('X')).rejects.toThrow('保存配置失败')

    const config = makeConfig()
    config.oj.active = 'X'
    getConfig.mockResolvedValue(config)
    updateConfig.mockResolvedValue(undefined)
    await save(svc, () => {})

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
    await save(svc, () => {})

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
    const saved = await save(svc, () => {})

    // 后端写锁内实际落盘的是 Hydro：返回值与警告解除都按它判定
    expect(saved.oj.active).toBe('Hydro')
    expect(svc.latestOjSwitchResult()).toEqual({ target: 'Hydro', outcome: 'success' })
  })
})

describe('保存入口 — runSaveTxn 事务句柄（唯一写入通道）', () => {
  it('读取当前配置 → 应用变更 → 整体写回 → 失效缓存', async () => {
    getConfig.mockResolvedValue(makeConfig())
    updateConfig.mockResolvedValue(undefined)

    const saved = await save(configService, (draft) => {
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

  it('写回不携带锁定声明（锁定真值只在后端）', async () => {
    getConfig.mockResolvedValue(makeConfig())
    updateConfig.mockResolvedValue(undefined)

    await save(configService, (draft) => {
      draft.editor.fontSize = 18
    })

    // bridge 签名只有 config 一个入参：任何 IPC 载荷都不得再声明「是否锁定」
    expect(updateConfig).toHaveBeenCalledWith(expect.any(Object))
    expect(updateConfig.mock.calls[0]).toHaveLength(1)
  })

  it('写回失败同样失效缓存（避免缓存与磁盘漂移）并向上抛错', async () => {
    getConfig.mockResolvedValue(makeConfig())
    updateConfig.mockRejectedValue(new Error('磁盘写入失败'))

    await expect(
      save(configService, (draft) => {
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

    await save(configService, (draft) => {
      draft.layout.splitRatio = 0.66
    })

    expect(current.layout.splitRatio).toBe(0.48)
  })
})

/// save 门禁的真实服务测试（与 switchOj 排队套件同款方法：独立实例 + 延迟
/// bridge promise）。useSettingsForm 的 mock 复刻了这套机器，若只测 mock，
/// 删掉真实排队/代际仍会全绿——真实行为必须在这里锁定。
describe('save 门禁 — 真实排队与代际', () => {
  function deferred<T = void>() {
    let resolve!: (value: T) => void
    let reject!: (reason?: unknown) => void
    const promise = new Promise<T>((res, rej) => {
      resolve = res
      reject = rej
    })
    return { promise, resolve, reject }
  }

  /// 清空微任务队列：足够多跳之后仍未发生的事，只可能是被门禁挡住的工作
  ///（避免按微任务跳数硬编码断言——`getConfig().catch` 等包装会引入额外跳）
  async function flushMicrotasks() {
    for (let i = 0; i < 20; i++) await Promise.resolve()
  }

  it('并发保存串行：第二个事务在第一个落地后才发 bridge（读-改-写不交错）', async () => {
    const svc = new ConfigService()
    const first = deferred<AppConfig>()
    const second = deferred<AppConfig>()
    const order: string[] = []
    getConfig.mockResolvedValue(makeConfig())
    updateConfig.mockImplementationOnce(() => {
      order.push('bridge-1')
      return first.promise
    })
    updateConfig.mockImplementationOnce(() => {
      order.push('bridge-2')
      return second.promise
    })

    // 顺序断言记在**事务体内部**而不是调用方的 `.then`：后者的先后只取决于
    // Promise 包装层数（微任务跳数），不是契约。这里要锁的是「第二个事务的
    // 工作不得早于第一个事务结束」——去掉排队时 start-2 会先于 end-1。
    const p1 = svc.runSaveTxn(async (tx) => {
      order.push('start-1')
      await tx.updateConfig(() => {})
      order.push('end-1')
    })
    const p2 = svc.runSaveTxn(async (tx) => {
      order.push('start-2')
      await tx.updateConfig(() => {})
      order.push('end-2')
    })

    first.resolve(makeConfig())
    second.resolve(makeConfig())
    await Promise.all([p1, p2])

    // 去掉排队时 start-2 会先于 end-1 出现（并发读-改-写）
    expect(order).toEqual(['start-1', 'bridge-1', 'end-1', 'start-2', 'bridge-2', 'end-2'])
  })

  it('saveEpoch 入队/结束各 +1；saveInFlight 覆盖排队与在途，队尾落地才复位', async () => {
    const svc = new ConfigService()
    const first = deferred<AppConfig>()
    const second = deferred<AppConfig>()
    getConfig.mockResolvedValue(makeConfig())
    updateConfig.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)

    expect(svc.saveEpoch).toBe(0)
    expect(svc.saveInFlight.value).toBe(false)
    const p1 = save(svc, () => {})
    const p2 = save(svc, () => {})
    // 两次「入队」各 +1（同步；排队中的事务也算开始，读取方不得忽略）
    expect(svc.saveEpoch).toBe(2)
    expect(svc.saveInFlight.value).toBe(true)

    first.resolve(makeConfig())
    await p1
    expect(svc.saveEpoch).toBe(3)
    // 第二个事务仍在队里：busy 不得被第一个事务的结束复位
    expect(svc.saveInFlight.value).toBe(true)

    second.resolve(makeConfig())
    await p2
    expect(svc.saveEpoch).toBe(4)
    expect(svc.saveInFlight.value).toBe(false)
  })

  it('whenSaveSettled 覆盖队尾；队尾失败也解除等待', async () => {
    const svc = new ConfigService()
    const first = deferred<AppConfig>()
    const second = deferred<AppConfig>()
    getConfig.mockResolvedValue(makeConfig())
    updateConfig.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)

    const p1 = save(svc, () => {})
    const p2 = save(svc, () => {})
    let settled = false
    const waiting = svc.whenSaveSettled().then(() => {
      settled = true
    })

    first.resolve(makeConfig())
    await p1
    await Promise.resolve()
    expect(settled).toBe(false) // 队尾（第二个）仍在途

    second.reject(new Error('第二次失败'))
    await expect(p2).rejects.toThrow('第二次失败')
    await waiting
    expect(settled).toBe(true)
    expect(svc.saveInFlight.value).toBe(false)
  })

  it('runSaveTxn 持锁跨后续编排：事务外发起的第二保存（编辑器偏好/分栏比例同款）排队到事务落地', async () => {
    const svc = new ConfigService()
    const saveBridge = deferred<AppConfig>()
    const followUp = deferred<void>()
    const prefsBridge = deferred<AppConfig>()
    const order: string[] = []
    getConfig.mockResolvedValue(makeConfig())
    updateConfig.mockImplementationOnce(() => {
      order.push('settings-bridge')
      return saveBridge.promise
    })
    updateConfig.mockImplementationOnce(() => {
      order.push('prefs-bridge')
      return prefsBridge.promise
    })

    // 复合保存事务：保存 → 后续编排（对应「保存新建实例 → 自动切换」）
    const txn = svc.runSaveTxn(async (tx) => {
      await tx.updateConfig(() => {})
      order.push('settings-saved')
      await followUp.promise
      order.push('txn-done')
    })
    // 事务进行中发起的第二保存入口（编辑器偏好 / 分栏比例）
    const prefs = save(svc, () => {}).then(() => {
      order.push('prefs-done')
    })

    saveBridge.resolve(makeConfig())
    await flushMicrotasks()
    // 后续编排仍在途：第二入口不得插队发 bridge（去掉排队时这里就会出现 prefs-bridge）
    expect(order).toEqual(['settings-bridge', 'settings-saved'])

    followUp.resolve()
    prefsBridge.resolve(makeConfig())
    await Promise.all([txn, prefs])
    expect(order).toEqual(['settings-bridge', 'settings-saved', 'txn-done', 'prefs-bridge', 'prefs-done'])
  })

  it('runSaveTxn 失败也释放：代际 +1、busy 复位、等待解除、缓存仍失效', async () => {
    const svc = new ConfigService()
    getConfig.mockResolvedValue(makeConfig())
    await svc.getConfig() // 预热缓存
    updateConfig.mockRejectedValueOnce(new Error('写盘失败'))

    const epochBefore = svc.saveEpoch
    await expect(save(svc, () => {})).rejects.toThrow('写盘失败')

    expect(svc.saveEpoch).toBe(epochBefore + 2)
    expect(svc.saveInFlight.value).toBe(false)
    let settled = false
    await svc.whenSaveSettled().then(() => {
      settled = true
    })
    expect(settled).toBe(true)
    await svc.getConfig()
    expect(getConfig).toHaveBeenCalledTimes(2) // 失败后缓存已失效，重读后端
  })

  it('settingsSaveInFlight 只标记设置页事务：后台写入期间设置页门禁保持空闲', async () => {
    const svc = new ConfigService()
    const background = deferred<AppConfig>()
    getConfig.mockResolvedValue(makeConfig())
    updateConfig.mockReturnValueOnce(background.promise)

    // 后台写入（编辑器偏好/分栏比例同款入口）：不传 settingsPage
    const prefs = save(svc, () => {})
    await flushMicrotasks()

    // 任意保存队列在途（load 等待用），但设置页的门禁/UI 不被它置位
    expect(svc.saveInFlight.value).toBe(true)
    expect(svc.settingsSaveInFlight.value).toBe(false)

    const settingsSave = svc.runSaveTxn(async () => {}, { settingsPage: true })
    expect(svc.settingsSaveInFlight.value).toBe(true)

    background.resolve(makeConfig())
    await Promise.all([prefs, settingsSave])
    expect(svc.saveInFlight.value).toBe(false)
    expect(svc.settingsSaveInFlight.value).toBe(false)
  })
})
