# contest.bridge（比赛 IPC 桥接）

> 源文件：`src/bridge/contest.bridge.ts`

## 职责

比赛相关 Tauri IPC 的薄封装：加载配置比赛与匿名比赛列表两个 Command 的透传。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `loadConfiguredContest` | `() => Promise<{ contest: Contest; problems: ContestProblem[] }>` | invoke `load_configured_contest`（后端读 `oj.contest_id`，返回 ContestBundle 对象供直接解构；需有效会话） |
| `listContests` | `() => Promise<Contest[]>` | invoke `list_contests`（匿名接口，登录页比赛简报/倒计时用；后端带 TTL 缓存） |

## 直接依赖

- `@/bridge`（`ipcInvoke`）
- `@/types/contest`（仅类型）

## 被依赖

- `services/contest.service.ts` — 唯一调用方

## 逻辑流程

```
contest.service.loadConfiguredContest / loadContestBrief
  → ipcInvoke('load_configured_contest' | 'list_contests')
  → Rust commands::contest_cmd → ContestService
```

设计要点：

- 两个 Command 的认证要求不同：`load_configured_contest` 需会话（题目列表属比赛数据），
  `list_contests` 可匿名（登录页在会话建立前就要展示比赛简报与倒计时）——桥接层不感知，
  由调用场景自然区分。
