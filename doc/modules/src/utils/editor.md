# editor（编辑器偏好值域）

> 源文件：`src/utils/editor.ts`

## 职责

编辑器偏好取值域的唯一权威模块：Monaco 主题候选与归一、字号上下界与默认值、Tab 宽度候选与默认值。前端所有「编辑器设置」相关校验/兜底都从这里取常量，避免多处硬编码漂移。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `EditorThemeOption` | `{ id: string; label: string; hint: string }` | 主题选项（`id` 即 `monaco.editor.setTheme` 的取值，`label`/`hint` 供弹层展示） |
| `EDITOR_THEMES` | `readonly EditorThemeOption[]` | 可选主题：`vs`（浅色）/ `vs-dark`（深色）——**仅作用于编辑器区域**，客户端界面仍只有浅色（dark UI 未实现） |
| `DEFAULT_EDITOR_THEME` | `'vs'` | 主题兜底值 |
| `DEFAULT_EDITOR_FONT_SIZE` / `EDITOR_FONT_SIZE_MIN` / `EDITOR_FONT_SIZE_MAX` | `14` / `8` / `32` | 字号默认值与上下界（与设置页校验、Rust `sanitize` 同域） |
| `DEFAULT_EDITOR_TAB_SIZE` / `EDITOR_TAB_SIZES` | `4` / `[2, 4, 8]` | Tab 宽度默认值与候选（ICPC 惯例 4） |
| `normalizeEditorTheme` | `(raw: unknown) => string` | 主题归一：未知/非法值回退 `'vs'` |

## 直接依赖

无（纯常量与纯函数模块）。

## 被依赖

- `services/config.service.ts` — `getEditorPrefs` 的字号/Tab 兜底、`editorTheme` 归一，`updateEditorPrefs` 落盘前归一
- `components/editor/EditorSettingsPopover.vue` — 字号滑杆上下界、Tab 候选、主题候选与默认值（「恢复默认」）
- `components/editor/CodeEditor.vue` — 偏好初值（配置读取到达前的占位值）
- `components/editor/EditorConsoleBar.vue` — 缩进状态行的 Tab 宽度兜底
- `views/ProblemSolveView.vue` — 状态行 Tab 宽度兜底

## 逻辑流程

```
normalizeEditorTheme(raw)
  raw ∈ EDITOR_THEMES 的 id → 原样返回
  否则（undefined / '' / 'dracula' / 'VS' …）→ DEFAULT_EDITOR_THEME

值域变更需同步三处：本文件、Rust `EditorConfig::sanitize`（editor_theme 白名单 +
font_size/tab_size 钳制）、`SettingsView` 表单校验
```

设计要点：

- **主题必须归一**：`monaco.editor.setTheme` 收到未知主题名会静默保留上一个主题，
  配置与界面就此不一致且用户无从察觉 —— 归一放在 service 入口，弹层不承担校验。
- **`vs-dark` 只影响编辑器**：`theme.themeName` 恒为 `light`（整机深色 UI 未实现），
  深色偏好落在 `theme.editorTheme`，两条配置互不干扰。
