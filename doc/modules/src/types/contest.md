# contest（比赛跨端契约类型）

> 源文件：`src/types/contest.ts`

## 职责

比赛实体与比赛题目摘要的跨端契约类型，字段形状与 Rust `core::entity::contest`（camelCase 序列化）严格对齐。

## 核心类型/函数

| 名称 | 形状 | 关键语义 |
|------|------|----------|
| `Contest` | `{ id, title, startTime, endTime, description, contestType, status, auth, rankShowName, sealRank, sealRankTime, allowEndSubmit, oiRankScoreType }` | `startTime`/`endTime` 为 **UTC 秒级时间戳**（各 Adapter 负责统一转换；消费方 `new Date(x * 1000)`）；`contestType` 0=ACM / 1=OI；`status` -1=未开始 / 0=进行中 / 1=已结束（列表接口快照，可能过期，阶段跃迁以本地时钟为准，见 `utils/contest`）；`auth` 0=公开 / 1=私有 / 2=保护 |
| — 榜单相关 5 字段 | `rankShowName: string`（空串回退 username）、`sealRank: boolean`、`sealRankTime: number \| null`（**UTC 秒**，未设置为 null）、`allowEndSubmit: boolean`、`oiRankScoreType: string \| null`（OI 计分规则 "Recent"=最后一次提交 / "Highest"=最高分，**服务端只读属性**，ACM 或未返回为 null） | 驱动榜单显示名、封榜提示（RankView 只判 `now >= sealRankTime` 下界）、`containsEnd` 是否生效与 OI 计分规则徽章 |
| `ContestProblem` | `{ id, displayId, cid, problemId, displayTitle, ac, total, color }` | `displayId` 比赛内题号（"A"/"B"…）、`problemId` 题目真实 ID（pid，字符串化的 HOJ pid）——**双 ID 分工**：工作区按 pid 隔离、题面按 displayId 查询；**`cid: string`**（各 OJ 比赛主键形态不同：HOJ 数字串 / Hydro 24 位 hex ObjectId，故为不透明字符串引用）；`color` 气球色（如 "#FF0000"，可能为空串，消费方有回退调色板） |

## 直接依赖

无（纯类型声明文件）

## 被依赖

- `bridge/contest.bridge.ts`、`services/contest.service.ts`、`stores/contestStore.ts`、`utils/contest.ts`、`components/problem/ProblemCard.vue`、`components/rank/ScoreboardTable.vue` 等（均仅类型引用）

## 逻辑流程

无（纯类型定义）。

设计要点：

- 时间戳统一为**秒**（与 Rust 实体一致）；前端所有毫秒转换（`* 1000`）都在消费点显式做，
  类型层不混单位——与榜单 `totalTime`（ACM 秒 / OI 毫秒）的双单位陷阱区分开（见 `types/rank.md`）。
- `sealRankTime` 用 `number | null` 而非可选字段：与 Rust `Option<i64>` 的显式 null 序列化对齐。
