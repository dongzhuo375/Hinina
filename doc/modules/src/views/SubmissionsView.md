# SubmissionsView（评测页）

> 源文件：`src/views/SubmissionsView.vue`

## 职责

本人提交历史列表页（`onlyMine` 由后端强制）：题目/状态服务端筛选 + 当前页客户端搜索、分页表格（状态 pill / 耗时 / 内存 / 代码长度）、评测中记录的 5s±1s 视图自有轮询（收敛即停），并作为 `?problem=displayId` 深链的落地页（题目卡片「评测记录」、解题页「提交记录 (n)」均携带该 query 跳入）。

## 核心类型/函数

常量：`POLL_INTERVAL_MS`(5s)、`POLL_JITTER_MS`(1s)。模块级普通变量：`alive`（卸载标记）、`poller: Poller | null`（副作用句柄，不进响应式系统）。

| 名称 | 签名 | 用途 |
|------|------|------|
| `history` | computed | `submissionStore.history`（服务端提交历史：records/total/current/pages/problemFilter/statusFilter/isLoading/error） |
| `statusSelectValue` | computed | 状态筛选下拉的字符串桥接值（`''` = 全部；其余为 HOJ 状态码字符串） |
| `searchQuery` / `visibleRecords` | ref/computed | **当前页**客户端搜索：运行编号 / 题目标题 / 展示题号（displayId/displayPid）小写包含匹配；不发请求 |
| `problemByDisplayId` | computed | displayId → 比赛题目映射（气球色徽章 / 解题页跳转用） |
| `contestElapsed` | `(record) => string` | 赛时相对时间 = `submitTime − contest.startTime`（同为 epoch 秒），经 `formatDurationHms` 格式化；赛前提交钳制 `--:--:--` |
| `TONE_CLASSES` / `toneClasses` | — | `StatusTone` → pill 样式类（ac 绿 / wa 红 / tle 琥珀 / pending 青 / system·neutral 灰） |
| `isBootstrapping` / `isFatalError` / `fatalMessage` | computed | 首次加载（无记录）才铺满 spinner；致命错误 =（history.error 且无记录）或（contestStore.error 且无 contest） |
| `pageItems` | computed | 页码窗口折叠：首尾页 + 当前页 ±1，其余 `…`（与榜单页同一算法） |
| `bootstrap` / `retry` | — | 引导链 / 错误条与致命错误重试（无 contest 时重走 bootstrap） |
| `onProblemFilterChange` / `onStatusFilterChange` | — | 下拉变更 → `setHistoryProblemFilter` / `setHistoryStatusFilter`（store 内部回到第 1 页） |
| `refresh` / `goToPage` / `clearProblemFilter` | — | 重拉当前页 / 翻页（`setHistoryPage`）/ 清除题目筛选 |
| `openDetail` / `openProblem` | — | 行操作 → `SubmissionDetail` 路由；题目单元格 → `ProblemSolve`（displayId 未匹配到比赛题目时不可点） |
| `syncPoller` / `stopPoller` | — | 轮询启停决策，见逻辑流程 |

## 直接依赖

- `vue` / `vue-router`
- 组件：`ErrorMessage` / `LoadingSpinner`
- stores：`contestStore`（比赛标题 / startTime / 题目列表）、`submissionStore`（history 数据源）
- `@/types/contest` / `@/types/submission`（仅类型）
- `@/utils/submission`（`STATUS_OPTIONS` / `formatClock` / `formatCodeLength` / `formatDurationHms` / `formatMemoryKb` / `isJudging` / `statusLabel` / `statusTone` + `StatusTone` 类型）
- `@/utils/polling`（`createPoller` + `Poller` 类型）

## 被依赖

- `router/index.ts` — 路由 `Submissions`（`/contest/submissions`，Contest 外壳子路由）
- 入口链接：`components/problem/ProblemCard.vue`（评测记录）、`components/editor/EditorConsoleBar.vue`（提交记录 (n)）——均携带 `?problem=displayId`

## 逻辑流程

```
onMounted → bootstrap():
  contestStore.whenLoaded()                    // P59 统一入口，复用在途请求
  route.query.problem 为非空字符串
    → submissionStore.setHistoryProblemFilter(contestId, displayId)  // 深链首查即带题目筛选
    → 否则 fetchHistory(contestId, 1)

watch(history.records) → syncPoller():
  当前页存在 isJudging 记录且无 poller → createPoller(5s±1s, task=重拉当前页,
                                        isPaused: document.hidden)
  当前页全部终态 → stopPoller()               // 收敛即停，不做常驻轮询

onUnmounted → alive = false；stopPoller()
```

设计要点：

- **列表轮询与提交收敛轮询取舍相反**：评测页轮询属于「在场才需要」的列表刷新，
  配置 `isPaused: document.hidden` 切后台暂停；而 submissionStore 的单提交收敛轮询
  刻意不暂停（切窗口回来就该看到结果）。节奏也更慢（5s±1s vs 2s 级）。
- **搜索只作用于当前页**（客户端过滤），与服务端筛选（题目/状态，触发重新请求）分层：
  搜索零网络开销，底栏同时展示「共 N 条」与「本页匹配 M 条」管理预期。
- **错误分流与榜单页一致**：有旧数据 → 顶部非阻断错误条（「已保留上一次数据」+ 立即重试），
  绝不清空表格；一条数据都没有 → 整块 ErrorMessage。所有 store 调用 catch 后静默，
  失败原因统一由 `history.error` / `contestStore.error` 承载。
- 评测中的行耗时/内存显示 `-`（服务端尚未回填），状态 pill 带旋转圈。
- 字母徽章底色用 HOJ 气球色（`problemByDisplayId` 匹配），未配置/未匹配回退石板灰。
- 空态区分两种：无提交（带题目筛选时提供「清除题目筛选」按钮）与搜索无匹配。
