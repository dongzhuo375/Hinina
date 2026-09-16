# QuickSubmitDialog（快捷提交对话框）

> 源文件：`src/components/problem/QuickSubmitDialog.vue`

## 职责

题目总览卡片内的模态提交弹窗：不进入解题页即可完成一次提交并跟踪评测结果。内嵌 CodeEditor 写代码、语言下拉、文件导入（选择 + HTML5 拖放，按扩展名自动切语言）、提交后在弹窗内展示评测状态行（收敛轮询由 submissionStore 负责），终态后可跳提交详情。

## 核心类型/函数

**props**：`problem: ContestProblem`。**emits**：`close: []`（Esc / 遮罩点击 / 关闭按钮 / 终态后「关闭」）。

| 名称 | 签名 | 用途 |
|------|------|------|
| `LANGUAGES` | 常量 | C / C++ / Java / Python 四选项 |
| `code` / `language` | ref | 代码草稿；默认语言沿用 `workspaceStore.language`（与解题页习惯一致），兜底 'cpp' |
| `submitError` / `fileError` | ref | 提交失败原因（store.error 兜底文案）/ 文件导入错误 |
| `TONE_CLASSES` | 常量 | `StatusTone` → pill 样式类（与评测页/详情页同一套语义色） |
| `onKeydown` | fn | window 级 Esc 监听（onMounted 挂 / onUnmounted 摘） |
| `MAX_FILE_BYTES` / `EXT_LANG` | 常量 | 256KB 大小护栏（快捷提交场景无需超大文件）；扩展名 → 语言映射（.cpp/.cc/.cxx→cpp、.c→c、.java→java、.py→python、.txt→null 不切语言），白名单外的扩展名直接拒绝 |
| `pickFile` / `onFileChange` / `ingestFile` | fn | 原生 `input[type=file]`（读后立即重置 value 允许连续选同一文件）；ingestFile 校验扩展名与大小 → `file.text()` 读入 → 按扩展名切语言 |
| `dragging` / `dragDepth` / `onDragEnter…onDrop` | ref/计数/fn | HTML5 拖放：`tauri.conf.json` 已关闭 `dragDropEnabled`，事件才能到达 WebView；进入/离开**深度计数**避免掠过子元素时高亮闪烁；拖拽中高亮层接管 drop（Monaco 会吞掉文件拖放事件） |
| `submittedId` / `entry` / `entryTerminal` | ref/computed | 本弹窗提交的 ID → 从 `submissionStore.submissions` 观察对应条目；`isTerminalStatus` 判定是否收敛 |
| `doSubmit` | fn | 提交链路，见逻辑流程 |
| `goDetail` | fn | 终态后跳 `SubmissionDetail` 路由 |

## 直接依赖

- `vue` / `vue-router`
- 组件：`CodeEditor`（v-model 代码、语言双向绑定、Ctrl+Enter submit 复用）
- stores：`contestStore`（contestId，`whenLoaded` 兜底）、`submissionStore`（`submitCode` / `submissions` / `isSubmitting` / `error`）、`workspaceStore`（仅读 `language` 作默认值）
- `@/types/contest`（仅类型）
- `@/utils/submission`（`formatMemoryKb` / `isTerminalStatus` / `statusLabel` / `statusTone` + `StatusTone` 类型）

## 被依赖

- `components/problem/ProblemCard.vue` — 卡片内 `v-if` 挂载（保证同一时刻至多一个实例），「快捷提交」按钮打开

## 逻辑流程

```
打开（ProblemCard quickSubmitOpen = true）→ 挂载即监听 Esc

doSubmit():
  isSubmitting 短路；空代码（trim 后）→ submitError「代码不能为空」
  contestId 缺失 → contestStore.whenLoaded() 兜底再取；仍无 → 报错终止
  submissionStore.submitCode(contestId, problem.problemId, language, code)
    → 成功记录 submittedId（store 内部随即启动该提交的收敛轮询）
    → 失败 submitError = store.error

评测状态行（entry 非空即显示）:
  非终态 → 旋转圈 +「评测中…（#id，结果将自动更新）」   // 轮询回填由 store 驱动
  终态   → 状态 pill（statusLabel/statusTone）+ 耗时 ms · 内存（formatMemoryKb）
           + 「查看详情」（→ SubmissionDetail）+「关闭」
```

设计要点：

- **弹窗只观察、不轮询**：提交后的评测收敛轮询完全由 `submissionStore` 负责
  （每条提交独立 Poller、终态/超时自停），本组件仅从 `submissions` 列表读取条目状态，
  关闭弹窗轮询照常收敛。
- **代码不写入工作区**：快捷提交是一次性提交（`code` 为组件本地 ref），不触碰
  workspaceStore 的代码/脏标记——只借它的 `language` 作默认语言。
- 文件导入与 CodeEditor 上传口径一致（扩展名白名单 + `file.text()`，不引入 Tauri
  dialog 插件），额外加 256KB 护栏。
- 拖放依赖 `tauri.conf.json` 关闭 `dragDropEnabled`（Tauri 默认拦截文件拖放，
  WebView 收不到 HTML5 drag 事件）。
- 遮罩 `@click.self` 关闭、Esc 关闭；提交中按钮禁用 +「提交中…」旋转圈。
