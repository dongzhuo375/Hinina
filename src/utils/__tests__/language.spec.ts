import { describe, expect, it } from 'vitest'
import {
  DEFAULT_LANGUAGE,
  DEFAULT_LANGUAGES,
  hojLanguageOfFileName,
  monacoIdOf,
  normalizeHojLanguage,
  resolveAllowedLanguage,
  SOURCE_FILE_EXTENSIONS,
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
    expect(hojLanguageOfFileName('main.go')).toBe('Go')
    expect(hojLanguageOfFileName('main.rs')).toBe('Rust')
    expect(hojLanguageOfFileName('Main.kt')).toBe('Kotlin')
    expect(hojLanguageOfFileName('main.js')).toBe('JavaScript')
    expect(hojLanguageOfFileName('Main.cs')).toBe('C#')
  })

  it('无法识别返回 null（调用方保持当前语言选择）', () => {
    expect(hojLanguageOfFileName('notes.txt')).toBeNull()
    expect(hojLanguageOfFileName('data.json')).toBeNull()
    expect(hojLanguageOfFileName('noext')).toBeNull()
  })
})

describe('resolveAllowedLanguage — 识别语言 → 允许列表中的服务端原名', () => {
  const ALLOWED = ['C', 'C++17', 'Java', 'Python3', 'Go']

  it('精确匹配优先', () => {
    expect(resolveAllowedLanguage('Java', ALLOWED)).toBe('Java')
  })

  it('按语言族命中部署变体（HOJ 列表值可能是 Python3/C++17 等写法）', () => {
    expect(resolveAllowedLanguage('Python', ALLOWED)).toBe('Python3')
    expect(resolveAllowedLanguage('C++', ALLOWED)).toBe('C++17')
  })

  it('列表含版本后缀时同样按族命中', () => {
    expect(resolveAllowedLanguage('Python', ['Python 3.10', 'C++'])).toBe('Python 3.10')
  })

  it('本题不允许该语言族时返回 null（调用方保持原选择）', () => {
    expect(resolveAllowedLanguage('Rust', ALLOWED)).toBeNull()
    expect(resolveAllowedLanguage('Python', ['C', 'C++'])).toBeNull()
  })

  it('空识别值或空列表返回 null', () => {
    expect(resolveAllowedLanguage('', ALLOWED)).toBeNull()
    expect(resolveAllowedLanguage('Python', [])).toBeNull()
  })
})

describe('SOURCE_FILE_EXTENSIONS — 识别面唯一来源', () => {
  it('清单由扩展名映射表派生，每一项都能被 hojLanguageOfFileName 识别', () => {
    expect(SOURCE_FILE_EXTENSIONS.length).toBeGreaterThanOrEqual(17)
    for (const ext of SOURCE_FILE_EXTENSIONS) {
      expect(hojLanguageOfFileName(`main${ext}`), ext).not.toBeNull()
    }
  })

  it('覆盖常用后缀（上传白名单/工作区探测/accept 属性共用此清单，防三处漂移）', () => {
    for (const ext of ['.c', '.cpp', '.cc', '.cxx', '.java', '.py', '.go', '.rs']) {
      expect(SOURCE_FILE_EXTENSIONS, ext).toContain(ext)
    }
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
