import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { effectScope } from 'vue'
import type { EffectScope } from 'vue'
import { useSettingsForm } from '@/composables/useSettingsForm'
import type { AppConfig } from '@/types/config'

const mocks = vi.hoisted(() => ({
  getConfig: vi.fn(),
  updateConfig: vi.fn(),
  invalidate: vi.fn(),
  switchOj: vi.fn(),
  activeOjBaseUrl: vi.fn(),
  resetSessionForOjSwitch: vi.fn(),
  routerReplace: vi.fn(),
}))

vi.mock('@/services/config.service', () => ({
  configService: {
    getConfig: mocks.getConfig,
    updateConfig: mocks.updateConfig,
    invalidate: mocks.invalidate,
    switchOj: mocks.switchOj,
    activeOjBaseUrl: mocks.activeOjBaseUrl,
  },
}))
vi.mock('@/stores/session', () => ({ resetSessionForOjSwitch: mocks.resetSessionForOjSwitch }))
vi.mock('vue-router', () => ({ useRouter: () => ({ replace: mocks.routerReplace }) }))

function baseConfig(): AppConfig {
  return {
    user: { lastUsername: '' },
    oj: {
      active: 'HOJ',
      instances: [{ id: 'HOJ', baseUrl: 'https://hoj.example', enabled: true, options: {} }],
      contestRef: '1001', contestPassword: null, timeoutSecs: 30,
      pollIntervalSecs: 5, pollTimeoutSecs: 300, cacheTtlSecs: 60, cacheProblemStatement: true,
    },
    editor: { fontSize: 14, tabSize: 4, autoSave: true, autoSaveIntervalSecs: 30, defaultLanguage: 'C++' },
    theme: { themeName: 'light', editorTheme: 'vs' },
    layout: { sidebarWidth: 240, splitRatio: 0.48 },
  }
}

let currentConfig: AppConfig
let scope: EffectScope
let settings: ReturnType<typeof useSettingsForm>

beforeEach(async () => {
  vi.resetAllMocks()
  vi.useFakeTimers()
  currentConfig = baseConfig()
  mocks.getConfig.mockImplementation(async () => structuredClone(currentConfig))
  mocks.updateConfig.mockImplementation(async (mutator: (draft: AppConfig) => void) => {
    const draft = structuredClone(currentConfig)
    mutator(draft)
    currentConfig = draft
    return draft
  })
  mocks.switchOj.mockResolvedValue(undefined)
  mocks.activeOjBaseUrl.mockImplementation((config: AppConfig) => config.oj.instances[0]?.baseUrl ?? '')
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
  it('严格校验与放弃：autoSave=false 也不豁免无效间隔', () => {
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
    expect(mocks.routerReplace).toHaveBeenCalledWith({ name: 'Login' })
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
})
