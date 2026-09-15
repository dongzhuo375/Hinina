# problem.service（题目服务）

> 源文件：`src/services/problem.service.ts`

## 职责

题目领域业务逻辑的编排层：题目详情/列表获取、我的提交状态与题目 limits 的批量查询，屏蔽 IPC 细节并做入参短路与结果整形，是 Store 访问题目数据的唯一入口。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `ProblemService.getProblem` | `(contestId, problemId) => Promise<Problem>` | 获取单个题目详情 |
| `ProblemService.listProblems` | `(contestId) => Promise<Problem[]>` | 列出比赛下所有题目 |
| `ProblemService.getUserProblemStatus` | `(contestId, problemIds: string[]) => Promise<Record<pid, UserProblemStatus>>` | 批量获取我的提交状态（0=未提交 / 1=已AC / 2=尝试过）；空列表直接返回 `{}`，不发无意义 IPC |
| `ProblemService.getProblemLimits` | `(contestId, displayIds: string[]) => Promise<Record<displayId, ProblemLimits>>` | 批量获取 limits（后端带双层缓存）；空列表短路；把后端返回的数组按 `displayId` 索引为 Record |
| `problemService` | 单例 | 全局唯一实例 |

## 直接依赖

- `@/bridge/problem.bridge`（四个桥接函数）
- `@/types/problem`、`@/types/rank`（仅类型）

## 被依赖

- `stores/problemStore.ts` — 唯一调用方（分层约定：View/Store 不直接调 bridge）

## 逻辑流程

```
getProblem / listProblems → 对应 bridge 函数原样透传

getUserProblemStatus(contestId, problemIds)
  → problemIds 为空 → {}（零 IPC）
  → problem.bridge.getUserProblemStatus → Record<pid, 0|1|2>

getProblemLimits(contestId, displayIds)
  → displayIds 为空 → {}（零 IPC）
  → problem.bridge.getContestProblemLimits → ProblemLimits[]（顺序与入参一致）
  → Object.fromEntries 按 displayId 索引为 Record
```

设计要点：

- **缺失即失败**：`getProblemLimits` 结果中缺失的题表示后端获取失败（如 403 私有题
  不可访问、单题详情请求失败被跳过），调用方应显示占位而不是回退成假默认值
  （HOJ-Problem-Limits-API.md §9.5）；服务层不补零、不填充。
- 数组 → Record 的整形放在服务层而非视图，多个消费方（题目卡片、提交页）共享同一形状。
- 无状态、无缓存：limits 缓存归后端（内存 + 磁盘），提交状态实时性要求高不宜前端缓存。
