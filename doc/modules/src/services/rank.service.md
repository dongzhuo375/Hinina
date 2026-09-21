# rank.service（榜单服务）

> 源文件：`src/services/rank.service.ts`

## 职责

比赛排行榜的获取与查询参数编排：把调用方给出的部分查询条件按 HOJ 语义补全默认值后交给 Bridge，是 View/Store 访问榜单数据的唯一业务入口。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `DEFAULT_RANK_PAGE_SIZE` | `50` | 默认分页大小（HOJ 建议值：榜单为全量计算后分页，limit 越大单次越慢）。**这是该默认值的唯一取值点**：后端 `DEFAULT_RANK_LIMIT` 与 `RankQuery::default()` 只是「绕过前端直接调命令」时的防御值，三处必须同值（P71），改动须同步两侧锁定用例 |
| `RankService.getRank` | `(query: Partial<RankQuery> & { contestId: string }) => Promise<ContestRankPage>` | 获取一页榜单；除 contestId 外均可省略 |
| `rankService` | 单例 | 全局唯一实例 |

默认值补全：`currentPage ?? 1`、`limit ?? DEFAULT_RANK_PAGE_SIZE`、`keyword ?? null`、`removeStar ?? false`、`containsEnd ?? false`（`containsEnd` 仅在比赛 `allowEndSubmit=true` 时才真正生效，HOJ §9.5）。

## 直接依赖

- `@/bridge/rank.bridge`（`getContestRank`）
- `@/types/rank`（仅类型）

## 被依赖

- `stores/rankStore.ts` — `loadRank()` 唯一数据通道
- `stores/__tests__/rankStore.spec.ts` — 被 mock（store 测试不触达 IPC）

## 逻辑流程

```
rankStore.loadRank → rankService.getRank(部分参数)
  → 补全 HOJ 默认值 → rank.bridge.getContestRank → IPC get_contest_rank
  → ContestRankPage 原样返回（不做任何归一，去重/参与人数修正在 store.applyPage）
```

设计要点：

- **不做前端缓存**：HOJ 内榜每次实时计算（HOJ-Contest-Rank-API.md §4），缓存会给出过期名次，
  赛场上名次即决策依据，宁缺勿旧。刷新节奏由 `rankStore` 的轮询器控制
  （≥10s + 抖动错峰 + 后台暂停）。
- 分层约定：View/Store 不直接调 `rank.bridge`；本服务只编排参数，不持有状态。
