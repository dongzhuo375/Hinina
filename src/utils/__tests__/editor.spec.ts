import { describe, expect, it } from 'vitest'
import {
  DEFAULT_EDITOR_TAB_SIZE,
  DEFAULT_EDITOR_THEME,
  EDITOR_TAB_SIZES,
  EDITOR_THEMES,
  normalizeEditorTheme,
} from '@/utils/editor'

describe('normalizeEditorTheme — Monaco 主题值域收敛', () => {
  it('取值域内的主题原样返回', () => {
    expect(normalizeEditorTheme('vs')).toBe('vs')
    expect(normalizeEditorTheme('vs-dark')).toBe('vs-dark')
  })

  it('未知/非法值回退默认（setTheme 收到未知主题名会静默无效）', () => {
    for (const raw of [undefined, null, '', 'dracula', 'VS', 'vs dark', 0, {}]) {
      expect(normalizeEditorTheme(raw)).toBe(DEFAULT_EDITOR_THEME)
    }
  })
})

describe('编辑器偏好值域常量', () => {
  it('主题候选 id 唯一且默认值在候选内', () => {
    const ids = EDITOR_THEMES.map((t) => t.id)
    expect(new Set(ids).size).toBe(ids.length)
    expect(ids).toContain(DEFAULT_EDITOR_THEME)
  })

  it('Tab 宽度候选与默认值一致（ICPC 惯例 4）', () => {
    expect(EDITOR_TAB_SIZES).toEqual([2, 4, 8])
    expect(EDITOR_TAB_SIZES).toContain(DEFAULT_EDITOR_TAB_SIZE)
  })
})
