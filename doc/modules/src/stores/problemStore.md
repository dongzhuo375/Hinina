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
| state.`myStatusStale` | `boolean` | 我的题目状态是否过期。该数据**只由我自己的提交**改变，故不按轮询周期整表重拉：提交终态、**或轮询超时（终态未知）**时 `submissionStore` 调 `invalidateMyStatus()` 置位，总览页在下次可见刷新时重拉一次并清位；初始 `true`（首屏必拉一次） |
| state.`myStatusContestId` | `string \| null` | `myStatus` 数据归属的比赛（null = 尚无数据）。状态表以 **pid** 为键，而同一题可能出现在多场比赛里 —— 切换比赛（设置页改 `contestId`）后必须重拉，否则旧比赛的判定会命中新比赛的卡片。拉取失败不改变归属（表里仍是上次成功那场比赛的数据） |
| `currentProblemId` | getter | 当前打开的题目 ID（无则 null） |
| `limitsOf` | getter `(displayId) => ProblemLimits \| null` | 取某题 limits；**null = 后端获取失败**，视图应显示占位而非假默认值 |
| `statusOf` | getter `(problemId) => UserProblemStatus` | 取某题我的状态；未出现在 map 中视为未提交（0） |
| `openProblem` | `(contestId, problemId) => Promise<void>` | 加载详情并设为当前题目；失败记录 error 并抛出 |
| `loadLimits` | `(contestId, displayIds) => Promise<void>` | 批量加载 limits（后端双层缓存，命中时零网络请求）；**失败不抛出** |
| `loadMyStatus` | `(contestId, problemIds) => Promise<void>` | 批量加载我的提交状态；**失败不抛出**；成功后清除 `myStatusStale`，失败则置为过期（数据仍是旧的）以便下一周期重试 |
| `invalidateMyStatus` | `() => void` | 标记我的题目状态已过期（提交终态、轮询超时时由 `submissionStore` 调用）；**只置位不发请求** —— 重拉交给总览页在下次可见刷新时执行，避免在解题页后台凭空多打一次请求 |
| `myStatusNeedsReloadFor` | `(contestId) => boolean`（getter） | 指定比赛是否需要重拉我的状态：`myStatusStale`（提交终态/超时）**或** `myStatusContestId !== contestId`（切比赛）—— 两个失效维度取并集，总览页只读这一个判据 |

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
     + myStatusStale = false + myStatusContestId = contestId（记录归属）
  → 失败：myStatusStale = true（下一周期重试）+ log.error 记录，归属不变
     （表里仍是上次成功那场比赛的数据），状态缺失按「未提交」展示
```

设计要点：

- **loadLimits / loadMyStatus 失败不抛出**：两者只影响卡片上的一行信息或一个角标，
  不应打断整页渲染；与 `openProblem` / `loadProblems`（主数据，失败必须让调用方感知）
  的抛出策略刻意不同。认证类错误（401/403）已由全局会话守卫（`guards/sessionGuard`）
  统一处理，此处只记录日志，不重复应对。
- **缺失 ≠ 零值**：后端对获取失败的题不产出条目（如 403 私有题），`limitsOf` 返回 null、
  `statusOf` 回退 0，视图用占位符 `—` 表达「未取得」，避免假默认值误导
  （HOJ-Problem-Limits-API.md §9.5）。
- limits 以后端缓存为准，store 不做磁盘持久化；登出 `$reset()` 清空，防止机位复用泄露。
- **myStatus 的失效维度是两个而非一个**：① 过期（我的提交到达终态，或轮询超时导致
  终态未知）；② **数据归属的比赛变化**（设置页可改 `contestId` 后返回总览，而状态表以
  pid 为键、同一题可能出现在多场比赛里）。两者由 `myStatusNeedsReloadFor` 取并集表达，
  总览页只读这一个判据 —— 少任何一个维度都会让卡片 pill 停在别的比赛/别的时刻的判定上。
  注意 `ensureContest()` 在 store 残留旧比赛时会短路，故挂载时拉到的可能是旧比赛数据，
  切比赛后的自愈完全依赖这个判据。

## 测试

`src/stores/__tests__/problemStore.spec.ts`：myStatusStale 初值为真、成功清位、失败置位、
`invalidateMyStatus` 只置位不发请求、空列表短路；数据归属（首次记录 `myStatusContestId`、
同比赛不重拉、换比赛必须重拉、过期与归属取并集、**失败不改变归属**）。
