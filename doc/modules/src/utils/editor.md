# editor（编辑器偏好值域）

> 源文件：`src/utils/editor.ts`

## 职责

编辑器偏好取值域的唯一权威模块：Monaco 主题候选与归一、字号上下界与默认值、Tab 宽度存储域与候选档位。前端所有「编辑器设置」相关校验/兜底都从这里取常量（`config.service` 的钳位、设置页表单校验、解题页弹层候选），不再有第二份字面量。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `EditorThemeOption` | `{ id: string; label: string; hint: string }` | 主题选项（`id` 即 `monaco.editor.setTheme` 的取值，`label`/`hint` 供弹层展示） |
| `EDITOR_THEMES` | `readonly EditorThemeOption[]` | 可选主题：`vs`（浅色）/ `vs-dark`（深色）——**仅作用于编辑器区域**，客户端界面仍只有浅色（dark UI 未实现） |
| `DEFAULT_EDITOR_THEME` | `'vs'` | 主题兜底值 |
| `DEFAULT_EDITOR_FONT_SIZE` / `EDITOR_FONT_SIZE_MIN` / `EDITOR_FONT_SIZE_MAX` | `14` / `8` / `32` | 字号默认值与上下界（与设置页校验、Rust `sanitize` 同域） |
| `DEFAULT_EDITOR_TAB_SIZE` | `4` | Tab 宽度默认值（ICPC 惯例 4） |
| `EDITOR_TAB_SIZE_MIN` / `EDITOR_TAB_SIZE_MAX` | `1` / `8` | Tab 宽度**存储域**（与 Rust `sanitize` 一致）；刻意宽于候选集 —— 候选是 UI 快捷档位，非候选值（如手改配置写入的 6）仍是合法宽度 |
| `EDITOR_TAB_SIZES` | `[2, 4, 8]` | Tab 宽度候选档位（弹层/设置页的快捷按钮） |
| `normalizeEditorTheme` | `(raw: unknown) => string` | 主题归一：未知/非法值回退 `'vs'` |

## 直接依赖

无（纯常量与纯函数模块）。

## 被依赖

- `services/config.service.ts` — `getEditorPrefs` 的字号/Tab 钳位（上下界取本模块常量）、`editorTheme` 归一，`updateEditorPrefs` 落盘前归一
- `components/editor/EditorSettingsPopover.vue` — 字号滑杆上下界、Tab 候选、主题候选与默认值（「恢复默认」）
- `components/editor/CodeEditor.vue` — 偏好初值（配置读取到达前的占位值）
- `components/editor/EditorConsoleBar.vue` — 缩进状态行的 Tab 宽度兜底
- `views/ProblemSolveView.vue` — 状态行 Tab 宽度兜底
- `views/SettingsView.vue` — 编辑器分组的 Tab 档位与字号校验上下界

## 逻辑流程

```
normalizeEditorTheme(raw)
  raw ∈ EDITOR_THEMES 的 id → 原样返回
  否则（undefined / '' / 'dracula' / 'VS' …）→ DEFAULT_EDITOR_THEME

值域变更需同步两处：本文件、Rust `EditorConfig::sanitize`（editor_theme 白名单 +
font_size/tab_size 钳制）。前端消费方一律 import 本模块常量，禁止再写字面量
（曾出现第 4 份拷贝：config.service 里硬编码 8/32/1/8）
```

设计要点：

- **主题必须归一**：`monaco.editor.setTheme` 收到未知主题名会静默保留上一个主题，
  配置与界面就此不一致且用户无从察觉 —— 归一放在 service 入口，弹层不承担校验。
- **`vs-dark` 只影响编辑器**：`theme.themeName` 恒为 `light`（整机深色 UI 未实现），
  深色偏好落在 `theme.editorTheme`，两条配置互不干扰。
- **存储域 ≠ 候选集**：Tab 宽度存储域是 1–8（与 Rust `sanitize` 同域，手改配置的 6 合法），
  候选档位只是弹层/设置页的快捷按钮 —— 弹层对非候选值补一枚档位呈现（见
  `EditorSettingsPopover.tabSizeOptions`），而不是把 6 悄悄改写成 4。
