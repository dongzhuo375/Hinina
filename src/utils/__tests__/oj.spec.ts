import { describe, expect, it } from 'vitest'
import { OJ_TYPES, ojBaseUrlHint, ojSelectOptions } from '@/utils/oj'

/**
 * OJ 下拉候选的取值域与顺序契约。
 *
 * 这条链路直接决定「能不能从 UI 启用一个新 OJ」：候选若只来自配置实例，
 * 用户永远无法启用尚未配置的 OJ（只能手改 config.json）。
 */

describe('ojSelectOptions', () => {
  it('已知 OJ 全部列出，未配置者标注 configured: false', () => {
    const options = ojSelectOptions([{ id: 'HOJ', enabled: true }])

    // 枚举里的每个类型都要出现（未配置的也在 —— 那正是「启用新 OJ」的入口）
    for (const type of OJ_TYPES) {
      expect(options.some((o) => o.id === type)).toBe(true)
    }
    const hydro = options.find((o) => o.id === 'Hydro')
    expect(hydro).toEqual({ id: 'Hydro', configured: false, disabled: false })

    const hoj = options.find((o) => o.id === 'HOJ')
    expect(hoj).toEqual({ id: 'HOJ', configured: true, disabled: false })
  })

  it('禁用实例仍列出但标为 disabled（配置不得在 UI 里凭空消失）', () => {
    const options = ojSelectOptions([{ id: 'Hydro', enabled: false }])
    expect(options.find((o) => o.id === 'Hydro')).toEqual({
      id: 'Hydro',
      configured: true,
      disabled: true,
    })
  })

  it('枚举外的 id 追加在后（手改配置的私有部署不丢信息）', () => {
    const options = ojSelectOptions([
      { id: 'MySchoolOJ', enabled: true },
      { id: 'HOJ', enabled: true },
    ])

    // 顺序：已知枚举在前，未知追加在后
    const ids = options.map((o) => o.id)
    expect(ids.slice(0, OJ_TYPES.length)).toEqual([...OJ_TYPES])
    expect(ids[ids.length - 1]).toBe('MySchoolOJ')
    expect(options.at(-1)).toEqual({ id: 'MySchoolOJ', configured: true, disabled: false })
  })

  it('实例清单为空时仍列出全部已知类型（首次启动即可选新 OJ）', () => {
    const options = ojSelectOptions([])
    expect(options.map((o) => o.id)).toEqual([...OJ_TYPES])
    expect(options.every((o) => !o.configured)).toBe(true)
  })

  it('同一 id 不会重复出现（枚举与配置重合时只留一项）', () => {
    const options = ojSelectOptions([
      { id: 'HOJ', enabled: true },
      { id: 'Hydro', enabled: true },
    ])
    const ids = options.map((o) => o.id)
    expect(new Set(ids).size).toBe(ids.length)
    expect(ids).toEqual([...OJ_TYPES])
  })
})

describe('ojBaseUrlHint', () => {
  it('已知 OJ 给专属示例地址，未知类型回退通用示例', () => {
    expect(ojBaseUrlHint('HOJ')).toContain('hoj')
    expect(ojBaseUrlHint('Hydro')).toContain('hydro')
    expect(ojBaseUrlHint('MySchoolOJ')).toBe('https://example-oj.com')
  })
})
