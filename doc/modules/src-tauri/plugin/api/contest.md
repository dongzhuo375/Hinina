# api/contest

## 职责
只读比赛能力：比赛元信息（含封榜三字段）、题目列表（题号 ↔ 气球颜色）、全场榜单分页。

## 核心类型/函数
- `PluginContestType` — `'acm' / 'oi'`（未知值归一 acm）
- `PluginContest` — contestId、title（≤256 字符）、startsAt/endsAt（UTC ms）、contestType、`sealed`、`sealedSince`
- `PluginContestProblem` — problemId、displayId、title（≤256 B）、color（≤64 B）
- `PluginRankQuery` — `page`（1..100）、`limit`（1..200）
- `PluginRankCell` — displayId、solved、firstAc、acTime、errorNum、tryNum、score、afterContest
- `PluginRankRow` — rank（-1 打星）、teamKey、displayName、school、ac、totalScore、cells
- `PluginRankPage` — rows、page、hasMore（**刻意不带 total**——HOJ 的 total 含服务端前置条目且偏大，行数以去重后非打星行为准；也不带 sealed/contestType）
- `ContestApi`（trait）— `current(context)` / `get(context, contestId)` / `problems(context)` / `rank(context, query)`

## 直接依赖
- `plugin::api`（PluginError / PluginQueryContext / PluginResult）
- `async_trait`、`serde`、`std::collections::HashMap`

## 被依赖
- `plugin::execution_api`（能力转发）、`plugin::query`（取数与投影）

## 逻辑流程
- **权限与作用域**：current / get / problems 需 `contest.read`；rank 需 `contest.rank` **∩** `contest.read`，缺任一返回 `permission_denied` 并指认缺失项。无当前比赛或请求其他比赛时 current/get 返回 null；problems/rank 未选中比赛返回 `context_expired`（与 submission.list 一致）。只读 Service 方法不调用 load_contest_with_problems，不会选择或切换比赛
- **比赛投影**：`sealed` 按 `seal_rank && seal_rank_time <= now` 计算（与 RankView 同规则）——解释性标志，不是安全闸门，真正的闸门是服务端封榜期间不下发通过状态；`sealedSince` 是封榜**开始**时刻（仅封榜时非 null；服务端不下发结束时刻，故无 sealedUntil）。不返回密码、HTML 描述或完整领域实体
- **题目列表**：题号 ↔ 气球颜色映射；Hydro 无 color（空串透传）；title/color 按字节截断（≤256/≤64 UTF-8 字节，千题级比赛仍可编码）
- **榜单投影**：行含 rank、teamKey（与 identity.userKey 同算法，可直接比对自身）、displayName、school、ac、totalScore、cells（封榜期间 solved/firstAc 恒 false、acTime 恒 null）。不返回 uid 字段、原始 username、realname、nickname、avatar、gender、totalTime 或任何原始响应
- **displayName 解析**：宿主按比赛 `rank_show_name` 解析（与前端 `resolveDisplayName` 同规则：realname/nickname/username 选字段，空回退 username，再回退 uid 文本），保证插件榜与 App 榜一致；为此 rank 每次额外读一次比赛元信息（仅名字解析，sealed/contestType 仍只来自 contest.current）。行 uid 为空（Hydro 无 raw 的 user cell）→ 整页 `provider_error`：空 uid 无法生成稳定 teamKey
- **边界与预算**：page 1..100、每页 ≤200 行、单行 ≤64 列、单页总 cells ≤6 000（顶满名字的 200×30 页仍可编码，病态内容由 1 MiB 桥上限兜底）；行文本按字节截断（≤96 UTF-8 字节，字符边界安全）；limit 是提示（Hydro 无视分页、整榜单页返回）；page 回显服务端实际页（Hydro 恒 1），hasMore = current < pages；跨页无事务快照，按 teamKey 去重；榜单查询另有每激活 6 次/分钟独立预算（HOJ 全榜重算昂贵，见 [limits](../limits.md)）
