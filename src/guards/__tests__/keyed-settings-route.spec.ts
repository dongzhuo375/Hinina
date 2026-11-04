import { beforeEach, describe, expect, it, vi } from 'vitest'

/// 服务层只提供进程级判据；本模块补路由身份，二者都要成立才算「钥匙设置页」。
const { isKeyed } = vi.hoisted(() => ({ isKeyed: vi.fn() }))
vi.mock('@/services/system.service', () => ({ systemService: { isKeyed } }))

import { isKeyedSettingsRoute } from '@/guards/keyed-settings-route'

describe('isKeyedSettingsRoute', () => {
  beforeEach(() => {
    isKeyed.mockReset()
  })

  it('钥匙模式 + Settings 路由才为真', () => {
    isKeyed.mockReturnValue(true)
    expect(isKeyedSettingsRoute('Settings')).toBe(true)
    expect(isKeyedSettingsRoute('ProblemSet')).toBe(false)
    expect(isKeyedSettingsRoute('Login')).toBe(false)
  })

  it('非钥匙模式下一律为假（哪怕路由是 Settings）', () => {
    isKeyed.mockReturnValue(false)
    expect(isKeyedSettingsRoute('Settings')).toBe(false)
  })

  it('缺失/符号路由名一律为假', () => {
    isKeyed.mockReturnValue(true)
    expect(isKeyedSettingsRoute(undefined)).toBe(false)
    expect(isKeyedSettingsRoute(null)).toBe(false)
    expect(isKeyedSettingsRoute(Symbol('Settings'))).toBe(false)
  })
})
