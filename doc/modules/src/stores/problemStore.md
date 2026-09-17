# problemStore（题目状态与 limits）

> 源文件：`src/stores/problemStore.ts`

## 职责

持有当前比赛的题目列表、当前打开的题目详情，以及两类渐进补齐的辅助数据：题目 limits（时限/内存）与「我的提交状态」，驱动题目卡片与解题进度展示。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| state.`currentProblem` / `problems` / `isLoading` / `error` | — | 题目详情与列表 |
| state.`limits` | `Record<displayId, ProblemLimits>` | 渐进填充：卡片先渲染，limits 到达后补齐 |
| state.`isLimitsLoading` | `boolean` | limits 加载中标记 |
| state.`myStatus` | `Record<pid, UserProblemStatus>` | 我的提交状态（0=未提交 / 1=已AC / 2=尝试过） |
| state.`isStatusLoading` | `boolean` | 状态加载中标记 |
| `currentProblemId` | getter | 当前打开的题目 ID（无则 null） |
| `limitsOf` | getter `(displayId) => ProblemLimits \| null` | 取某题 limits；**null = 后端获取失败**，视图应显示占位而非假默认值 |
| `statusOf` | getter `(problemId) => UserProblemStatus` | 取某题我的状态；未出现在 map 中视为未提交（0） |
| `openProblem` | `(contestId, problemId) => Promise<void>` | 加载详情并设为当前题目；失败记录 error 并抛出 |
| `loadProblems` | `(contestId) => Promise<void>` | 加载比赛题目列表；失败记录 error 并抛出 |
| `loadLimits` | `(contestId, displayIds) => Promise<void>` | 批量加载 limits（后端双层缓存，命中时零网络请求）；**失败不抛出** |
| `loadMyStatus` | `(contestId, problemIds) => Promise<void>` | 批量加载我的提交状态；**失败不抛出** |

## 直接依赖

- `pinia`
- `@/types/problem`、`@/types/rank`（仅类型：`ProblemLimits` / `UserProblemStatus`）
- `@/services/problem.service`（`problemService`）
- `@/utils/error`（`errorMessage` —— 错误文案收敛）
- `@/utils/logger`（`createLogger` —— 作用域日志）

## 被依赖

- `views/ProblemSetView.vue` — 题目总览（`loadLimits` / `loadMyStatus` 渐进补齐卡片信息）
- `views/ProblemSolveView.vue` — 解题视图（`openProblem` / 当前题目）
- `components/problem/ProblemCard.vue` / `ProblemStatement.vue` / `ProblemTabStrip.vue` — 卡片状态角标、题面 limits 展示、标签页
- `stores/session.ts` — 登出清理 `$reset()`

## 逻辑流程

```
loadLimits(contestId, displayIds)
  → 空列表短路
  → problemService.getProblemLimits（后端内存+磁盘缓存，缺失题不出现在结果里）
  → this.limits = { ...旧值, ...fetched }   // 合并而非替换：分批/重试时已到达的题不丢
  → 失败：log.error 记录（utils/logger 作用域日志），不抛出、不清空已有数据

loadMyStatus(contestId, problemIds)
  → 空列表短路 → problemService.getUserProblemStatus → 整体替换 myStatus
  → 失败：同上，状态缺失按「未提交」展示
```

设计要点：

- **loadLimits / loadMyStatus 失败不抛出**：两者只影响卡片上的一行信息或一个角标，
  不应打断整页渲染；与 `openProblem` / `loadProblems`（主数据，失败必须让调用方感知）
  的抛出策略刻意不同。认证类错误（401/403）已由全局会话守卫（`stores/sessionGuard`）
  统一处理，此处只记录日志，不重复应对。
- **缺失 ≠ 零值**：后端对获取失败的题不产出条目（如 403 私有题），`limitsOf` 返回 null、
  `statusOf` 回退 0，视图用占位符 `—` 表达「未取得」，避免假默认值误导
  （HOJ-Problem-Limits-API.md §9.5）。
- limits 以后端缓存为准，store 不做磁盘持久化；登出 `$reset()` 清空，防止机位复用泄露。
