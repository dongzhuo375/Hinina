# EditorSettingsPopover（编辑器设置弹层）

> 源文件：`src/components/editor/EditorSettingsPopover.vue`

## 职责

解题页编辑器工具条「编辑器设置」按钮 + 弹层：字号（滑杆 8–32）、Tab 宽度（2/4/8 分段控件）、编辑器主题（浅色 `vs` / 深色 `vs-dark`），以及「恢复默认」。只呈现与收集意图，**即时应用与落盘由 CodeEditor 承担**。

## 核心类型/函数

**props**：`fontSize: number`、`tabSize: number`、`editorTheme: string`（当前偏好，受控展示）、`error?: string | null`（落盘失败提示，为空时该行不占位）。
**emits**：`change: [patch: { fontSize?: number; tabSize?: number; editorTheme?: string }]`（偏好增量变更）、`reset: []`（恢复默认值）。

| 名称 | 签名 | 用途 |
|------|------|------|
| `open` | `ref<boolean>` | 弹层开合；点击面板外遮罩（`fixed inset-0 z-40`）或右上关闭按钮收起 |
| `tabSizeOptions` | computed | Tab 档位：`EDITOR_TAB_SIZES` ∪ `{当前值}`（升序）。存储域 1–8 宽于候选集，非候选值（手改配置的 6）必须仍有一枚呈选中态，否则用户以为设置丢了 |
| `onFontSizeInput` | `(e: Event) => void` | `input[type=range]` 的 value 是字符串，转数字校验后上抛 `change`（值未变不上抛） |

## 直接依赖

- `vue`
- `@/utils/editor`（`EDITOR_THEMES` / `EDITOR_TAB_SIZES` / `EDITOR_FONT_SIZE_MIN` / `EDITOR_FONT_SIZE_MAX`）

## 被依赖

- `components/editor/CodeEditor.vue` — 工具条右侧（非 readonly 模式）；父级把 `change` 应用到 Monaco 实例并 debounce 落盘，`reset` 走同一条链路

## 逻辑流程

```
点击 tune 按钮 → open = !open
弹层内交互 → emit change（增量）→ CodeEditor：
    Object.assign(prefs, patch) → applyPrefs()（updateOptions 字号/Tab + setTheme 主题）
    → emit prefs-change（父级同步状态行）→ 400ms debounce 落盘
「恢复默认」→ emit reset → CodeEditor 以 utils/editor 默认值走同一条 change 链路
footer：仅落盘失败时出现错误文案（成功路径不打扰），右侧恒为「恢复默认」
```

设计要点：

- **受控组件、无副作用**：弹层不持有 Monaco 实例、不直接写配置 —— 与 `cursor` 上报同款
  单向数据流约定，故任何持有编辑器实例的容器都能复用（当前为解题页与快捷提交对话框）。
- **即时生效优先于落盘**：赛场调字号是「边看边调」，等 IPC 回来才生效不可接受；
  落盘失败只提示「仅本次会话生效」，不回滚已应用的值（把刚调好的字号弹回去更糟）。
- **不做解释性文案**：改动即时可见，弹层不再写「仅编辑器区域」「改动即时生效并自动保存」
  之类的说明 —— 面板内只留控件本身与失败提示。
- 主题候选用分段控件而非卡片列表：弹层需在 45vh 的编辑器容器内不溢出，
  选项说明改挂 `title` 悬浮提示。
- **Tab 档位对非候选值容错**：`tabSizeOptions = 候选 ∪ {当前值}`。把钳位收紧到候选集
  （6 → 4）是另一种做法，但那是静默改写用户配置；这里选择「呈现真实值」。
- 图标内联 SVG；配色沿用工具条既有 slate + `--color-primary` 体系。
