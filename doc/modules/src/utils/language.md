# language（语言域唯一权威模块）

> 源文件：`src/utils/language.ts`

## 职责

定义语言的**权威值域 = HOJ 显示名**（"C" / "C++" / "Java" / "Python" / "Go"…），并提供全部派生映射：Monaco 高亮 id、工作区源文件名、历史值归一、扩展名反推。与 HOJ 提交契约（`submit-problem-judge` 的 `language: "C++"`）和题目详情 `languages` 允许列表同源。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `DEFAULT_LANGUAGES` | `readonly string[]` = `['C','C++','Java','Python']` | 题目未返回允许语言时的内置回退列表 |
| `DEFAULT_LANGUAGE` | `'C++'` | 配置缺失/非法时兜底（与 Rust `EditorConfig::default` 一致） |
| `monacoIdOf` | `(language) => string` | 显示名 → Monaco language id（高亮用）；带版本后缀（"C++17 (GCC 13.2)"/"PyPy3"）按前缀归一；Kotlin 以 Java 近似；**无法识别回退 'cpp'**（高亮错语言好过无高亮） |
| `isCLikeLanguage` | `(language) => boolean` | 是否 C/C++ 族（limits 1 倍基准，判题端按 .c/.cpp 后缀判定）；空/未知返回 false，调用方按保守 ×2 |
| `sourceFileNameOf` | `(language) => string` | 显示名 → 工作区源文件名（C++→main.cpp、Java→Main.java、C#→Main.cs、Kotlin→Main.kt…）；**未知语言回退 main.txt，绝不猜 .cpp**（判题端按后缀判语言，猜错后缀 = 用错语言评测） |
| `normalizeHojLanguage` | `(raw) => string` | 任意历史值 → 显示名：旧 Monaco id（'cpp' 等，P55 时代工作区/配置遗留）映射回显示名；其它非空值原样保留（OJ 可能提供映射外语言）；空 → "C++" |
| `hojLanguageOfFileName` | `(fileName) => string \| null` | 扩展名 → 显示名（上传/拖拽代码文件自动识别语言）；无法识别返回 null。识别面来自 `HOJ_LANGUAGE_BY_EXT` 映射表 |
| `SOURCE_FILE_EXTENSIONS` | `readonly string[]` | 可识别的源代码扩展名集合（**由映射表派生，识别面唯一来源**）—— QuickSubmitDialog 上传白名单、workspaceStore 代码文件探测清单、两处文件选择器 accept 属性全部由它派生，防止多处清单各自维护漂移 |
| `resolveAllowedLanguage` | `(detected, allowed) => string \| null` | 把识别出的语言解析为**允许列表中的服务端原名**：精确匹配优先，否则按语言族（`monacoIdStrict` 同族）命中部署变体（"Python"→列表中的 "Python3"、"C++"→"C++17"）；无命中返回 null。上传/拖拽切语言、解题页加载归位（ProblemSolveView）与对话框初始语言校正都必须经它 —— 提交参数必须用服务端认得的写法 |
| `monacoIdStrict` | 模块内私有 | 严格识别（未知返回 null）—— 区分「高亮可回退」与「后缀/倍率判定不可猜测」两种语义 |

判定顺序陷阱（测试锁定）：`c#` 必须先于 `c`（`/^c\b/` 会吞 "c#"）；`javascript` 必须先于 `java`（`startsWith('java')` 会吞 "javascript"）。

## 直接依赖

无（纯函数模块）。

## 被依赖

- `stores/workspaceStore.ts` — 语言归一 + 源文件名派生
- `components/editor/CodeEditor.vue` — Monaco id 派生 + 默认语言列表
- `components/problem/QuickSubmitDialog.vue` — 扩展名反推 + 默认列表
- `components/problem/ProblemStatement.vue`（经 `utils/limits`）
- `services/config.service.ts` — `getDefaultLanguage` 归一
- `utils/limits.ts` — `isCLikeLanguage` 倍率判定
- `views/SettingsView.vue` / `views/SubmissionDetailView.vue`

## 逻辑流程

```
权威值（HOJ 显示名）流转：
题目详情 languages ─→ CodeEditor 下拉 / QuickSubmitDialog 下拉
工作区元数据 language ─(normalizeHojLanguage)→ workspaceStore.language
config.editor.defaultLanguage ─(normalizeHojLanguage)→ 新建工作区默认语言
提交：workspaceStore.language 原样进 submit_code（= HOJ 契约值）
派生：monacoIdOf → Monaco 高亮；sourceFileNameOf → 落盘文件名；isCLikeLanguage → limits 倍率
```

设计要点：

- **为什么权威值是显示名而非 Monaco id**：HOJ 提交契约与每题允许列表都是显示名；
  Monaco id 有损（C++17/C++20 → 同一个 'cpp'，反向无法还原）且 OJ 可能提供
  Go/Rust/Kotlin 等无对应 id 的语言。历史教训：曾用 Monaco id 作权威值，
  导致提交 language='cpp' 与 HOJ 契约 "C++" 不符、语言下拉与赛题允许列表脱节。
- **回退语义分层**：高亮可回退（monacoIdOf → 'cpp'）；文件名与倍率判定不可猜测
  （monacoIdStrict → null → main.txt / 保守 ×2）。

## 测试

`src/utils/__tests__/language.spec.ts`（10 例）：显示名与部署变体归一（含 c#/javascript 顺序陷阱）、未知回退、源文件名严格一致、历史 id 迁移、空值兜底、扩展名反推、默认列表自洽。
