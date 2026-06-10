# problem

## 职责
定义题目实体 `Problem` 及其样例数据 `Sample`，包含题目标题、描述、输入/输出说明、样例、时空限制，支持 Serde 序列化。

## 核心类型/函数
- **`Problem`** — 题目 struct，字段：`id`, `title`, `description`, `input_description`, `output_description`, `samples`, `time_limit`, `memory_limit`
- **`Sample`** — 样例 struct，字段：`input`, `output`

## 直接依赖
- `serde::{Deserialize, Serialize}`

## 被依赖
- `core::provider::problem`（ProblemProvider trait 使用 Problem）
- `commands::problem_cmd`

## 逻辑流程
无（纯类型定义）。
