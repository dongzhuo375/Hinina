# contest

## 职责
定义比赛实体 `Contest`、比赛题目摘要 `ContestProblem` 及「配置比赛加载结果」`ContestBundle`，支持 Serde 序列化。

## 核心类型
- **`Contest`** — 比赛 struct，字段：`id`, `title`, `start_time`, `end_time`, `description`, `contest_type`, `status`, `auth`（均为 camelCase 序列化）；榜单相关字段：
  - `rank_show_name: String` — 榜单显示名规则（`username` / `realname` / `nickname`），为空时前端回退 username
  - `seal_rank: bool` — 是否封榜（封榜期间榜单只显示尝试次数，不显示通过状态）
  - `seal_rank_time: Option<i64>` — 封榜起始时间（UTC 秒级时间戳）；未封榜或未设置时为 `None`
  - `allow_end_submit: bool` — 是否允许赛后提交（决定榜单查询的 `containsEnd` 是否真正生效）
  - `oi_rank_score_type: Option<String>` — OI 榜单计分规则（`"Recent"` 最近一次 / `"Highest"` 最高分）；非 OI 赛或未设置时为 `None`，前端据此解释 OI 榜单得分口径
- **`ContestProblem`** — 比赛题目摘要，字段：`id`, `display_id`, `cid`, `problem_id`, `display_title`, `ac`, `total`, `color`（气球颜色如 "#FF0000"，驱动题目卡片字母徽章与榜单列头配色，可能为空）
- **`ContestBundle`** — 配置比赛加载结果：`{ contest: Contest, problems: Vec<ContestProblem> }`，作为 `load_configured_contest` Command 的返回值（对象而非元组，供前端直接解构）

## 直接依赖
- `serde::{Deserialize, Serialize}`

## 被依赖
- `core::event::app_event`（ContestEvent::ListLoaded 携带 Vec<Contest>）
- `core::provider::contest`（ContestProvider trait 使用 Contest）
- `adapter::hoj`（`into_contest` 把 ContestVO 映射为 Contest，含榜单相关新字段与 `oi_rank_score_type`）
- `service::contest`（`load_contest_with_problems` 返回 ContestBundle）
- `commands::contest_cmd`（`load_configured_contest` 返回 ContestBundle）
- 前端 `src/types/contest.ts`（camelCase 跨端契约；`rankShowName` / `sealRank` 等驱动榜单渲染）

## 逻辑流程
无（纯类型定义）。
