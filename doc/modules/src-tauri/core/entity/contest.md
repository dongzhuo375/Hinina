# contest

## 职责
定义比赛实体 `Contest`、比赛题目摘要 `ContestProblem` 及「配置比赛加载结果」`ContestBundle`，支持 Serde 序列化。

## 核心类型
- **`Contest`** — 比赛 struct，字段：`id`, `title`, `start_time`, `end_time`, `description`, `contest_type`, `status`, `auth`（均为 camelCase 序列化）
- **`ContestProblem`** — 比赛题目摘要，字段：`id`, `display_id`, `cid`, `problem_id`, `display_title`, `ac`, `total`
- **`ContestBundle`** — 配置比赛加载结果：`{ contest: Contest, problems: Vec<ContestProblem> }`，作为 `load_configured_contest` Command 的返回值（对象而非元组，供前端直接解构）

## 直接依赖
- `serde::{Deserialize, Serialize}`

## 被依赖
- `core::event::app_event`（ContestEvent::ListLoaded 携带 Vec<Contest>）
- `core::provider::contest`（ContestProvider trait 使用 Contest）
- `service::contest`（`load_contest_with_problems` 返回 ContestBundle）
- `commands::contest_cmd`（`load_configured_contest` 返回 ContestBundle）

## 逻辑流程
无（纯类型定义）。
