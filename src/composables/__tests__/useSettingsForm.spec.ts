import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { effectScope, ref } from 'vue'
import type { EffectScope } from 'vue'
import { IpcError, parseAppError } from '@/bridge'
import { useSettingsForm } from '@/composables/useSettingsForm'
import { configService } from '@/services/config.service'
import type { AppConfig } from '@/types/config'

/// 以真实 AppError 载荷构造 IpcError（与 ipcInvoke 的错误归一化同源）
function makeIpcError(cmd: string, payload: unknown): IpcError {
  const { variant, message } = parseAppError(payload)
  return new IpcError(cmd, message, variant, payload)
}

const mocks = vi.hoisted(() => ({
  getConfig: vi.fn(),
  updateConfig: vi.fn(),
  invalidate: vi.fn(),
  switchOj: vi.fn(),
  resetSessionForOjSwitch: vi.fn(),
  resetAuthForOjSwitch: vi.fn(),
  routerReplace: vi.fn(),
  // 启动标志响应式快照（systemService.launchFlags）：钥匙模式切换后留在设置页
  launchFlags: { value: { settings: false } },
  // 当前路由（**活对象**：mock 的 currentRoute.value 直接引用它，测试中途可改）
  route: { name: 'Settings' as string | undefined },
}))

/// 模拟真实 ConfigService 的在途切换/代际：composable 的
/// `readSettledConfig` 依赖这两个成员，裸 vi.fn 无法表达「等待在途切换」。
const switchState = vi.hoisted(() => ({
  pending: null as Promise<void> | null,
  epoch: 0,
  last: null as { target: string; outcome: 'success' | 'persist-failed' } | null,
}))

/// 保存门禁的孪生状态（与生产 `runSaveTxn` 同构；真实排队/代际/队尾语义由
/// config.service.spec 的「save 门禁」套件用真实服务锁定，这里只复刻语义）。
/// `busy`（ref 需要 vue，vi.hoisted 环境拿不到）由下方 mock 工厂注入。
const saveState = vi.hoisted(() => ({
  tail: null as Promise<unknown> | null,
  epoch: 0,
  busy: null as { value: boolean } | null,
  /// 设置页发起的事务在途（与生产 `settingsSaveInFlight` 同构，由 depth 派生）。
  /// ref 需要 vue，vi.hoisted 环境拿不到，由 mock 工厂注入。
  settingsDepth: null as { value: number } | null,
  /// 设置页保存代际（与生产 `settingsSaveEpoch` 同构：入队/结束各 +1）
  settingsEpoch: 0,
  /// 等设置页保存落地的等待者
  settingsWaiters: [] as Array<() => void>,
}))

function settleSwitch(run: Promise<void>): void {
  if (switchState.pending === run) switchState.pending = null
  switchState.epoch += 1
}

/// 与真实 ConfigService 一致：成功保存同一 active 会解除 persist-failed
function resolvePersisted(active: string): void {
  if (switchState.last?.outcome === 'persist-failed' && switchState.last.target === active) {
    switchState.last = { target: active, outcome: 'success' }
  }
}

vi.mock('@/services/config.service', async () => {
  const { computed, readonly, ref } = await vi.importActual<typeof import('vue')>('vue')
  const saveBusy = ref(false)
  saveState.busy = saveBusy
  // 必须是 ref：`settingsSaveInFlight` 是 computed，普通字段不会触发重新求值
  const settingsDepth = ref(0)
  saveState.settingsDepth = settingsDepth

  /// 与生产 `runSaveTxn` 同构：入队即置位/代际 +1，链式等前一个落地，
  /// 只有队尾落地且无人再排队才复位；事务内直通 `tx.updateConfig`（不再排队）。
  /// `options.settingsPage` 只影响 `settingsSaveInFlight`（后台写入不置位）。
  async function runSaveTxn<T>(
    fn: (tx: {
      updateConfig: (mutate: (draft: AppConfig) => void) => Promise<AppConfig>
    }) => Promise<T>,
    options: { settingsPage?: boolean } = {},
  ): Promise<T> {
    const previous = saveState.tail
    let release!: () => void
    const gate = new Promise<void>((resolve) => {
      release = resolve
    })
    saveState.tail = gate
    saveBusy.value = true
    saveState.epoch += 1
    if (options.settingsPage) {
      settingsDepth.value += 1
      saveState.settingsEpoch += 1
    }
    try {
      if (previous) await previous.then(() => {}, () => {})
      return await fn({
        updateConfig: async (mutate: (draft: AppConfig) => void) => {
          const saved = await mocks.updateConfig(mutate)
          resolvePersisted(saved.oj.active)
          return saved
        },
      })
    } finally {
      saveState.epoch += 1
      release()
      if (saveState.tail === gate) {
        saveState.tail = null
        saveBusy.value = false
      }
      if (options.settingsPage) {
        saveState.settingsEpoch += 1
        settingsDepth.value -= 1
        if (settingsDepth.value === 0) {
          for (const resolve of saveState.settingsWaiters.splice(0)) resolve()
        }
      }
    }
  }

  return {
    configService: {
      getConfig: mocks.getConfig,
      // 刻意不 mock 公共 updateConfig：生产已删除该公开包装（保存走
      // runSaveTxn/tx.updateConfig），mock 面与生产 API 面保持一致
      invalidate: mocks.invalidate,
      switchOj: (id: string) => {
        switchState.epoch += 1
        const run = mocks.switchOj(id) as Promise<void>
        switchState.pending = run
        run.then(
          () => {
            switchState.last = { target: id, outcome: 'success' }
            settleSwitch(run)
          },
          (e: { variant?: string }) => {
            if (e?.variant === 'OjSwitchPersistFailed') {
              switchState.last = { target: id, outcome: 'persist-failed' }
            }
            settleSwitch(run)
          },
        )
        return run
      },
      latestOjSwitchResult: () => switchState.last,
      get ojSwitchEpoch() {
        return switchState.epoch
      },
      get ojSwitchInFlight() {
        return switchState.pending !== null
      },
      whenOjSwitchSettled: () => {
        const pending = switchState.pending
        return pending ? pending.catch(() => {}) : Promise.resolve()
      },
      get saveEpoch() {
        return saveState.epoch
      },
      saveInFlight: readonly(saveBusy),
      // 与生产一致：由 depth 派生（没有独立布尔量可漂移）
      settingsSaveInFlight: readonly(computed(() => settingsDepth.value > 0)),
      get settingsSaveEpoch() {
        return saveState.settingsEpoch
      },
      whenSaveSettled: () => {
        const tail = saveState.tail
        return tail ? tail.then(() => {}, () => {}) : Promise.resolve()
      },
      whenSettingsSaveSettled: () => {
        if (settingsDepth.value === 0) return Promise.resolve()
        return new Promise<void>((resolve) => saveState.settingsWaiters.push(resolve))
      },
      runSaveTxn,
    },
    // 与生产同源的具名判据；真实类型化判据由 config.service.spec 用真实 IpcError 锁定
    isOjSwitchPersistFailed: (e: { variant?: string } | null | undefined) =>
      e?.variant === 'OjSwitchPersistFailed',
  }
})
vi.mock('@/services/system.service', () => ({
  systemService: {
    launchFlags: mocks.launchFlags,
    isKeyed: () => mocks.launchFlags.value.settings,
  },
}))
vi.mock('@/stores/session', () => ({ resetSessionForOjSwitch: mocks.resetSessionForOjSwitch }))
/// OJ 切换的认证态复位（与 resetSessionForOjSwitch 成对调用；纯 composable 测试
/// 无活动 pinia，打桩同 session 模块）
vi.mock('@/stores/authStore', () => ({
  useAuthStore: () => ({ resetForOjSwitch: mocks.resetAuthForOjSwitch }),
}))
/// 路由级钥匙判据经 systemStore（收敛单一入口）：同样无 pinia，
/// 打桩同 authStore 模块模式，判据复刻「launchFlags × 'Settings'」
vi.mock('@/stores/systemStore', () => ({
  useSystemStore: () => ({
    isKeyedSettingsRoute:
      (routeName: string | symbol | null | undefined) =>
        mocks.launchFlags.value.settings && routeName === 'Settings',
  }),
}))
vi.mock('vue-router', () => ({
  useRouter: () => ({
    replace: mocks.routerReplace,
    currentRoute: { value: mocks.route },
  }),
}))

function baseConfig(): AppConfig {
  return {
    oj: {
      active: 'HOJ',
      instances: [{ id: 'HOJ', baseUrl: 'https://hoj.example', enabled: true, options: {} }],
      contestRef: '1001', contestPassword: null, timeoutSecs: 30,
      pollIntervalSecs: 5, pollTimeoutSecs: 300, cacheTtlSecs: 60, cacheProblemStatement: true,
    },
    editor: { fontSize: 14, tabSize: 4, autoSave: true, autoSaveIntervalSecs: 30, defaultLanguage: 'C++' },
    theme: { editorTheme: 'vs' },
    layout: { sidebarWidth: 240, splitRatio: 0.48 },
  }
}

let currentConfig: AppConfig
let scope: EffectScope
let settings: ReturnType<typeof useSettingsForm>

beforeEach(async () => {
  vi.resetAllMocks()
  vi.useFakeTimers()
  switchState.pending = null
  switchState.epoch = 0
  switchState.last = null
  saveState.tail = null
  saveState.epoch = 0
  if (saveState.busy) saveState.busy.value = false
  if (saveState.settingsDepth) saveState.settingsDepth.value = 0
  saveState.settingsEpoch = 0
  saveState.settingsWaiters.length = 0
  currentConfig = baseConfig()
  // 默认正常模式（无 --settings）：切换成功照常回登录页；钥匙用例显式改写
  mocks.launchFlags.value = { settings: false }
  mocks.route.name = 'Settings'
  mocks.getConfig.mockImplementation(async () => structuredClone(currentConfig))
  mocks.updateConfig.mockImplementation(async (mutator: (draft: AppConfig) => void) => {
    const draft = structuredClone(currentConfig)
    mutator(draft)
    currentConfig = draft
    return draft
  })
  // 与真实 switch_oj 一致：切换会改写配置的 oj.active（切换后重读才有意义）
  mocks.switchOj.mockImplementation(async (id: string) => {
    currentConfig.oj.active = id
  })
  scope = effectScope()
  settings = scope.run(() => useSettingsForm())!
  await settings.load()
})

afterEach(() => {
  scope.stop()
  vi.useRealTimers()
  vi.restoreAllMocks()
})

describe('useSettingsForm', () => {
  it('严格校验与放弃：无效字号阻止保存，隐藏的自动保存间隔不阻止保存', () => {
    settings.form.fontSize = '1.5'
    expect(settings.errors.value.fontSize).not.toBeNull()
    expect(settings.canSave.value).toBe(false)

    settings.discard()
    expect(settings.form.fontSize).toBe('14')
    expect(settings.dirty.value).toBe(false)

    settings.form.autoSave = false
    settings.form.autoSaveIntervalSecs = '1'
    expect(settings.errors.value.autoSaveIntervalSecs).toBeNull()
    settings.form.contestRef = '2002'
    expect(settings.canSave.value).toBe(true)
  })

  it.each(['abc', '', '1e5'])(
    '关闭自动保存后，隐藏的非法间隔 %j 不进入保存载荷（保留已存值）',
    async (raw) => {
      settings.form.autoSaveIntervalSecs = raw
      expect(settings.errors.value.autoSaveIntervalSecs).not.toBeNull()
      expect(settings.canSave.value).toBe(false)

      // 关闭开关后错误隐藏、保存被放行：隐藏值仍不得写进载荷
      settings.form.autoSave = false
      expect(settings.errors.value.autoSaveIntervalSecs).toBeNull()
      expect(settings.canSave.value).toBe(true)
      await settings.save()

      expect(currentConfig.editor.autoSave).toBe(false)
      expect(currentConfig.editor.autoSaveIntervalSecs).toBe(30)
    },
  )

  it('开启自动保存时把通过严格校验的间隔写入载荷', async () => {
    settings.form.autoSaveIntervalSecs = '120'
    await settings.save()
    expect(currentConfig.editor.autoSaveIntervalSecs).toBe(120)
  })

  it('保存期间的新编辑不被误标为已保存，discard 回到提交快照', async () => {
    let resolveSave!: (config: AppConfig) => void
    mocks.updateConfig.mockImplementationOnce((mutator: (draft: AppConfig) => void) => {
      const draft = structuredClone(currentConfig)
      mutator(draft)
      currentConfig = draft
      return new Promise<AppConfig>((resolve) => { resolveSave = resolve })
    })

    settings.form.fontSize = '16'
    const saving = settings.save()
    await Promise.resolve()
    settings.form.fontSize = '18'
    resolveSave(currentConfig)
    await saving

    expect(settings.dirty.value).toBe(true)
    settings.discard()
    expect(settings.form.fontSize).toBe('16')
  })

  it('active 指向缺失实例时不借用其它实例地址', async () => {
    currentConfig.oj.active = 'Hydro'
    await settings.load()
    expect(settings.form.ojUrl).toBe('')
  })

  it('保存无关字段不会重新启用已有禁用实例', async () => {
    currentConfig.oj.instances[0].enabled = false
    await settings.load()
    settings.form.fontSize = '16'
    await settings.save()
    expect(currentConfig.oj.instances[0].enabled).toBe(false)
  })

  it('新实例自动切换失败回滚下拉，可重新选择重试并清除旧提示', async () => {
    mocks.switchOj.mockRejectedValueOnce(new Error('切换失败'))
    settings.form.activeOj = 'Hydro'
    await settings.onSwitchOj()
    settings.form.ojUrl = 'https://hydro.example'
    await settings.save()

    expect(settings.form.activeOj).toBe('HOJ')
    expect(settings.form.ojUrl).toBe('https://hoj.example')
    expect(settings.dirty.value).toBe(false)
    expect(settings.switchNotice.value).toContain('实例已保存')

    // 回滚后的下一次无关保存必须写回 HOJ 地址，不能把 Hydro 地址串进 HOJ
    settings.form.fontSize = '16'
    await settings.save()
    expect(currentConfig.oj.instances.find((i) => i.id === 'HOJ')?.baseUrl)
      .toBe('https://hoj.example')

    settings.discard()
    expect(settings.switchError.value).toBeNull()
    expect(settings.switchNotice.value).toBeNull()
  })

  it('保存写完整载荷但不写 active，成功后清 dirty', async () => {
    settings.form.contestRef = ' 2002 '
    settings.form.contestPassword = ''
    await settings.save()

    expect(currentConfig.oj.contestRef).toBe('2002')
    expect(currentConfig.oj.contestPassword).toBeNull()
    expect(currentConfig.oj.active).toBe('HOJ')
    expect(settings.dirty.value).toBe(false)
    expect(mocks.switchOj).not.toHaveBeenCalled()
  })

  it('新实例保存成功后自动切换并清理会话上下文', async () => {
    settings.form.activeOj = 'Hydro'
    await settings.onSwitchOj()
    settings.form.ojUrl = 'https://hydro.example'
    await settings.save()

    expect(currentConfig.oj.instances).toContainEqual({
      id: 'Hydro', baseUrl: 'https://hydro.example', enabled: true, options: {},
    })
    expect(mocks.switchOj).toHaveBeenCalledWith('Hydro')
    expect(mocks.resetSessionForOjSwitch).toHaveBeenCalledTimes(1)
    // 认证态复位与领域清理成对出现（断环后拆到 authStore）
    expect(mocks.resetAuthForOjSwitch).toHaveBeenCalledTimes(1)
    expect(mocks.routerReplace).toHaveBeenCalledWith({ name: 'Login' })
  })

  it('钥匙模式切换成功后留在设置页（不导航），会话清理照旧', async () => {
    // --settings 的核心修复动作就是「填新地址/切 OJ → 保存」——切换成功后
    // 踢到登录页等于重新锁死；resetSessionForOjSwitch 照做，最小壳接管
    //（内容区共享实例，草稿保留）
    mocks.launchFlags.value = { settings: true }
    currentConfig.oj.instances.push({ id: 'Hydro', baseUrl: 'https://hydro.example', enabled: true, options: {} })
    await settings.load()
    settings.form.activeOj = 'Hydro'
    await settings.onSwitchOj()

    expect(mocks.switchOj).toHaveBeenCalledWith('Hydro')
    expect(mocks.resetSessionForOjSwitch).toHaveBeenCalledTimes(1)
    expect(mocks.routerReplace).not.toHaveBeenCalled()
    // 切换后重读配置并复位 baseline——不得出现假的「未保存更改」
    expect(settings.dirty.value).toBe(false)
    expect(settings.canSave.value).toBe(false)
  })

  it('切换期间已导航离开设置页——钥匙豁免不带走，照常回登录', async () => {
    // switch_oj 是异步的：期间用户可能已跳到比赛页。若无条件「停留」，无会话
    // 用户会被留在比赛页（豁免只属于设置页本身）
    mocks.launchFlags.value = { settings: true }
    currentConfig.oj.instances.push({ id: 'Hydro', baseUrl: 'https://hydro.example', enabled: true, options: {} })
    await settings.load()
    settings.form.activeOj = 'Hydro'
    mocks.route.name = 'ProblemSet' // 慢切换落地前路由已变
    await settings.onSwitchOj()

    expect(mocks.switchOj).toHaveBeenCalledWith('Hydro')
    expect(mocks.resetSessionForOjSwitch).toHaveBeenCalledTimes(1)
    expect(mocks.routerReplace).toHaveBeenCalledWith({ name: 'Login' })
  })

  it('切换保留全局草稿、只重置 OJ 域，dirty 只反映未保存的全局编辑', async () => {
    mocks.launchFlags.value = { settings: true }
    currentConfig.oj.instances.push({ id: 'Hydro', baseUrl: 'https://hydro.example', enabled: true, options: {} })
    await settings.load()
    // 全局草稿（字体）+ OJ 域草稿（超时）
    settings.form.fontSize = '18'
    settings.form.timeoutSecs = '99'
    settings.form.activeOj = 'Hydro'
    await settings.onSwitchOj()

    expect(settings.form.fontSize).toBe('18') // 全局草稿保留
    expect(settings.form.timeoutSecs).toBe('30') // OJ 域以磁盘为准
    expect(settings.form.activeOj).toBe('Hydro')
    expect(settings.dirty.value).toBe(true) // 未保存的全局编辑仍为 dirty

    await settings.save()
    expect(currentConfig.editor.fontSize).toBe(18)
    expect(settings.dirty.value).toBe(false)
    // OJ 域草稿（99）不得被写进新 OJ
    expect(currentConfig.oj.timeoutSecs).toBe(30)
  })

  it('切换后端成功但重载失败——承认已生效（对齐 B），不假装失败', async () => {
    mocks.launchFlags.value = { settings: true }
    currentConfig.oj.instances.push({ id: 'Hydro', baseUrl: 'https://hydro.example', enabled: true, options: {} })
    await settings.load()
    settings.form.activeOj = 'Hydro'
    // switch_oj 成功，但随即的重载失败
    mocks.getConfig.mockRejectedValueOnce(new Error('读取配置失败'))

    await settings.onSwitchOj()

    // switch_oj 已写盘并切 registry：UI 必须对齐 B，否则「界面 A、实际 B」
    expect(settings.form.activeOj).toBe('Hydro')
    expect(settings.form.ojUrl).toBe('https://hydro.example')
    expect(settings.switchError.value).toBeNull()
    expect(settings.switchNotice.value).toContain('切换已生效')
    // 基线同步重算——不得出现假「未保存更改」
    expect(settings.dirty.value).toBe(false)
  })

  it('重载失败时 A 的 OJ 域草稿也被重置，全局草稿仍保留', async () => {
    mocks.launchFlags.value = { settings: true }
    currentConfig.oj.instances.push({ id: 'Hydro', baseUrl: 'https://hydro.example', enabled: true, options: {} })
    await settings.load()
    settings.form.contestRef = 'draft-for-A' // OJ 域草稿
    settings.form.fontSize = '18' // 全局草稿
    settings.form.activeOj = 'Hydro'
    mocks.getConfig.mockRejectedValueOnce(new Error('读取配置失败'))

    await settings.onSwitchOj()

    expect(settings.form.activeOj).toBe('Hydro')
    expect(settings.form.contestRef).toBe(currentConfig.oj.contestRef) // A 的草稿不得带进 B
    expect(settings.form.fontSize).toBe('18') // 全局草稿保留
    expect(settings.dirty.value).toBe(true) // 仅剩未保存的全局编辑
  })

  it('OJ 域草稿被磁盘值覆盖时给出说明（全局草稿保留）', async () => {
    mocks.launchFlags.value = { settings: true }
    currentConfig.oj.instances.push({ id: 'Hydro', baseUrl: 'https://hydro.example', enabled: true, options: {} })
    await settings.load()
    settings.form.contestRef = 'draft-ref' // OJ 域草稿
    settings.form.fontSize = '18' // 全局草稿
    settings.form.activeOj = 'Hydro'
    await settings.onSwitchOj()

    expect(settings.switchNotice.value).toContain('未保存修改已从磁盘重新加载')
    expect(settings.form.contestRef).toBe(currentConfig.oj.contestRef) // 被磁盘值覆盖
    expect(settings.form.fontSize).toBe('18') // 全局草稿保留
  })

  it('切换已生效但持久化失败——承认生效、重置会话、对齐新 OJ', async () => {
    mocks.launchFlags.value = { settings: true }
    currentConfig.oj.instances.push({ id: 'Hydro', baseUrl: 'https://hydro.example', enabled: true, options: {} })
    await settings.load()
    settings.form.activeOj = 'Hydro'
    // 后端 registry 已切换、仅持久化失败：命令以专用变体拒绝
    currentConfig.oj.active = 'Hydro'
    // 用**真实载荷**经 parseAppError 构造 IpcError（手写 variant
    // 会绕过 bridge 白名单，生产上 variant 为 null、分支永不进入）
    mocks.switchOj.mockRejectedValueOnce(makeIpcError('switch_oj', {
      OjSwitchPersistFailed: 'OJ 切换已生效但保存配置失败: 磁盘满',
    }))

    await settings.onSwitchOj()

    expect(settings.form.activeOj).toBe('Hydro')
    expect(settings.form.ojUrl).toBe('https://hydro.example')
    expect(settings.switchError.value).toBeNull()
    expect(settings.switchNotice.value).toContain('配置保存失败')
    // 旧 OJ 的登录态不得带到新 OJ
    expect(mocks.resetSessionForOjSwitch).toHaveBeenCalledTimes(1)
    expect(settings.dirty.value).toBe(false)
  })

  it('切换单飞——进行中禁止保存、再次选择被忽略', async () => {
    mocks.launchFlags.value = { settings: true }
    currentConfig.oj.instances.push({ id: 'Hydro', baseUrl: 'https://hydro.example', enabled: true, options: {} })
    await settings.load()
    // 让 switch_oj 悬停，模拟慢切换
    let release!: () => void
    mocks.switchOj.mockReturnValueOnce(new Promise<void>((resolve) => { release = resolve }))

    settings.form.activeOj = 'Hydro'
    settings.form.fontSize = '18' // 让 dirty=true（切换中用户改了可编辑字段）
    const pending = settings.onSwitchOj()
    await Promise.resolve()

    expect(settings.switching.value).toBe(true)
    expect(settings.canSave.value).toBe(false) // 保存不得与在途切换交错
    await settings.save()
    expect(mocks.updateConfig).not.toHaveBeenCalled()

    // 切换进行中再次选择：忽略并回退下拉，不会发出第二个 switch_oj
    settings.form.activeOj = 'HOJ'
    await settings.onSwitchOj()
    expect(mocks.switchOj).toHaveBeenCalledTimes(1)

    release()
    await pending
    expect(settings.switching.value).toBe(false)
  })

  it('自动切换（保存新建实例）也进入单飞，期间不可再次选择', async () => {
    mocks.launchFlags.value = { settings: true }
    await settings.load()
    let release!: () => void
    mocks.switchOj.mockReturnValueOnce(new Promise<void>((resolve) => { release = resolve }))
    settings.form.activeOj = 'Hydro'
    settings.form.ojUrl = 'https://hydro.example'

    const saving = settings.save() // 新建实例 → 保存成功后自动切换
    await vi.waitFor(() => expect(mocks.switchOj).toHaveBeenCalledTimes(1))
    expect(settings.switching.value).toBe(true)

    // 自动切换在途：下拉选择被忽略，不会发起第二个 switch_oj
    settings.form.activeOj = 'HOJ'
    await settings.onSwitchOj()
    expect(mocks.switchOj).toHaveBeenCalledTimes(1)

    release()
    await saving
    expect(settings.switching.value).toBe(false)
  })

  it('保存进行中禁止显式切换（反向互斥）：回退下拉并给出可见提示', async () => {
    await settings.load()
    currentConfig.oj.instances.push({ id: 'Hydro', baseUrl: 'https://hydro.example', enabled: true, options: {} })
    await settings.load()
    let releaseSave!: () => void
    mocks.updateConfig.mockReturnValueOnce(new Promise<AppConfig>((resolve) => {
      releaseSave = () => resolve(structuredClone(currentConfig))
    }))
    settings.form.fontSize = '18'

    const saving = settings.save()
    await vi.waitFor(() => expect(mocks.updateConfig).toHaveBeenCalledTimes(1))
    // 保存中：切换请求被忽略。UI 层 select 已 disabled（ojDomainBusy），本分支
    // 只能经程序化调用触达——锁定防御行为：回退选择 + 可见提示（不是静默）
    settings.form.activeOj = 'Hydro'
    await settings.onSwitchOj()
    expect(mocks.switchOj).not.toHaveBeenCalled()
    expect(settings.form.activeOj).toBe('HOJ')
    expect(settings.switchNotice.value).toContain('正在保存配置')
    releaseSave()
    await saving
  })

  it('后台写入不阻塞显式切换：编辑器偏好/分栏比例在途时 OJ 下拉不被静默回退', async () => {
    await settings.load()
    currentConfig.oj.instances.push({ id: 'Hydro', baseUrl: 'https://hydro.example', enabled: true, options: {} })
    await settings.load()
    mocks.switchOj.mockResolvedValue(undefined)

    // 后台写入（不标记 settingsPage）：只排保存队列，不进入设置页门禁
    let releaseBackground!: () => void
    const background = configService.runSaveTxn(
      () => new Promise<void>((resolve) => { releaseBackground = resolve }),
    )
    await vi.waitFor(() => expect(configService.saveInFlight.value).toBe(true))
    expect(settings.saving.value).toBe(false)

    settings.form.activeOj = 'Hydro'
    await settings.onSwitchOj()

    // 修复前（saving 直接取 saveInFlight）：下拉被静默回退，不发起切换
    expect(mocks.switchOj).toHaveBeenCalledWith('Hydro')
    expect(settings.form.activeOj).toBe('Hydro')

    releaseBackground()
    await background
  })

  it('旧控制器卸载后完成切换，新控制器等待事务并读到新 active', async () => {
    mocks.launchFlags.value = { settings: true }
    currentConfig.oj.instances.push({ id: 'Hydro', baseUrl: 'https://hydro.example', enabled: true, options: {} })
    await settings.load()
    let release!: () => void
    mocks.switchOj.mockImplementationOnce(
      (id: string) => new Promise<void>((resolve) => {
        release = () => {
          currentConfig.oj.active = id
          resolve()
        }
      }),
    )
    settings.form.activeOj = 'Hydro'
    const pending = settings.onSwitchOj()
    await Promise.resolve()
    expect(settings.switching.value).toBe(true)

    // 离开设置页（旧控制器销毁）→ 重新进入（新控制器挂载，读到仍是旧 active 的磁盘）。
    // 新控制器**不建 scope**：不注册切换完成观察者，从而把断言隔离到
    // 「挂载时等待在途切换」这一条路径上（观察者会掩盖缺失的等待）。
    scope.stop()
    const next = useSettingsForm()
    const loading = next.load()
    let loaded = false
    void loading.then(() => { loaded = true })
    await Promise.resolve()
    // 在途切换未落地前新控制器不得完成加载（否则展示旧 active，且旧控制器
    // 已销毁、没有回调会再来刷新它）
    expect(loaded).toBe(false)
    expect(next.switching.value).toBe(true)

    release()
    await pending
    await loading

    expect(next.form.activeOj).toBe('Hydro')
    expect(next.form.ojUrl).toBe('https://hydro.example')
    expect(next.dirty.value).toBe(false)
  })

  it('旧控制器卸载后的完成回调不再回写表单', async () => {
    mocks.launchFlags.value = { settings: true }
    currentConfig.oj.instances.push({ id: 'Hydro', baseUrl: 'https://hydro.example', enabled: true, options: {} })
    await settings.load()
    settings.form.contestRef = 'draft-before-leave'
    let release!: () => void
    mocks.switchOj.mockImplementationOnce(
      (id: string) => new Promise<void>((resolve) => {
        release = () => {
          currentConfig.oj.active = id
          resolve()
        }
      }),
    )
    settings.form.activeOj = 'Hydro'
    const pending = settings.onSwitchOj()
    await Promise.resolve()

    scope.stop() // 视图卸载：旧控制器失去表单所有权
    release()
    await pending

    // 回写被跳过：表单保持离开时的草稿，而不是切换后的磁盘值
    expect(settings.form.contestRef).toBe('draft-before-leave')
    expect(settings.form.activeOj).toBe('Hydro') // onSwitchOj 的选择回填（非磁盘对齐）
  })


  it('普通切换失败只回滚选择，保留地址草稿', async () => {
    currentConfig.oj.instances.push({ id: 'Hydro', baseUrl: 'https://hydro.example', enabled: true, options: {} })
    await settings.load()
    settings.form.ojUrl = 'https://draft-hoj.example' // A 的地址草稿（未保存）
    settings.form.activeOj = 'Hydro'
    mocks.switchOj.mockRejectedValueOnce(new Error('切换失败'))

    await settings.onSwitchOj()

    expect(settings.form.activeOj).toBe('HOJ') // 选择回滚
    expect(settings.form.ojUrl).toBe('https://draft-hoj.example') // 草稿保留
  })

  it('锁定态 active 指向缺失实例——不创建、不切换、不谎报', async () => {
    // 手改配置让 oj.active 指向不存在的实例：锁定态载荷跳过实例写入，
    // 因此绝不能触发 performSwitch（否则会提示「实例已保存；切换未成功」）
    currentConfig.oj.active = 'Ghost'
    const view = useSettingsForm(ref(true))
    await view.load()
    view.form.fontSize = '18'
    await view.save()

    expect(mocks.switchOj).not.toHaveBeenCalled()
    expect(view.switchNotice.value).toBeNull()
  })

  it('新实例保存失败后，下一次普通保存不得沿用自动切换意图', async () => {
    settings.form.activeOj = 'Hydro'
    await settings.onSwitchOj()
    settings.form.ojUrl = 'https://hydro.example'
    mocks.updateConfig.mockRejectedValueOnce(new Error('保存失败'))
    await settings.save()
    expect(mocks.switchOj).not.toHaveBeenCalled()

    settings.form.activeOj = 'HOJ'
    await settings.save()
    expect(mocks.switchOj).not.toHaveBeenCalled()
    expect(mocks.resetSessionForOjSwitch).not.toHaveBeenCalled()
    expect(mocks.routerReplace).not.toHaveBeenCalled()
  })

  it('已配置 OJ 切换失败回滚下拉，不清会话、不导航', async () => {
    currentConfig.oj.instances.push({ id: 'Hydro', baseUrl: 'https://hydro.example', enabled: true, options: {} })
    await settings.load()
    settings.form.activeOj = 'Hydro'
    mocks.switchOj.mockRejectedValueOnce(new Error('切换失败'))

    await settings.onSwitchOj()
    expect(settings.form.activeOj).toBe('HOJ')
    expect(settings.switchError.value).toBe('切换失败')
    expect(mocks.resetSessionForOjSwitch).not.toHaveBeenCalled()
    expect(mocks.routerReplace).not.toHaveBeenCalled()
  })

  it('foreign reload 仍处于共享 busy，完成后才解除', async () => {
    mocks.launchFlags.value = { settings: true }
    currentConfig.oj.instances.push({ id: 'Hydro', baseUrl: 'https://hydro.example', enabled: true, options: {} })
    await settings.load()

    // 新控制器在 A 的切换期间已经挂载（观察者已注册）
    const scopeB = effectScope()
    const formB = scopeB.run(() => useSettingsForm())!
    await formB.load()
    expect(formB.form.activeOj).toBe('HOJ')

    let releaseSwitch!: () => void
    mocks.switchOj.mockImplementationOnce(
      (id: string) => new Promise<void>((resolve) => {
        releaseSwitch = () => {
          currentConfig.oj.active = id
          resolve()
        }
      }),
    )
    settings.form.activeOj = 'Hydro'
    const pendingA = settings.onSwitchOj()
    await Promise.resolve()

    // A 卸载；B 继续挂载，A 的事务完成时会在 finally 里通知 B
    scope.stop()
    let releaseAReload!: () => void
    mocks.getConfig.mockImplementationOnce(
      () => new Promise<AppConfig>((resolve) => {
        releaseAReload = () => resolve(structuredClone(currentConfig))
      }),
    )
    let releaseBReload!: () => void
    let aDone = false
    void pendingA.then(() => { aDone = true })

    releaseSwitch()
    await vi.waitFor(() => expect(releaseAReload).toBeTypeOf('function'))
    // A（已卸载）自己的重读放行后，下一个 getConfig 才是 B 的 foreign reload
    mocks.getConfig.mockImplementationOnce(
      () => new Promise<AppConfig>((resolve) => {
        releaseBReload = () => resolve(structuredClone(currentConfig))
      }),
    )
    releaseAReload()
    await vi.waitFor(() => expect(releaseBReload).toBeTypeOf('function'))

    // B 的第二次读仍在途：A 的事务不得结束、共享 busy 不得解除
    expect(aDone).toBe(false)
    expect(formB.switching.value).toBe(true)

    releaseBReload()
    await pendingA
    expect(formB.form.activeOj).toBe('Hydro')
    expect(formB.switching.value).toBe(false)
    scopeB.stop()
  })

  it('foreign reload 失败时已发生新切换，不回写旧目标', async () => {
    mocks.launchFlags.value = { settings: true }
    currentConfig.oj.instances.push(
      { id: 'Hydro', baseUrl: 'https://hydro.example', enabled: true, options: {} },
      { id: 'C', baseUrl: 'https://c.example', enabled: true, options: {} },
    )
    await settings.load()
    const scopeB = effectScope()
    const formB = scopeB.run(() => useSettingsForm())!
    await formB.load()

    let releaseSwitch!: () => void
    mocks.switchOj.mockImplementationOnce(
      (id: string) => new Promise<void>((resolve) => {
        releaseSwitch = () => {
          currentConfig.oj.active = id
          resolve()
        }
      }),
    )
    settings.form.activeOj = 'Hydro'
    const pendingA = settings.onSwitchOj()
    await Promise.resolve()
    scope.stop()

    let releaseAReload!: () => void
    mocks.getConfig.mockImplementationOnce(
      () => new Promise<AppConfig>((resolve) => {
        releaseAReload = () => resolve(structuredClone(currentConfig))
      }),
    )
    let rejectBReload!: (e: Error) => void
    releaseSwitch()
    await vi.waitFor(() => expect(releaseAReload).toBeTypeOf('function'))
    mocks.getConfig.mockImplementationOnce(
      () => new Promise<AppConfig>((_resolve, reject) => {
        rejectBReload = reject
      }),
    )
    releaseAReload()
    await vi.waitFor(() => expect(rejectBReload).toBeTypeOf('function'))

    // B 的 foreign reload 读取期间，另一条路径完成了切到 C（直接走服务层）
    await configService.switchOj('C')
    rejectBReload(new Error('读取配置失败'))
    await pendingA

    // 旧目标 Hydro 的失败对齐不得覆盖新状态
    expect(formB.form.activeOj).not.toBe('Hydro')
    expect(formB.switchNotice.value).toBeNull()
    scopeB.stop()
  })

  it('切换期间新挂载的控制器加载后仍保留持久化失败警告', async () => {
    mocks.launchFlags.value = { settings: true }
    currentConfig.oj.instances.push({ id: 'Hydro', baseUrl: 'https://hydro.example', enabled: true, options: {} })
    await settings.load()

    let rejectSwitch!: () => void
    mocks.switchOj.mockImplementationOnce(
      (id: string) => new Promise<void>((_resolve, reject) => {
        rejectSwitch = () => {
          currentConfig.oj.active = id
          reject(makeIpcError('switch_oj', {
            OjSwitchPersistFailed: 'OJ 切换已生效但保存配置失败: 磁盘满',
          }))
        }
      }),
    )
    settings.form.activeOj = 'Hydro'
    const pendingA = settings.onSwitchOj()
    await Promise.resolve()
    scope.stop() // 旧控制器卸载

    // 新控制器在切换在途时挂载（load 等待事务落地后读取）
    const scopeB = effectScope()
    const formB = scopeB.run(() => useSettingsForm())!
    const loadingB = formB.load()

    rejectSwitch()
    await pendingA
    await loadingB

    // 载入后（会清空普通提示）仍保留持久化失败警告
    expect(formB.form.activeOj).toBe('Hydro')
    expect(formB.switchNotice.value).toContain('配置保存失败')
    expect(formB.switchNotice.value).toContain('重启后可能回退')
    scopeB.stop()
  })

  it('旧控制器保存在途时新控制器的切换被忽略', async () => {
    mocks.launchFlags.value = { settings: true }
    currentConfig.oj.instances.push({ id: 'Hydro', baseUrl: 'https://hydro.example', enabled: true, options: {} })
    await settings.load()

    let releaseSave!: () => void
    mocks.updateConfig.mockImplementationOnce(
      (mutator: (draft: AppConfig) => void) => new Promise<AppConfig>((resolve) => {
        releaseSave = () => {
          const draft = structuredClone(currentConfig)
          mutator(draft)
          currentConfig = draft
          resolve(draft)
        }
      }),
    )
    settings.form.activeOj = 'C'
    settings.form.ojUrl = 'https://c.example'
    const savingA = settings.save()
    await vi.waitFor(() => expect(mocks.updateConfig).toHaveBeenCalledTimes(1))

    // A 卸载，B 挂载：共享 saving 使 B 的 load 等待保存整段完成
    scope.stop()
    const scopeB = effectScope()
    const formB = scopeB.run(() => useSettingsForm())!
    const loadingB = formB.load()
    expect(formB.saving.value).toBe(true)

    // 新控制器的显式切换被共享 saving 拦下，不会发出第二个 switch
    formB.form.activeOj = 'Hydro'
    await formB.onSwitchOj()
    expect(mocks.switchOj).not.toHaveBeenCalled()

    const readsBeforeRelease = mocks.getConfig.mock.calls.length
    releaseSave()
    await savingA
    await loadingB

    // 等待保存落地后只读一次（A 的切换重置 + B 的 foreign reload + B 的 load）；
    // 若沿用等待前的 saveEpoch，load 会固定多读一次
    expect(mocks.getConfig.mock.calls.length - readsBeforeRelease).toBe(3)

    // 只有旧保存的自动切换（C），没有较新的 Hydro 选择被覆盖
    expect(mocks.switchOj).toHaveBeenCalledTimes(1)
    expect(mocks.switchOj).toHaveBeenCalledWith('C')
    expect(formB.form.activeOj).toBe('C')
    expect(formB.saving.value).toBe(false)
    scopeB.stop()
  })

  it('后台写入悬挂不阻塞设置页加载（load 只等设置页保存）', async () => {
    // 后台写入（编辑器偏好/分栏比例）永不返回：若 load 等「任意保存」，
    // 设置页会永远停在 loading 且无法打断（行为回归）
    let releaseBackground!: () => void
    mocks.updateConfig.mockImplementationOnce(
      (mutator: (draft: AppConfig) => void) => new Promise<AppConfig>((resolve) => {
        releaseBackground = () => {
          const draft = structuredClone(currentConfig)
          mutator(draft)
          currentConfig = draft
          resolve(draft)
        }
      }),
    )
    const background = configService.runSaveTxn((tx) => tx.updateConfig(() => {}))
    await vi.waitFor(() => expect(mocks.updateConfig).toHaveBeenCalledTimes(1))
    expect(configService.saveInFlight.value).toBe(true)
    expect(configService.settingsSaveInFlight.value).toBe(false)

    // load 不等后台写入：读盘完成、loading 复位
    await settings.load()
    expect(settings.loading.value).toBe(false)
    expect(settings.loadError.value).toBeNull()

    releaseBackground()
    await background
  })

  it('跨控制器保留「已生效但持久化失败」警告', async () => {
    mocks.launchFlags.value = { settings: true }
    currentConfig.oj.instances.push({ id: 'Hydro', baseUrl: 'https://hydro.example', enabled: true, options: {} })
    await settings.load()
    const scopeB = effectScope()
    const formB = scopeB.run(() => useSettingsForm())!
    await formB.load()

    let rejectSwitch!: () => void
    mocks.switchOj.mockImplementationOnce(
      (id: string) => new Promise<void>((_resolve, reject) => {
        rejectSwitch = () => {
          currentConfig.oj.active = id
          reject(makeIpcError('switch_oj', {
            OjSwitchPersistFailed: 'OJ 切换已生效但保存配置失败: 磁盘满',
          }))
        }
      }),
    )
    settings.form.activeOj = 'Hydro'
    const pendingA = settings.onSwitchOj()
    await Promise.resolve()
    scope.stop() // 旧控制器卸载后无法设置警告，必须由共享事务结果传给 B

    rejectSwitch()
    await pendingA

    expect(formB.form.activeOj).toBe('Hydro')
    expect(formB.switchNotice.value).toContain('配置保存失败')
    expect(formB.switchNotice.value).toContain('重启后可能回退')
    scopeB.stop()
  })

  it('无观察者时持久化失败，晚挂载控制器恢复警告；成功切换后清除', async () => {
    mocks.launchFlags.value = { settings: true }
    currentConfig.oj.instances.push({ id: 'Hydro', baseUrl: 'https://hydro.example', enabled: true, options: {} })
    await settings.load()

    let rejectSwitch!: () => void
    mocks.switchOj.mockImplementationOnce(
      (id: string) => new Promise<void>((_resolve, reject) => {
        rejectSwitch = () => {
          currentConfig.oj.active = id
          reject(makeIpcError('switch_oj', {
            OjSwitchPersistFailed: 'OJ 切换已生效但保存配置失败: 磁盘满',
          }))
        }
      }),
    )
    settings.form.activeOj = 'Hydro'
    const pendingA = settings.onSwitchOj()
    await Promise.resolve()
    // 旧控制器卸载后**没有任何观察者**，切换结果只能落在模块级共享结果里
    scope.stop()
    rejectSwitch()
    await pendingA

    const scopeB = effectScope()
    const formB = scopeB.run(() => useSettingsForm())!
    await formB.load()
    expect(formB.form.activeOj).toBe('Hydro')
    expect(formB.switchNotice.value).toContain('配置保存失败')

    // 后续成功切换把共享结果覆盖为 success：更晚挂载的控制器不再提示
    formB.form.activeOj = 'HOJ'
    await formB.onSwitchOj()
    expect(formB.form.activeOj).toBe('HOJ')
    scopeB.stop()

    const scopeC = effectScope()
    const formC = scopeC.run(() => useSettingsForm())!
    await formC.load()
    expect(formC.form.activeOj).toBe('HOJ')
    expect(formC.switchNotice.value).toBeNull()
    scopeC.stop()
  })

  it('persist-failed 后普通保存成功解除警告，晚挂载不再提示', async () => {
    mocks.launchFlags.value = { settings: true }
    currentConfig.oj.instances.push({ id: 'Hydro', baseUrl: 'https://hydro.example', enabled: true, options: {} })
    await settings.load()

    // 切换已生效但落盘失败：Registry/内存=Hydro，磁盘仍是 HOJ
    mocks.switchOj.mockImplementationOnce(
      (id: string) => new Promise<void>((_resolve, reject) => {
        currentConfig.oj.active = id
        reject(makeIpcError('switch_oj', {
          OjSwitchPersistFailed: 'OJ 切换已生效但保存配置失败: 磁盘满',
        }))
      }),
    )
    settings.form.activeOj = 'Hydro'
    await settings.onSwitchOj()
    expect(settings.switchNotice.value).toContain('配置保存失败')

    // 普通保存把包含 active=Hydro 的完整配置写入磁盘：回退风险解除
    settings.form.fontSize = '16'
    await settings.save()
    expect(currentConfig.oj.active).toBe('Hydro')
    expect(currentConfig.editor.fontSize).toBe(16)
    expect(settings.switchNotice.value).toBeNull()

    // 晚挂载控制器：共享结果已视为解决，不再提示
    scope.stop()
    const scopeB = effectScope()
    const formB = scopeB.run(() => useSettingsForm())!
    await formB.load()
    expect(formB.form.activeOj).toBe('Hydro')
    expect(formB.switchNotice.value).toBeNull()
    scopeB.stop()
  })
})
