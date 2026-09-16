import { describe, expect, it } from 'vitest'
import {
  DEFAULT_LANGUAGE,
  DEFAULT_LANGUAGES,
  hojLanguageOfFileName,
  monacoIdOf,
  normalizeHojLanguage,
  sourceFileNameOf,
} from '@/utils/language'

describe('monacoIdOf — HOJ 显示名 → Monaco language id', () => {
  it('常见语言与带版本后缀的部署变体归一', () => {
    expect(monacoIdOf('C++')).toBe('cpp')
    expect(monacoIdOf('C++17 (GCC 13.2)')).toBe('cpp')
    expect(monacoIdOf('cpp')).toBe('cpp')
    expect(monacoIdOf('C')).toBe('c')
    expect(monacoIdOf('C (GCC 13.2)')).toBe('c')
    expect(monacoIdOf('Java')).toBe('java')
    expect(monacoIdOf('Java 17 (OpenJDK)')).toBe('java')
    expect(monacoIdOf('Python 3.10')).toBe('python')
    expect(monacoIdOf('PyPy3')).toBe('python')
    expect(monacoIdOf('python')).toBe('python')
    expect(monacoIdOf('Go')).toBe('go')
    expect(monacoIdOf('Rust')).toBe('rust')
    expect(monacoIdOf('C#')).toBe('csharp')
    expect(monacoIdOf('JavaScript')).toBe('javascript')
    expect(monacoIdOf('Kotlin')).toBe('java')
  })

  it('无法识别的语言回退 cpp（高亮错语言好过无高亮）', () => {
    expect(monacoIdOf('')).toBe('cpp')
    expect(monacoIdOf('Brainfuck')).toBe('cpp')
  })
})

describe('sourceFileNameOf — 语言 → 工作区源文件名', () => {
  it('后缀与语言严格一致（判题端按后缀判定语言与倍率）', () => {
    expect(sourceFileNameOf('C')).toBe('main.c')
    expect(sourceFileNameOf('C++')).toBe('main.cpp')
    expect(sourceFileNameOf('C++17')).toBe('main.cpp')
    expect(sourceFileNameOf('Java')).toBe('Main.java')
    expect(sourceFileNameOf('Python')).toBe('main.py')
    expect(sourceFileNameOf('Go')).toBe('main.go')
    expect(sourceFileNameOf('Rust')).toBe('main.rs')
    expect(sourceFileNameOf('C#')).toBe('Main.cs')
    expect(sourceFileNameOf('Kotlin')).toBe('Main.kt')
  })

  it('未知语言回退 main.txt', () => {
    expect(sourceFileNameOf('Brainfuck')).toBe('main.txt')
  })
})

describe('normalizeHojLanguage — 历史值归一为 HOJ 显示名', () => {
  it('旧 Monaco id 映射回显示名（P55 时代工作区/配置的迁移）', () => {
    expect(normalizeHojLanguage('cpp')).toBe('C++')
    expect(normalizeHojLanguage('c')).toBe('C')
    expect(normalizeHojLanguage('java')).toBe('Java')
    expect(normalizeHojLanguage('python')).toBe('Python')
  })

  it('已是显示名或其它非空值原样保留（OJ 可能提供映射外语言）', () => {
    expect(normalizeHojLanguage('C++')).toBe('C++')
    expect(normalizeHojLanguage('Go')).toBe('Go')
    expect(normalizeHojLanguage('C++17 (GCC 13.2)')).toBe('C++17 (GCC 13.2)')
  })

  it('空/缺失回退默认 C++', () => {
    expect(normalizeHojLanguage('')).toBe(DEFAULT_LANGUAGE)
    expect(normalizeHojLanguage('  ')).toBe(DEFAULT_LANGUAGE)
    expect(normalizeHojLanguage(undefined)).toBe(DEFAULT_LANGUAGE)
    expect(normalizeHojLanguage(null)).toBe(DEFAULT_LANGUAGE)
    expect(DEFAULT_LANGUAGE).toBe('C++')
  })
})

describe('hojLanguageOfFileName — 扩展名 → 显示名（快捷提交拖拽）', () => {
  it('识别常见源代码扩展名', () => {
    expect(hojLanguageOfFileName('main.cpp')).toBe('C++')
    expect(hojLanguageOfFileName('solution.cc')).toBe('C++')
    expect(hojLanguageOfFileName('main.c')).toBe('C')
    expect(hojLanguageOfFileName('Main.java')).toBe('Java')
    expect(hojLanguageOfFileName('main.py')).toBe('Python')
  })

  it('无法识别返回 null（调用方保持当前语言选择）', () => {
    expect(hojLanguageOfFileName('notes.txt')).toBeNull()
    expect(hojLanguageOfFileName('main.go')).toBeNull()
    expect(hojLanguageOfFileName('noext')).toBeNull()
  })
})

describe('DEFAULT_LANGUAGES — 内置回退列表', () => {
  it('覆盖校内赛主流语言，且全部可归一为合法 Monaco id', () => {
    expect(DEFAULT_LANGUAGES).toEqual(['C', 'C++', 'Java', 'Python'])
    for (const lang of DEFAULT_LANGUAGES) {
      expect(['c', 'cpp', 'java', 'python']).toContain(monacoIdOf(lang))
    }
  })
})
