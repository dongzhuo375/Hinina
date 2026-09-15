# rank（榜单与题目限制跨端契约类型）

> 源文件：`src/types/rank.ts`

## 职责

榜单与题目限制的前后端跨端契约：字段形状与 Rust `core::entity::rank` 严格对齐（camelCase 序列化），HOJ 的 ACM/OI 两套 VO 及原始字段命名差异（`isAC`/`isFirstAC`/`ACTime`）已在 Adapter 归一，前端渲染只依赖本文件的形状。

## 核心类型/函数

| 名称 | 形状 | 关键语义 |
|------|------|----------|
| `RankCell` | `{ errorNum, tryNum, isAc, isFirstAc, acTime, isAfterContest, score }` | `tryNum: number \| null` —— **非 null 即封榜行**（封榜期间服务端不写 isAc/acTime）；`acTime: number \| null` —— AC 时的比赛进度（**秒**，相对 startTime，不是墙钟）；`score: number \| null` —— OI 该题得分（ACM 恒 null）；`errorNum` 不含本次 AC，显示尝试数需 +1 |
| `ContestRankRow` | `{ rank, uid, username, realname, nickname, school, gender, avatar, ac, total, totalTime, totalScore, submissionInfo, timeInfo }` | `rank = -1` 打星队伍；`totalTime` **ACM 为总罚时秒 / OI 为总耗时毫秒**（同名不同单位）；`totalScore: number \| null` 仅 OI；`submissionInfo: Record<displayId, RankCell>`（未出现的题 = 无提交）；`timeInfo: Record<displayId, number>`（OI 最优耗时，**毫秒**） |
| `ContestRankPage` | `{ records, total, size, current, pages }` | MyBatis-Plus IPage；`total` **含服务端前置副本**（当前用户/关注用户），不能当真实参赛人数，records 渲染前须按 uid 去重（HOJ §9.2） |
| `RankQuery` | `{ contestId, currentPage, limit, keyword?, removeStar?, containsEnd? }` | 前端调用态查询参数，见下方「与 Rust 的差异」 |
| `ProblemLimits` | `{ displayId, timeLimit, memoryLimit }` | `timeLimit` **毫秒**、`memoryLimit` **MB**，均为 C/C++ 基准值（其它语言判题 ×2，HOJ-Problem-Limits-API.md §4/§5） |
| `UserProblemStatus` | `0 \| 1 \| 2` | 我的题目状态：0=未提交 / 1=已AC / 2=尝试过；未出现在 map 中视为未提交 |

## 直接依赖

无（纯类型声明文件）

## 被依赖

- `bridge/rank.bridge.ts` / `bridge/problem.bridge.ts`、`services/rank.service.ts` / `problem.service.ts`、`stores/rankStore.ts` / `problemStore.ts`、`utils/rank.ts` / `limits.ts`、`components/rank/*`、`components/problem/*`、`components/contest/ContestStatsBar.vue`（均仅类型引用）
- 对齐来源：Rust `core::entity::rank`（serde camelCase 序列化）

## 逻辑流程

无（纯类型定义）。

设计要点：

- **前端 `RankQuery` 含 `contestId`，Rust 实体 `RankQuery` 不含——刻意的同名不同构**：
  Rust 侧 `ContestProvider::get_contest_rank(contest_id, &query)` 把比赛 ID 作为独立形参
  （Provider 方法一律以 `&str` 传 ID，与 get_contest / list_contest_problems 一致），
  Command 层同样把 `contest_id` 作为独立参数；前端把 contestId 一并装进查询对象，
  让 store → service → bridge 只传一个参数，`bridge/rank.bridge.ts` 在 IPC 边界把它摊平回
  独立字段与 Command 签名对齐。
- 可空字段一律 `| null` 而非 `undefined`/可选：与 Rust `Option` 的 serde 序列化
  （显式 `null`）一一对应，判空口径统一（`x === null` / `x != null`）。
- 单位陷阱集中在注释里：`acTime` 秒、OI `timeInfo`/`totalTime` 毫秒、`timeLimit` 毫秒、
  `memoryLimit` MB——混用会把 OI 用时放大 1000 倍（消费方见 `utils/rank` / `utils/limits`）。
