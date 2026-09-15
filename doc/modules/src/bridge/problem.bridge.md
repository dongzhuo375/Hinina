# problem.bridge（题目 IPC 桥接）

> 源文件：`src/bridge/problem.bridge.ts`

## 职责

题目相关 Tauri IPC 的薄封装：对 `get_problem` / `list_problems` / `get_user_problem_status` / `get_contest_problem_limits` 四个 Command 做参数透传，不含业务逻辑。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `getProblem` | `(contestId, problemId) => Promise<Problem>` | invoke `get_problem`（后端同时发布 `ProblemEvent::Opened`） |
| `listProblems` | `(contestId) => Promise<Problem[]>` | invoke `list_problems` |
| `getUserProblemStatus` | `(contestId, problemIds: string[]) => Promise<Record<pid, UserProblemStatus>>` | invoke `get_user_problem_status`；返回 `{ pid: 0\|1\|2 }`，未出现的 pid 视为未提交 |
| `getContestProblemLimits` | `(contestId, displayIds: string[]) => Promise<ProblemLimits[]>` | invoke `get_contest_problem_limits`；后端带内存 + 磁盘双层缓存，命中时零网络请求 |

## 直接依赖

- `@/bridge`（`ipcInvoke` 统一出口：错误归一为 `IpcError` + 观察者通知）
- `@/types/problem`、`@/types/rank`（仅类型）

## 被依赖

- `services/problem.service.ts` — 唯一调用方

## 逻辑流程

```
problem.service → 各桥接函数 → ipcInvoke(cmd, { contestId, ... })
  → Rust commands::problem_cmd 对应 Command → ProblemService → ProblemProvider
```

设计要点：

- 本轮新增 `getUserProblemStatus` 与 `getContestProblemLimits` 两个桥接函数，
  与 Rust 端 `commands::problem_cmd` 新增的两个 Command 一一对应（参数名 camelCase，
  Tauri 自动映射到 snake_case 形参）。
- **limits 结果中获取失败的题目不会出现在数组里**（后端部分失败只跳过该题），
  调用方应显示占位而不是假默认值；桥接层不做任何补全。
