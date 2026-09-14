# EditorConsoleBar（编辑器底部控制台条）

> 源文件：`src/components/editor/EditorConsoleBar.vue`

## 职责

编辑器下方状态区：本题最新一条评测记录 pill（状态色 + 图标 + 耗时）、提交失败信息、提交记录入口与「提交代码」按钮、光标位置 / 编码 / 缩进状态行。

## 核心类型/函数

**props**：`cursor: { line, column } | null`（Monaco 光标，由 CodeEditor 经父级转发；null = 尚未产生光标事件）、`problemId: string | null`（当前题 pid，用于筛「本题最新一条」）。
**emits**：`submit: []`。

| 名称 | 签名 | 用途 |
|------|------|------|
| `latest` | computed | 从 `submissionStore.submissions`（只存本会话提交）尾部倒序找本题最新一条；无本地提交为 null，该区域**留空不显示假数据** |
| `toneOf(status)` | `(JudgementStatus) => Tone` | 状态 → 语义色：AC=success；TLE/MLE=warning；Pending/Compiling/Running=pending（旋转圈图标）；Unknown=muted；WA/RE/CE=error |
| `TONE_STYLES` / `toneStyle` | — | Tone → global.css 语义色变量（fg/bg/border） |
| `statusLabel(status)` | `(JudgementStatus) => string` | 驼峰拆空格：`WrongAnswer → Wrong Answer`。**不缩写成 AC/WA**——状态文案以后端 JudgementStatus 原词为准，与判题语义一一对应，避免歧义（如 SF/PA 都被归入 WrongAnswer，缩写会掩盖归并事实） |
| `timeLabel` | computed | 轮询回填的运行耗时（`entry.time`，ms）；未到终态无值不显示 |
| `goSubmissions` | fn | 跳转评测占位页（Submissions 路由） |

状态行常量：`UTF-8`（工作区文件由 Rust 后端以 UTF-8 落盘）、`Spaces: 4`（Monaco tabSize=4 且未关 insertSpaces）——二者是固定事实，按常量展示而非实时读取；`Ctrl + Enter 快捷提交` 快捷键提示。

## 直接依赖

- `vue` / `vue-router`
- `@/stores/submissionStore`（`submissions` / `error` / `isSubmitting`）
- `@/types/submission`（仅 `JudgementStatus` 类型）

## 被依赖

- `views/ProblemSolveView.vue` — 右栏编辑器下方（submit 事件与 CodeEditor 共用同一 `handleSubmit`）

## 逻辑流程

```
submissionStore.submissions 变化（提交/轮询回填）
  → latest 重算 → toneOf/statusLabel/timeLabel → pill 渲染
  非终态：pending 色 + 旋转圈；终态：勾/叉/警告图标 + 耗时
submissionStore.error 非空 → pill 旁截断展示失败原因（title 悬浮全文）
提交按钮：isSubmitting 时禁用 + 「提交中…」旋转圈，点击 emit submit
```

设计要点：

- **只信本会话提交列表**：提交历史接口本轮未接入，不从服务端拉「最新记录」，
  没有本地提交就留空——显示别人的/过期的记录比留空更危险。
- 状态文案不缩写（见 `statusLabel` 注释），色彩语义与 global.css 变量对齐，
  与榜单/题目卡片的状态色体系一致。
- 组件不触 service/bridge，评测数据一律经 submissionStore。
