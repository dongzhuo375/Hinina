# problem（题目跨端契约类型）

> 源文件：`src/types/problem.ts`

## 职责

题目详情与样例的跨端契约类型，与 Rust `core::entity::problem::Problem` 对齐。

## 核心类型/函数

| 名称 | 形状 | 关键语义 |
|------|------|----------|
| `Problem` | `{ id, title, description, inputDescription, outputDescription, samples, timeLimit, memoryLimit, languages }` | `id` 为题目真实 ID（pid 字符串化）；description/input/output 为 Markdown 原文（渲染经 `utils/markdown`）；`timeLimit` **毫秒** / `memoryLimit` **MB**，为 C/C++ 基准值（来源 HOJ ProblemVO，其它语言判题 ×2，见 `utils/limits`）；`languages` 为本题允许的提交语言（**HOJ 显示名**，如 "C++"，来源 get-contest-problem-details；空数组 = 服务端未提供，消费方回退 `utils/language` 的 `DEFAULT_LANGUAGES`） |
| `Sample` | `{ input: string; output: string }` | 样例对（Rust 侧从 HOJ 的 `<input>/<output>` HTML 成对提取） |

## 直接依赖

无（纯类型声明文件）

## 被依赖

- `bridge/problem.bridge.ts`、`services/problem.service.ts`、`stores/problemStore.ts`、`components/problem/ProblemStatement.vue`（均仅类型引用）

## 逻辑流程

无（纯类型定义）。

设计要点：

- **limits 只是兜底来源**：比赛题目列表接口不返回 limits，`Problem.timeLimit/memoryLimit`
  仅在题目详情接口可达时有值（列表构造的摘要 Problem 中为 0）；题面头部优先取
  limits API 精确值（`problemStore.limitsOf`），本字段作回退，两者皆无显示 `—`
  （见 `components/problem/ProblemStatement.md`）。
- 无 `hint` 字段：HOJ ProblemVO 有 hint 但 Rust 实体未透传，题面按约定跳过提示节。
