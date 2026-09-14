# submissionStore（提交记录与评测轮询）

> 源文件：`src/stores/submissionStore.ts`

## 职责

持有本次会话的提交记录列表，编排「提交 → 轮询评测 → 终态停止」的完整链路：终态判据复用 `utils/submission`，节奏参数来自 `configService`，总超时兜底防止无限轮询。

## 核心类型/函数

模块级 `pollTimers: Map<submissionId, setInterval 句柄>` — 定时器是副作用句柄而非渲染状态，放模块作用域（不进响应式系统），由 `stopPolling` / `stopAllPolling` 统一回收。

| 名称 | 签名 | 用途 |
|------|------|------|
| `SubmissionEntry` | interface（模块内） | `{ id, problemId, status, time?, memory?, submittedAt }` —— 本会话提交条目；`time`(ms)/`memory`(KB) 由轮询回填 |
| state | `submissions` / `isSubmitting` / `error` | 提交列表（EditorConsoleBar 取「本题最新一条」的数据源）与请求状态 |
| `submitCode` | `(contestId, problemId, language, sourceCode) => Promise<string>` | 提交 → 追加 Pending 条目 → 启动轮询；提交失败记录 error 并抛出 |
| `pollResult` | `(submissionId) => Promise<JudgementResult>` | 单次查询并回填条目的 `status` / `time = result.timeMs` / **`memory = result.memoryKb`**（内存同样回填，否则控制台条/提交列表只能显示耗时） |
| `startPolling` | `(submissionId) => Promise<void>` | 幂等启动：先停该提交已有定时器；读 `configService.getPollSchedule()`（失败有 2s/300s 兜底）后按 interval 注册 `pollOnce` |
| `pollOnce` | `(submissionId, deadline) => Promise<void>` | 超过 deadline → warn + 停止；否则查询一次，`isTerminalStatus` 命中即停止；**瞬时失败静默等下一周期**（由总超时兜底） |
| `stopPolling` / `stopAllPolling` | `(submissionId?) => void` | 停单个 / 停全部（登出经 `stores/session` 调用，防止定时器脱离会话继续请求） |

## 直接依赖

- `pinia`
- `@/types/submission`（仅 `JudgementStatus` 类型）
- `@/services/submission.service`（提交与查询）
- `@/services/config.service`（轮询间隔与总超时）
- `@/utils/submission`（`isTerminalStatus` 终态判据）

## 被依赖

- `views/ProblemSolveView.vue` — `handleSubmit` 调 `submitCode`（轮询随提交自动启动）
- `components/editor/EditorConsoleBar.vue` — `submissions` / `error` / `isSubmitting` 渲染最新记录 pill
- `stores/session.ts` — 登出清理：`stopAllPolling()` + `$reset()`

## 逻辑流程

```
submitCode
  → submissionService.submitCode → submissionId
  → submissions.push({ id, problemId, status: 'Pending', submittedAt })
  → startPolling(submissionId)      // 与提交结果解耦：提交已成功，轮询参数读取失败也有兜底值
     ├─ stopPolling(id)（幂等）
     ├─ getPollSchedule() → { intervalMs, timeoutMs }
     ├─ 配置读取期间会话可能已被清理（登出/切号）→ 条目不存在则不注册定时器
     └─ deadline = now + timeoutMs；setInterval(pollOnce, intervalMs) → pollTimers.set

pollOnce(id, deadline)
  now ≥ deadline → warn + stopPolling（总超时兜底，绝不无限轮询）
  pollResult → 回填 status/time/memory → isTerminalStatus(status) → stopPolling
  抛错（网络抖动）→ 吞掉，等下一周期
```

设计要点：

- **终态判据单一来源**：`utils/submission.isTerminalStatus`（与 Rust `is_terminal_status`
  对齐，`Unknown` 视为终态防止 OLE/SE/RJE 无限轮询）——store 不自建状态列表。
- **总超时兜底**：即使服务端一直返回非终态（评测机卡死），deadline（默认 300s）也会
  停止轮询；瞬时网络失败不中断循环，靠 deadline 收敛。
- **句柄放模块级 Map**：进响应式系统会被 Vue 深层代理且 `$reset()` 清不掉；
  登出时必须先 `stopAllPolling()` 再 `$reset()`（见 `stores/session.md`）。
- 提交列表只存**本会话**提交：提交历史接口未接入，EditorConsoleBar 据此显示
  「最新记录」，无本地提交则留空不造假。
- 未用 `utils/polling` 的 createPoller：本 store 的轮询是「每提交一个 id 一条有限生命
  周期（终态/超时即停）」的 setInterval，与榜单「页面级无限周期 + 抖动错峰」模型不同。
