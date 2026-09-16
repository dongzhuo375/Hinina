# problem

## 职责
定义题目实体 `Problem` 及其样例数据 `Sample`，包含题目标题、描述、输入/输出说明、样例、时空限制、允许提交语言列表，支持 Serde 序列化。

## 核心类型/函数
- **`Problem`** — 题目 struct，字段：`id`, `title`, `description`, `input_description`, `output_description`, `samples`, `time_limit`, `memory_limit`, `languages`
  - `languages: Vec<String>` — 题目允许的提交语言（HOJ 显示名，如 `"C++"`）；来自 `get-contest-problem-details`，空列表表示服务端未提供，前端回退内置默认。带 `#[serde(default)]`，缺少该字段的旧 JSON 仍可反序列化
- **`Sample`** — 样例 struct，字段：`input`, `output`

## 直接依赖
- `serde::{Deserialize, Serialize}`

## 被依赖
- `core::provider::problem`（ProblemProvider trait 使用 Problem）
- `commands::problem_cmd`

## 逻辑流程
无（纯类型定义）。
