# contest

`contest.current(context)` / `contest.get(context, contestId)` / `contest.problems(context)` 需 contest.read；`contest.rank(context, {page, limit})` 需 contest.rank **∩** contest.read，缺任一返回 permission_denied 并指认缺失项。无当前比赛或请求其他比赛时 current/get 返回 null；problems/rank 未选中比赛返回 context_expired（与 submission.list 一致）。只读 Service 方法不调用 load_contest_with_problems，不会选择或切换比赛。

返回 contestId、纯文本 title（最多256字符）、startsAt、endsAt（UTC毫秒）、contestType（'acm'/'oi'，未知值归一 acm）、sealed（`seal_rank && seal_rank_time <= now`，与 RankView 同规则；解释性标志，不是安全闸门——真正的闸门是服务端封榜期间不下发通过状态）、sealedUntil（仅封榜时非 null，UTC 毫秒），不返回密码、HTML 描述或完整领域实体。

problems 返回 `{problemId, displayId, title, color}`：题号 ↔ 气球颜色映射，Hydro 无 color（空串透传）。rank 返回 `{rows, total, page, hasMore}`：行含 rank（-1 打星）、teamKey（与 identity.userKey 同算法，可直接比对自身）、displayName（服务端 rankShowName 规则结果）、school、ac、totalScore、cells（key=displayId；含 solved/firstAc/acTime/errorNum/tryNum/score/afterContest，封榜期间 solved/firstAc 恒 false、acTime 恒 null）。不返回 uid、原始 username、realname、nickname、avatar、gender、totalTime 或任何原始响应。page 1..100、limit 1..200、单行 ≤64 列；跨页无事务快照，按 teamKey 去重；Hydro 单页全量返回，超 200 行返回 limit_exceeded。PluginRankPage 刻意不带 sealed/contestType：一个 API 方法对应一个 service 取数路径。
