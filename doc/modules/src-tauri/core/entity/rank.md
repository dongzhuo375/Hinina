# rank

## 职责
定义榜单与题目限制领域实体：`RankCell` / `ContestRankRow` / `ContestRankPage` / `RankQuery` / `ProblemLimits`。把 HOJ 的 ACM 与 OI 两套榜单 VO 归一为 OJ 无关结构（归一动作在 Adapter 完成），上层 Service / Command / 前端不感知赛制差异。全部 camelCase 序列化，与前端 `src/types/rank.ts` 构成跨端契约。

## 核心类型
- **`RankCell`** — 榜单单元格（ACM 与 OI 归一后）。ACM 用 `error_num` / `try_num` / `is_ac` / `is_first_ac` / `ac_time` / `is_after_contest`；OI 用 `score`（该题得分）。关键字段语义：
  - `error_num`：未通过次数（含罚时）。**该题 AC 时 HOJ 前端既有语义是显示 `error_num + 1`**（把 AC 那次计入），原始值不含本次 AC
  - `try_num: Option<i32>`：封榜时段内的提交次数；非封榜为 `None`（封榜期间服务端不写入 is_ac/ac_time），因此 `try_num != null` 即封榜行判据
  - `ac_time: Option<i64>`：AC 时的比赛进度（**秒**，相对 startTime，不是墙钟时间）
  - `is_after_contest`：赛后提交且查询 `containsEnd=true` 时为 true（展示时时间前加 `*`）
  - `is_first_ac`：相同提交时间也算一血
- **`ContestRankRow`** — 榜单行：`rank`（**`-1` 表示打星队伍**，不参与排名）、`uid` / `username` / `realname` / `nickname` / `school` / `gender`（`female` 时前端加背景色）/ `avatar`、`ac`（ACM AC 题数）、`total`（ACM 该用户比赛内总提交数）、`total_time`（**ACM：总罚时秒；OI：总耗时毫秒**，同名不同单位）、`total_score: Option<i64>`（OI 总得分）、`submission_info: HashMap<displayId, RankCell>`（未出现的题目 = 无任何提交记录）、`time_info: HashMap<displayId, i64>`（OI 该题 AC 最优耗时，毫秒）
- **`ContestRankPage`** — 分页榜单（对应 MyBatis-Plus `IPage`）：`records` / `total` / `size` / `current` / `pages`。**坑**：HOJ 会把「当前登录用户」与「关注列表用户」的排名复制一份插到 records 最前面（HOJ-Contest-Rank-API.md §9.2），因此 `total` 略大于真实参赛人数、第 1 页可能出现重复行 —— 渲染前必须按 `uid` 去重，真实人数以去重后非打星行的最大 `rank` 为准（前端 `utils/rank` 实现）
- **`RankQuery`** — 榜单查询参数（对应 HOJ `ContestRankDTO` 的客户端可控部分）：`current_page`（从 1 开始）、`limit`（HOJ 建议 50，榜单为全量计算后分页，limit 越大单次越慢）、`keyword`（服务端只匹配**学校**或**榜单显示名**）、`remove_star`、`contains_end`（仅比赛 `allowEndSubmit=true` 时生效）。`Default`：第 1 页 / 50 条 / 无关键词 / 不移除打星 / 不含赛后提交
- **`ProblemLimits`** — 题目限制：`display_id`（比赛内序号如 "A"）、`time_limit`（**毫秒**）、`memory_limit`（**MB**），均为 C/C++ 基准值，其它语言判题时 ×2（HOJ-Problem-Limits-API.md §4/§5）。只能从题目详情类接口取得（比赛题目列表接口不返回 limits）且对同一题基本不变，故由 `ProblemService` 做内存 + 磁盘缓存

## 直接依赖
- `std::collections::HashMap`
- `serde::{Deserialize, Serialize}`

## 被依赖
- `core::provider::contest`（`ContestProvider::get_contest_rank` 签名使用 `RankQuery` / `ContestRankPage`）
- `adapter::hoj::types`（`ContestRankVO::into_rank_row` / `cell_from_value` 产出 `ContestRankRow` / `RankCell`）
- `adapter::hoj`（`get_contest_rank` 实现组装 `ContestRankPage`）
- `service::contest`（`get_rank` 透传）
- `service::problem`（`load_problem_limits` 返回 `Vec<ProblemLimits>` 并缓存）
- `commands::contest_cmd`（`get_contest_rank` Command）/ `commands::problem_cmd`（`get_contest_problem_limits` Command）
- 前端 `src/types/rank.ts`（字段形状与本文件严格对齐）

## 逻辑流程
无（纯类型定义）。数据流：

```
HOJ ACMContestRankVO / OIContestRankVO（赛制各异）
  → adapter::hoj::types::ContestRankVO（宽松 DTO，submissionInfo 保留 serde_json::Value）
  → into_rank_row / cell_from_value 归一（ACM 对象 / OI 整数得分 → RankCell）
  → ContestRankPage（camelCase JSON 过 IPC）
  → 前端 types/rank.ts → utils/rank 纯函数推导显示文案
```
