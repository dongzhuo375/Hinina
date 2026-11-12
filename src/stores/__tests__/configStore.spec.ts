import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { computed } from 'vue'
import { useConfigStore } from '@/stores/configStore'
import type { OjSwitchResult } from '@/services/config.service'

/**
 * configStore 的转发契约用例。
 *
 * 本 store 是纯转发（不复制状态、不加 await 壳），这里的职责是把「转发面按
 * service 状态支撑形态分两类」的约定钉死：
 * - ref 支撑的（settingsSaveInFlight / ojSwitchInFlight）走 getter——响应式
 *   追踪，值翻转后重读拿到新值；
 * - 普通字段支撑的（ojSwitchEpoch / settingsSaveEpoch / latestOjSwitchResult）
 *   走 action 方法——**逐次直读**。若有人把它们改成 getter，本文件的调用形态
 *   会立即抛错（store 方法名不再是函数，比断言更响亮）；即便调用点同步改成
 *   属性访问，computed 对无响应式依赖的求值结果也会永久缓存，代际比对与
 *   持久化失败提示会静默失效——「翻转后必须读到新值」用例锁定取值新鲜度。
 */

const mocks = vi.hoisted(() => ({
  getConfig: vi.fn(),
  invalidate: vi.fn(),
  switchOj: vi.fn(),
  runSaveTxn: vi.fn(),
  whenOjSwitchSettled: vi.fn(),
  whenSettingsSaveSettled: vi.fn(),
  getOjBaseUrl: vi.fn(),
  getPollSchedule: vi.fn(),
  getEditorPrefs: vi.fn(),
  updateEditorPrefs: vi.fn(),
  getSplitRatio: vi.fn(),
  saveSplitRatio: vi.fn(),
  latestOjSwitchResult: vi.fn(),
  isOjSwitchPersistFailed: vi.fn(),
}))

/// service 侧的普通字段（非响应式）：测试中途改写，锁定「action 逐次直读」
const plainState = vi.hoisted(() => ({
  ojSwitchEpoch: 0,
  settingsSaveEpoch: 0,
  last: null as OjSwitchResult | null,
}))

/// service 侧的 ref 支撑状态（mock 工厂注入真实 ref，测试中途翻转）
const refs = vi.hoisted(() => ({
  settingsDepth: null as { value: number } | null,
  switchTail: null as { value: boolean } | null,
}))

vi.mock('@/services/config.service', async () => {
  const { computed, readonly, ref } = await vi.importActual<typeof import('vue')>('vue')
  const settingsDepth = ref(0)
  const switchTail = ref(false)
  refs.settingsDepth = settingsDepth
  refs.switchTail = switchTail

  return {
    configService: {
      getConfig: mocks.getConfig,
      invalidate: mocks.invalidate,
      switchOj: mocks.switchOj,
      runSaveTxn: mocks.runSaveTxn,
      whenOjSwitchSettled: mocks.whenOjSwitchSettled,
      whenSettingsSaveSettled: mocks.whenSettingsSaveSettled,
      getOjBaseUrl: mocks.getOjBaseUrl,
      getPollSchedule: mocks.getPollSchedule,
      getEditorPrefs: mocks.getEditorPrefs,
      updateEditorPrefs: mocks.updateEditorPrefs,
      getSplitRatio: mocks.getSplitRatio,
      saveSplitRatio: mocks.saveSplitRatio,
      latestOjSwitchResult: mocks.latestOjSwitchResult,
      // ref 支撑：settingsSaveInFlight 由 depth 派生（与生产同构）
      settingsSaveInFlight: readonly(computed(() => settingsDepth.value > 0)),
      get ojSwitchInFlight() {
        return switchTail.value
      },
      get ojSwitchEpoch() {
        return plainState.ojSwitchEpoch
      },
      get settingsSaveEpoch() {
        return plainState.settingsSaveEpoch
      },
    },
    isOjSwitchPersistFailed: mocks.isOjSwitchPersistFailed,
  }
})

const ERR = new Error('boom')

beforeEach(() => {
  setActivePinia(createPinia())
  vi.resetAllMocks()
  plainState.ojSwitchEpoch = 0
  plainState.settingsSaveEpoch = 0
  plainState.last = null
  if (refs.settingsDepth) refs.settingsDepth.value = 0
  if (refs.switchTail) refs.switchTail.value = false
  mocks.latestOjSwitchResult.mockImplementation(() => plainState.last)
  mocks.isOjSwitchPersistFailed.mockImplementation(
    (e: unknown) => e === ERR,
  )
})

describe('configStore — ref 支撑的响应式转发（getter）', () => {
  it('settingsSaveInFlight 跟随 service ref 翻转（普通字段不会触发重新求值，必须是 ref）', () => {
    const store = useConfigStore()
    expect(store.settingsSaveInFlight).toBe(false)

    // 经 computed 消费（useSettingsForm 的包裹形态）同样跟随
    const saving = computed(() => store.settingsSaveInFlight)
    expect(saving.value).toBe(false)

    refs.settingsDepth!.value = 1
    expect(store.settingsSaveInFlight).toBe(true)
    expect(saving.value).toBe(true)

    refs.settingsDepth!.value = 0
    expect(store.settingsSaveInFlight).toBe(false)
    expect(saving.value).toBe(false)
  })

  it('ojSwitchInFlight 跟随 service 队尾翻转', () => {
    const store = useConfigStore()
    expect(store.ojSwitchInFlight).toBe(false)
    refs.switchTail!.value = true
    expect(store.ojSwitchInFlight).toBe(true)
  })
})

describe('configStore — 普通字段的逐次直读（action 方法）', () => {
  it('ojSwitchEpoch 翻转后必须读到新值（误改成 getter 会在调用形态直接抛错；本用例另锁定逐次直读的新鲜度）', () => {
    const store = useConfigStore()
    expect(store.ojSwitchEpoch()).toBe(0)
    plainState.ojSwitchEpoch = 2
    expect(store.ojSwitchEpoch()).toBe(2)
  })

  it('settingsSaveEpoch 翻转后必须读到新值', () => {
    const store = useConfigStore()
    expect(store.settingsSaveEpoch()).toBe(0)
    plainState.settingsSaveEpoch = 4
    expect(store.settingsSaveEpoch()).toBe(4)
  })

  it('latestOjSwitchResult 逐次直读（不缓存首次结果）', () => {
    const store = useConfigStore()
    expect(store.latestOjSwitchResult()).toBeNull()
    plainState.last = { target: 'HOJ', outcome: 'persist-failed' }
    expect(store.latestOjSwitchResult()).toEqual({ target: 'HOJ', outcome: 'persist-failed' })
  })

  it('isOjSwitchPersistFailed 委托 service 判据（透传同一错误对象）', () => {
    const store = useConfigStore()
    expect(store.isOjSwitchPersistFailed(ERR)).toBe(true)
    expect(store.isOjSwitchPersistFailed(new Error('other'))).toBe(false)
    expect(mocks.isOjSwitchPersistFailed).toHaveBeenCalledWith(ERR)
  })
})

describe('configStore — action 纯转发（参数与回传直透，不加 await 壳）', () => {
  it('读侧方法透传参数并回传 service 结果', async () => {
    const store = useConfigStore()
    const config = { oj: { active: 'HOJ' } }
    const prefs = { fontSize: 14, tabSize: 4, editorTheme: 'vs' }
    const schedule = { intervalMs: 2_000, timeoutMs: 300_000 }
    mocks.getConfig.mockResolvedValue(config)
    mocks.getOjBaseUrl.mockResolvedValue('https://hoj.example.com')
    mocks.getPollSchedule.mockResolvedValue(schedule)
    mocks.getEditorPrefs.mockResolvedValue(prefs)
    mocks.getSplitRatio.mockResolvedValue(0.48)

    await expect(store.getConfig()).resolves.toBe(config)
    await expect(store.getOjBaseUrl()).resolves.toBe('https://hoj.example.com')
    await expect(store.getPollSchedule()).resolves.toBe(schedule)
    await expect(store.getEditorPrefs()).resolves.toBe(prefs)
    await expect(store.getSplitRatio()).resolves.toBe(0.48)

    await store.switchOj('Hydro')
    expect(mocks.switchOj).toHaveBeenCalledWith('Hydro')

    await store.updateEditorPrefs({ fontSize: 16 })
    expect(mocks.updateEditorPrefs).toHaveBeenCalledWith({ fontSize: 16 })

    await store.saveSplitRatio(0.52)
    expect(mocks.saveSplitRatio).toHaveBeenCalledWith(0.52)

    store.invalidate()
    expect(mocks.invalidate).toHaveBeenCalledOnce()
  })

  it('runSaveTxn 透传回调与 options；回调收到的 tx 来自 service', async () => {
    const store = useConfigStore()
    const txStub = { updateConfig: vi.fn() }
    mocks.runSaveTxn.mockImplementation(
      async (fn: (tx: unknown) => Promise<string>, _options: unknown) => fn(txStub),
    )

    const result = await store.runSaveTxn(
      async (tx) => {
        expect(tx).toBe(txStub)
        return 'saved'
      },
      { settingsPage: true },
    )

    expect(result).toBe('saved')
    expect(mocks.runSaveTxn).toHaveBeenCalledOnce()
    expect(mocks.runSaveTxn.mock.calls[0][1]).toEqual({ settingsPage: true })
  })

  it('action 同步进入 service（零等待即可见调用）——转发层不得再加 await 壳', () => {
    // 若有人把转发改成 `async runSaveTxn(...) { await ... }` 或在中间插
    // `await Promise.resolve()`，调用会在下一个微任务才到达 service：入队/代际
    // 的「同步置位」窗口被推迟（仅注释锁定不够，补一个零节拍断言）
    const store = useConfigStore()
    mocks.runSaveTxn.mockReturnValue(Promise.resolve('ok'))
    mocks.getConfig.mockReturnValue(Promise.resolve({}))
    mocks.switchOj.mockReturnValue(Promise.resolve())

    void store.runSaveTxn(async () => 'ok')
    expect(mocks.runSaveTxn).toHaveBeenCalledTimes(1)

    void store.getConfig()
    expect(mocks.getConfig).toHaveBeenCalledTimes(1)

    void store.switchOj('HOJ')
    expect(mocks.switchOj).toHaveBeenCalledTimes(1)
  })

  it('等待队列方法透传（whenOjSwitchSettled / whenSettingsSaveSettled）', async () => {
    const store = useConfigStore()
    const settled = Promise.resolve()
    mocks.whenOjSwitchSettled.mockReturnValue(settled)
    mocks.whenSettingsSaveSettled.mockResolvedValue(undefined)

    await expect(store.whenOjSwitchSettled()).resolves.toBeUndefined()
    await expect(store.whenSettingsSaveSettled()).resolves.toBeUndefined()
    expect(mocks.whenOjSwitchSettled).toHaveBeenCalledOnce()
    expect(mocks.whenSettingsSaveSettled).toHaveBeenCalledOnce()
  })
})
