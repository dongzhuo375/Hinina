# rank.bridge（榜单 IPC 桥接）

> 源文件：`src/bridge/rank.bridge.ts`

## 职责

榜单相关 Tauri IPC 的薄封装：把 `RankQuery` 透传为 `get_contest_rank` Command 的参数对象，不做任何业务处理。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `getContestRank` | `(query: RankQuery) => Promise<ContestRankPage>` | invoke `get_contest_rank`，参数 `{ contestId, currentPage, limit, keyword, removeStar, containsEnd }` |

可选参数在桥接层归一：`keyword ?? null`、`removeStar ?? false`、`containsEnd ?? false`，保证 Rust 端 `Option` 参数收到确定值。

## 直接依赖

- `@/bridge`（`ipcInvoke` 统一出口：错误归一为 `IpcError` + 观察者通知）
- `@/types/rank`（仅类型）

## 被依赖

- `services/rank.service.ts` — 唯一调用方

## 逻辑流程

```
rank.service.getRank → getContestRank(query)
  → ipcInvoke('get_contest_rank', { contestId, currentPage, limit, keyword, removeStar, containsEnd })
  → Rust commands::contest_cmd::get_contest_rank → ContestService::get_rank
  → ContestRankPage 原样返回
```

设计要点：

- 返回的 records 可能含服务端前置的「当前用户/关注用户」副本（HOJ §9.2），
  **去重与参与人数修正由 `utils/rank` 的纯函数处理**，Bridge 层只做 IPC——
  保持桥接层无逻辑，跨端契约的形状差异全部收敛在 types 与 utils。
