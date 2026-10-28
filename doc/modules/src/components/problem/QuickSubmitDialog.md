# QuickSubmitDialog（快捷提交对话框）

> 源文件：`src/components/problem/QuickSubmitDialog.vue`

## 职责

题目总览行内的模态提交弹窗：不进入解题页即可完成一次提交并跟踪评测结果。内嵌 CodeEditor 写代码（**语言下拉由 CodeEditor 自带菜单承担**）、文件导入（选择 + HTML5 拖放，按扩展名自动切语言）、提交后在弹窗内展示评测状态行（收敛轮询由 submissionStore 负责），终态后可跳提交详情。

## 核心类型/函数

**props**：`problem: ContestProblem`。**emits**：`close: []`（Esc / 遮罩点击 / 关闭按钮 / 终态后「关闭」）。

| 名称 | 签名 | 用途 |
|------|------|------|
| `allowedLanguages` | `ref<string[]>` | 本题允许的提交语言（HOJ 显示名）：`loadAllowedLanguages()` 经 `problemService.getProblem` 拉取，失败回退 `DEFAULT_LANGUAGES`（不阻断快捷提交）；当前语言经 `resolveAllowedLanguage` 归位，不命中则切首项 |
| `code` / `language` | ref | 代码草稿；默认语言沿用 `workspaceStore.language`（与解题页习惯一致），兜底 `DEFAULT_LANGUAGE` |
| `submitError` / `fileError` | ref | 提交失败原因（store.error 兜底文案）/ 文件导入错误 |
| `preparingSubmission` | `ref<boolean>` | **请求前即置位**的提交锁，覆盖 `whenLoaded()` 等待窗口（此时 store 的 `isSubmitting` 还没被置位）；`finally` 复位 |
| `pendingSubmission` | computed | `submissionStore.pendingFor(contestId, problemId, displayId, language, code)` —— 同一份代码在评测期间复用编号（同内容不重复发送） |
| `submitting` | computed | `preparingSubmission \|\| submissionStore.isSubmitting` |
| `entry` | computed | **优先 `pendingSubmission`**（同内容在途即回显其条目），否则按 `submittedId` 在 `submissionStore.submissions` 中查找 |
| `entryTerminal` | computed | `isTerminalStatus(entry.status)` |
| `submittedId` | `ref<string \| null>` | 本弹窗提交的 ID |
| `doSubmit` | `async fn` | 提交链路，见逻辑流程 |
| `goDetail` | fn | 终态后跳 `SubmissionDetail`（**用 `entry.id`**，与 `pendingSubmission` 复用编号保持一致） |
| `statusToneClasses` / `QUICK_SUBMIT_PENDING_CLASS` | 共享 fn / 本地覆盖 | 使用 `utils/submission-tone` 的基础表，仅 pending 覆盖为原有琥珀色；其余 tone 与评测页/详情页一致 |
| `onKeydown` | fn | window 级 Esc 监听（onMounted 挂 / onUnmounted 摘） |
| `MAX_FILE_BYTES` / `ACCEPTED_EXTENSIONS` | 常量 | 256KB 大小护栏（快捷提交场景无需超大文件）；可导入扩展名白名单**从 `utils/language` 的 `SOURCE_FILE_EXTENSIONS` 派生**（识别面唯一来源，防多处清单漂移）+ `.txt`。语言自动切换经 `hojLanguageOfFileName` 反推 + `resolveAllowedLanguage` 按语言族解析为允许列表中的服务端原名（"Python3" 等变体命中）；无命中保持原选择（拖入 .py 但本题只允许 C++ 时不切换）；`.txt` 不切语言 |
| `pickFile` / `onFileChange` / `extOf` / `ingestFile` | fn | 原生 `input[type=file]`（读后立即重置 value 允许连续选同一文件）；`ingestFile` 校验扩展名与大小 → `file.text()` 读入 → 按扩展名切语言 |
| `dragging` / `dragDepth` / `onDragEnter…onDrop` | ref/计数/fn | HTML5 拖放：`tauri.conf.json` 已关闭 `dragDropEnabled`，事件才能到达 WebView；进入/离开**深度计数**避免掠过子元素时高亮闪烁；拖拽中高亮层接管 drop（Monaco 会吞掉文件拖放事件） |

## 直接依赖

- `vue` / `vue-router`
- 组件：`CodeEditor`（v-model 代码、语言双向绑定、Ctrl+Enter submit 复用）
- `@/utils/focus-scope`（`vFocusScope` —— 对话框焦点陷阱与 Esc 关闭）
- stores：`contestStore`（contestId，`whenLoaded` 兜底）、`submissionStore`（`submitCode` / `pendingFor` / `submissions` / `isSubmitting` / `error`）、`workspaceStore`（仅读 `language` 作默认值）
- `@/services/problem.service`（允许语言列表）
- `@/types/contest`（仅类型）
- `@/utils/language`（`DEFAULT_LANGUAGE` / `DEFAULT_LANGUAGES` / `hojLanguageOfFileName` / `resolveAllowedLanguage` / `SOURCE_FILE_EXTENSIONS`）
- `@/utils/submission`（`formatMemoryKb` / `isTerminalStatus` / `statusLabel` / `statusTone`）
- `@/utils/submission-tone`（共享 pill 类串 + 单项 pending 覆盖）
- `@/utils/logger`（`createLogger` —— 作用域日志）

## 被依赖

- `components/problem/ProblemCard.vue` — 行内 `v-if` 挂载（保证同一时刻至多一个实例），「快捷提交」按钮打开

## 逻辑流程

```
打开（ProblemCard quickSubmitOpen = true）→ 挂载即监听 Esc；卡片挂 v-focus-scope + role="dialog" aria-modal

doSubmit():
  submitting 或 pendingSubmission 命中 → return      // 鼠标 / Ctrl+Enter 同帧只发一次
  空代码（trim 后）→ submitError「代码不能为空」
  preparingSubmission = true                        // 先上锁，覆盖 whenLoaded 等待窗口
  contestId 缺失 → contestStore.whenLoaded() 兜底再取；仍无 → 报错终止
  submissionStore.submitCode(contestId, problem.problemId, problem.displayId, language, code)
    // store 内部：pendingFor 命中即复用编号；否则 isSubmitting 同步上锁
    → 成功记录 submittedId（store 随即启动该提交的收敛轮询）
    → 失败 submitError = store.error ?? '提交失败'
  finally → preparingSubmission = false

评测状态行（entry 非空即显示）:
  非终态 → 旋转圈 +「已提交 #id，评测中…」（轮询回填由 store 驱动）
  终态   → 状态 pill（statusLabel/statusTone）+ 耗时 ms · 内存（formatMemoryKb）
           + 「查看详情」（→ SubmissionDetail）+「关闭」
提交中 → <p role="status" aria-live="polite">正在提交代码，请稍候…</p>
```

设计要点：

- **重复发送防护为三层**：① 组件 `submitting || pendingSubmission` 守卫（同帧多入口）；
  ② `preparingSubmission` **先锁再等数据**（`whenLoaded()` 可能耗时数秒，这段窗口内
  store 的 `isSubmitting` 尚未置位，只靠 store 锁会被连点钻空子）；
  ③ store 侧 `pendingFor` 同内容复用编号 + `isSubmitting` 同步上锁（见
  `stores/submissionStore.md`）。三层缺一都会留下可重复发送的缝隙。
- **即时反馈**：请求发出前就显示「正在提交代码，请稍候…」，受理后立刻变成
  「已提交 #id，评测中…」——选手不必猜「按钮点没点上」。
- **同内容锁定可恢复**：改动代码或语言后可发起新提交（新 payload），改回原 payload
  则恢复锁定（由 `pendingFor` 按五元组匹配，而非永久禁用按钮）。
- **弹窗只观察、不轮询**：提交后的评测收敛轮询完全由 `submissionStore` 负责
  （每条提交独立 Poller、终态/超时自停），本组件仅从 `submissions` 读取条目状态，
  关闭弹窗轮询照常收敛。
- **代码不写入工作区**：快捷提交是一次性提交（`code` 为组件本地 ref），不触碰
  workspaceStore 的代码/脏标记——只借它的 `language` 作默认语言。
- **语言选择统一由 CodeEditor 承担**：头部原生的 `<select>` 已移除，避免同一弹窗里
  出现两套语言控件、两份候选列表。
- 文件导入与 CodeEditor 上传口径一致（扩展名白名单 + `file.text()`，不引入 Tauri
  dialog 插件），额外加 256KB 护栏。
- 拖放依赖 `tauri.conf.json` 关闭 `dragDropEnabled`（Tauri 默认拦截文件拖放，
  WebView 收不到 HTML5 drag 事件）。
- 键盘可访问：遮罩 `@click.self` 关闭、Esc 关闭（window 监听 + `v-focus-scope`）、
  打开即焦点入内且 Tab 循环、提交/评测中按钮 `aria-busy`。

## 测试

`src/components/problem/__tests__/QuickSubmitDialog.spec.ts`（jsdom + Pinia，mock `vue-router` / `submission.service` / `problem.service` / `config.service` / `CodeEditor`）锁定：① 立即显示「提交中」，鼠标点击 + Ctrl+Enter 双入口**只发 1 次**请求；受理后显示「评测中 / 已提交 #s1」并继续锁；改代码后解锁（可再次提交）、改回原代码恢复锁定；轮询转 WA 后显示「Wrong Answer」。② **在等待比赛数据前即上锁**：`whenLoaded` 被调用 1 次，等待期间提交按钮 disabled 且 `submitCode` 未被调用，数据到达后只发 1 次。③ 提交报错显示 `[role=alert]` 文案并解锁以便重试。
