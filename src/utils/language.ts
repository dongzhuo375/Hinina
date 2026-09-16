/**
 * 语言域唯一权威模块。
 *
 * 值域约定（跨端契约）：语言的**权威值是 HOJ 显示名**（"C" / "C++" / "Java" /
 * "Python" / "Go"…），与 HOJ 提交契约（submit-problem-judge 的 `language: "C++"`）
 * 和题目详情返回的 `languages` 允许列表同源 —— 工作区元数据、配置默认语言、
 * 提交参数一律存显示名。Monaco 高亮 id 与源文件名都是**派生值**，只在消费点
 * 经本模块映射，绝不反向作为存储值（历史上曾用 Monaco id 作权威值，导致提交
 * 语言与 HOJ 契约不符、语言下拉与赛题允许列表脱节）。
 */

/// 题目未返回允许语言时的内置回退列表（覆盖校内赛绝大多数场景）
export const DEFAULT_LANGUAGES: readonly string[] = ['C', 'C++', 'Java', 'Python']

/// 默认语言（配置缺失/非法时的兜底，与 Rust `EditorConfig::default` 一致）
export const DEFAULT_LANGUAGE = 'C++'

/**
 * 严格识别：HOJ 语言显示名 → 语言族 id；无法识别返回 null。
 *
 * 与 `monacoIdOf` 的回退语义区分开：高亮可以回退（错高亮好过无高亮），
 * 但**文件后缀与 limits 倍率判定绝不可猜测**（判题端按后缀判语言，
 * 把未知语言命名成 .cpp 会被当 C++ 评测）。
 */
function monacoIdStrict(language: string): string | null {
  const s = (language ?? '').trim().toLowerCase()
  if (s.startsWith('c++') || s.startsWith('cpp') || s.startsWith('cxx') || s.startsWith('g++')) return 'cpp'
  // c# 必须先于 c 判定（/^c\b/ 会把 "c#" 误判为 C）
  if (s.startsWith('c#') || s.startsWith('csharp')) return 'csharp'
  if (s === 'c' || /^c\b/.test(s)) return 'c'
  // javascript/typescript 必须先于 java 判定（startsWith('java') 会吞掉 "javascript"）
  if (s.startsWith('javascript') || s.startsWith('js')) return 'javascript'
  if (s.startsWith('typescript') || s.startsWith('ts')) return 'typescript'
  if (s.startsWith('java')) return 'java'
  if (s.startsWith('kotlin')) return 'kotlin'
  if (s.startsWith('python') || s.startsWith('py') || s.startsWith('pypy')) return 'python'
  if (s.startsWith('go')) return 'go'
  if (s.startsWith('rust')) return 'rust'
  if (s.startsWith('php')) return 'php'
  if (s.startsWith('ruby')) return 'ruby'
  if (s.startsWith('sql')) return 'sql'
  return null
}

/**
 * HOJ 语言显示名 → Monaco language id（编辑器高亮用）。
 *
 * 服务端/部署差异会带版本后缀（"C++17 (GCC 13.2)"、"Python 3.10"、"PyPy3"），
 * 按前缀归一；Kotlin 无内置高亮以 Java 近似；无法识别回退 'cpp'
 * （赛场绝大多数提交为 C++，高亮错语言好过无高亮）。
 */
export function monacoIdOf(language: string): string {
  const id = monacoIdStrict(language)
  if (id === 'kotlin') return 'java'
  return id ?? 'cpp'
}

/**
 * 是否 C/C++ 族语言（limits 1 倍基准）。
 *
 * 判题端以源文件后缀 `.c`/`.cpp` 为 1 倍基准（HOJ-Problem-Limits-API.md §5）；
 * 空/未知语言返回 false，由调用方按保守放大（×2）处理。
 */
export function isCLikeLanguage(language: string): boolean {
  const id = monacoIdStrict(language)
  return id === 'c' || id === 'cpp'
}

/**
 * HOJ 语言显示名 → 工作区源文件名。
 *
 * 判题端按**源文件后缀**判定语言与倍率，后缀必须与所选语言严格一致；
 * Java/C#/Kotlin 类名约束用大写主名；未知语言回退 main.txt（绝不猜 .cpp）。
 */
export function sourceFileNameOf(language: string): string {
  switch (monacoIdStrict(language)) {
    case 'c':
      return 'main.c'
    case 'cpp':
      return 'main.cpp'
    case 'java':
      return 'Main.java'
    case 'kotlin':
      return 'Main.kt'
    case 'python':
      return 'main.py'
    case 'javascript':
      return 'main.js'
    case 'typescript':
      return 'main.ts'
    case 'go':
      return 'main.go'
    case 'rust':
      return 'main.rs'
    case 'csharp':
      return 'Main.cs'
    case 'php':
      return 'main.php'
    case 'ruby':
      return 'main.rb'
    case 'sql':
      return 'main.sql'
    default:
      return 'main.txt'
  }
}

/// 历史 Monaco id → HOJ 显示名（旧工作区元数据 / 旧配置值的迁移映射）
const HOJ_NAME_BY_LEGACY_ID: Record<string, string> = {
  c: 'C',
  cpp: 'C++',
  java: 'Java',
  python: 'Python',
}

/**
 * 把任意历史值归一为 HOJ 显示名。
 *
 * - 旧 Monaco id（'cpp' 等，P55 时代的工作区元数据/配置）→ 对应显示名；
 * - 已是显示名或其它非空值（"Go"/"Rust"/"C++17"…）→ 原样保留（OJ 可能提供
 *   映射表之外的语言，不得强行归到 C++）；
 * - 空/缺失 → 默认 "C++"。
 */
export function normalizeHojLanguage(raw: string | undefined | null): string {
  if (typeof raw !== 'string') return DEFAULT_LANGUAGE
  const trimmed = raw.trim()
  if (trimmed === '') return DEFAULT_LANGUAGE
  return HOJ_NAME_BY_LEGACY_ID[trimmed.toLowerCase()] ?? trimmed
}

/// 文件扩展名 → HOJ 显示名（上传/拖拽代码文件时自动识别语言）；无法识别返回 null。
/// 调用方须自行校验结果是否在题目允许列表内再切换（见 CodeEditor / QuickSubmitDialog）
export function hojLanguageOfFileName(fileName: string): string | null {
  const ext = fileName.slice(fileName.lastIndexOf('.')).toLowerCase()
  switch (ext) {
    case '.c':
      return 'C'
    case '.cpp':
    case '.cc':
    case '.cxx':
      return 'C++'
    case '.java':
      return 'Java'
    case '.kt':
      return 'Kotlin'
    case '.py':
      return 'Python'
    case '.go':
      return 'Go'
    case '.rs':
      return 'Rust'
    case '.js':
      return 'JavaScript'
    case '.ts':
      return 'TypeScript'
    case '.cs':
      return 'C#'
    case '.php':
      return 'PHP'
    case '.rb':
      return 'Ruby'
    case '.pl':
      return 'Perl'
    case '.hs':
      return 'Haskell'
    case '.sql':
      return 'SQL'
    default:
      return null
  }
}
