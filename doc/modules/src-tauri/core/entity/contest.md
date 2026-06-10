# contest

## 职责
定义比赛实体 `Contest`，包含比赛 ID、标题、起止时间戳及题目 ID 列表，支持 Serde 序列化。

## 核心类型/函数
- **`Contest`** — 比赛 struct，字段：`id`, `title`, `start_time`, `end_time`, `problems`

## 直接依赖
- `serde::{Deserialize, Serialize}`

## 被依赖
- `core::event::app_event`（ContestEvent::ListLoaded 携带 Vec<Contest>）
- `core::provider::contest`（ContestProvider trait 使用 Contest）
- `commands::contest_cmd`

## 逻辑流程
无（纯类型定义）。
