# ProblemSolveView（解题工作台）

> 源文件：`src/views/ProblemSolveView.vue`

## 职责

解题页：左题面（48%）/ 右代码编辑器（52%）可拖拽分栏，编排「比赛 → 工作区 → 题目详情」的加载链，处理 Tab 快速切题的并发防护、`?focus=1` 快捷提交联动与代码提交。

## 核心类型/函数

模块级普通变量：`loadToken: number`（并发防护令牌）、`isLoadingPage: boolean`（加载进行中标记，供 focus 监听让位）、`flushingToDisk: boolean`（落盘进行中标记，避免失焦与页面隐藏重复落盘）。

| 名称 | 签名 | 用途 |
|------|------|------|
| `displayId` | computed | 路由参数 `:displayId`（比赛内题号 A/B/C…） |
| `ensureContestId` | `() => Promise<string>` | 深链/刷新直达本页时外壳可能未加载完比赛，`contestStore.whenLoaded()`（P59 统一入口，复用在途请求）后取 id |
| `load` | `(id: string) => Promise<void>` | 加载编排主流程（见逻辑流程），每步之后校验 `token !== loadToken` 则放弃 |
| `consumeFocusQuery` | `() => Promise<void>` | 消费 `?focus=1`：`nextTick` 后调 `codeEditor.focus()`，随后 `router.replace({ query: {} })` 清掉 |
| `splitRatio` | ref | 初始值经 `configService.getSplitRatio()` 从配置读取（P55 消费落地；异步到达时若用户已拖拽则不覆盖） |
| `startDrag` | `(e: MouseEvent) => void` | 分栏拖拽：比例钳制 0.3–0.7，拖拽期间全局锁定 `cursor: col-resize` 与 `user-select: none` |
| `persistSplitRatio` | `() => void` | 拖拽结束把比例经 `configService.updateConfig` 写回配置（下次进入解题页生效）；失败只 `log.error` 记录（`utils/logger` 作用域日志），不打断使用 |
| `handleSubmit` | `() => Promise<void>` | 提交：`submissionStore.submitCode(contestId, problem.id, workspaceStore.language, workspaceStore.code)`；轮询由 store 自动启动 |
| `cursor` | ref | Monaco 光标位置（CodeEditor emit → 本视图 → EditorConsoleBar prop，单向数据流） |
| `flushToDisk` | `(reason: string) => Promise<void>` | `workspaceStore.saveWorkspace()`（内部先推送在途改动再落盘）；带 `flushingToDisk` 去重，失败只 `log.error` 不打断使用 |
| `onVisibilityChange` / `onWindowBlur` | `() => void` | 页面隐藏（`visibilitychange` → hidden）与窗口失焦（`blur`）时落盘；`onMounted` 注册、`onBeforeUnmount` 注销并**再落盘一次**（离开解题页） |
| `editorPrefs` | `ref<EditorPrefs \| null>` | 编辑器偏好（CodeEditor `prefs-change` 上报，挂载读配置后 + 弹层每次改动后到达）；本视图只消费 `tabSize` 供状态行展示缩进宽度，**不重复读配置**（避免与弹层写入竞态） |
| `viewError` | computed | `localError ?? problemStore.error`（本地编排错误优先） |

## 直接依赖

- `vue` / `vue-router`
- 组件：`ProblemTabStrip` / `ProblemStatement` / `CodeEditor` / `EditorConsoleBar` / `LoadingSpinner` / `ErrorMessage`
- stores：`contestStore` / `problemStore` / `workspaceStore` / `submissionStore`
- `@/services/config.service`（分栏比例读取/回写、`EditorPrefs` 类型 —— 配置读写的指定唯一入口，不违反「View 不得 import bridge」约束）
- `@/utils/editor`（`DEFAULT_EDITOR_TAB_SIZE` — 偏好上报到达前的状态行兜底）
- `@/utils/error`（`errorMessage` —— 错误文案收敛）
- `@/utils/logger`（`createLogger` —— 作用域日志）

## 被依赖

- `router/index.ts` — 路由 `ProblemSolve`（`/contest/problem/:displayId`）
- 入口：`ProblemSetView` 的卡片 open、`ProblemTabStrip` 的 chip 切换（原 quickSubmit 跳转 + `?focus=1` 入口已随 ProblemCard 改为卡片内弹窗而移除；`consumeFocusQuery` 机制保留，当前无调用方携带该 query）

## 逻辑流程

```
watch(displayId, load, { immediate: true })     // 挂载 + Tab 切题共用同一入口

load(id):
  token = ++loadToken                            // 并发防护：只认最后一次加载
  1. workspaceStore.isDirty → saveWorkspace()    // 切题前先落盘（代码保留是工作区核心承诺）
     内部先推送在途防抖改动再落盘                // 「刚敲完就切题」也不会写进旧内容
     失败不阻断切题，仅 log.error 记录
  2. ensureContestId()                           // contest 未加载则兜底拉取
  3. contestStore.problems 按 displayId 找 ContestProblem（找不到 → localError）
  4. workspaceStore.loadWorkspace(contestId, cp.problemId)   // 工作区按题目真实 ID(pid) 隔离
     （内部先 flushPendingSync：加载会整体替换 code，不先推送就会丢最后一次编辑）
  5. problemStore.openProblem(contestId, id)                 // 题面详情按比赛内展示题号查询
  5.5 允许语言归位：题目 languages 非空时
      changeLanguage(resolveAllowedLanguage(当前语言, languages) ?? languages[0])
      // 工作区/配置默认存规范名（"C++"），服务端列表可能是部署变体（"C++17 (GCC 13.2)"）
      // 或不含当前语言族；提交参数必须用服务端认得的写法（与 QuickSubmitDialog 同款语义）。
      // changeLanguage 同名短路 → 归位幂等；写回服务端原名让下次加载直接命中
  6. void loadLimits([id]) + void loadMyStatus(全部 pid)      // 补齐题面限制/Tab 状态点，不 await
  每步之后 token !== loadToken → 直接 return（被更新的切题取代）
  finally: 仅最新加载负责 isLoadingPage = false 并 consumeFocusQuery()

?focus=1 联动（两条路径，注册顺序关键）：
  displayId watch 先注册 → load() 同步置位 isLoadingPage
  query.focus watch 后注册 → 仅当 !isLoadingPage 时直接消费（已在本题、只追加 focus 的场景）
  同时变化时由 load 的 finally 统一消费，避免双份 focus/replace
```

设计要点：

- **pid 与 displayId 双轨**：工作区按 `problemId`（pid）隔离——同一题在不同比赛/练习场景
  共享代码；题面按 `displayId` 查询——HOJ 比赛题目详情接口以展示题号为键。
- **loadToken 而非取消请求**：IPC 无法中途取消，用令牌让过期结果静默丢弃，防止快速切题时
  旧题的题面/工作区覆盖新题（竞态）。
- **切题前落盘失败不阻断**：代码保留优先靠 2s 防抖推送（进后端内存）兜底，保存失败只记录——
  阻断切题会让选手卡在坏页面上，比日志更伤。
- **落盘时机由本视图编排**：切题（load 第 1 步）、窗口失焦、页面隐藏、离开解题页
  （`onBeforeUnmount`）四处显式调用 `flushToDisk`；关窗由 `main.ts` 的关窗握手负责。
  后端 auto-save 周期最长 300 秒，这些时刻若只依赖周期，进程退出或内存被替换就会丢改动。
- 消费 focus 后清除 query（保留 params）：避免切题回来或刷新后反复抢焦点；编辑器未就绪时
  静默降级为不聚焦，但 query 仍清除，保证幂等。
- 提交语言取 `workspaceStore.language`（乐观更新 + 后端持久化，见 workspaceStore 文档），
  提交失败原因由 store 写入 `submissionStore.error`，EditorConsoleBar 负责展示。
- **编辑器偏好单向流转**：CodeEditor 读配置 → `prefs-change` 上报 → 本视图转发给
  EditorConsoleBar 的 `tabSize`。本视图不自己读配置：弹层改动是即时的，重复读取只会
  拿到滞后值并与弹层写入竞态。
- 分层约束：View 不 import `@/bridge`；配置读写走 `config.service`（架构指定的配置唯一入口）。
