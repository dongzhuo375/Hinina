# ContestStatsBar（比赛统计卡）

> 源文件：`src/components/contest/ContestStatsBar.vue`

## 职责

题目总览与榜单页共用的顶部统计条：解题进度（`x / n AC`）、实时排名（`#x / 人数`）、ACM 总罚时或 OI 总得分，零额外请求。

## 核心类型/函数

`<script setup>` 组合式 API，无对外 props/emits：

| 名称 | 类型 | 用途 |
|------|------|------|
| `isAcm` | `computed<boolean>` | `contest.contest?.contestType !== 1`（OI=1，其余按 ACM 处理） |
| `solvedText` | `computed<{value, unit}>` | 解题进度：`myRow.ac`，unit 为 `/ ${题目数} AC`（题目数缺失时只显示 `AC`） |
| `rankText` | `computed<{value, unit}>` | 实时排名：`#${myRow.rank}`，unit 为 `/ ${participants}`（participants 为 0 时不显示）；打星（`rank === -1`）或缺失显示 `—` |
| `penaltyText` | `computed<{label, value}>` | ACM：`总罚时` = `formatPenaltyMinutes(totalTime)` + `m`；OI：`总得分` = `totalScore` |

## 直接依赖

- `vue`（`computed`）
- `@/stores/contestStore`（赛制类型、题目列表）
- `@/stores/rankStore`（`myRow` / `participants` / `lastUpdated`）
- `@/utils/rank`（`formatPenaltyMinutes`）

## 被依赖

- `views/RankView.vue` — 榜单页顶部统计条
- `views/ProblemSetView.vue` — 题目总览顶部统计条（两视图共用同一组件与同一份 rankStore 数据）

## 逻辑流程

```
rankStore（轮询刷新） ──myRow/participants/lastUpdated──▶ 统计卡三段 computed 渲染
contestStore ──contestType/problems.length──▶ 赛制分支与分母

数据源是榜单中「我的行」：HOJ 会把当前用户前置复制到 records（§9.2），
因此任意页都能取到 myRow —— 统计卡不需要为「我的名次」单发请求。

无数据时的占位：
  myRow 未加载 / ac 缺失        → `—`（不是 0）
  rank 缺失或 -1（打星不排名）  → `—`
  OI totalScore === null        → `—`
根节点 title：lastUpdated 有值 → 「榜单更新于 hh:mm:ss」，否则「榜单数据尚未加载」
```

设计要点：

- **无数据显示 `—` 而不是 0**：0 会被误读成「一题未解 / 排名第 0」这类真实且伤人的状态，
  占位符才明确表达「数据尚未取得」；榜单加载前与加载失败时统计卡都保持占位。
- **OI 赛制显示总得分而非罚时**：OI 的 `totalTime` 是毫秒且语义为耗时，罚时口径不适用
  （见 `utils/rank` 的 `formatPenaltyMinutes` 注释），故第三段整体切换为 `总得分`。
- 纯展示组件：所有状态来自 store，自身不发请求、不持有轮询——刷新节奏完全由
  `rankStore.startLive` 编排。
