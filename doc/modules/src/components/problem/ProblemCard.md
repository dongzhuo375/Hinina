# ProblemCard（题目卡片）

> 源文件：`src/components/problem/ProblemCard.vue`

## 职责

题目总览网格中的单题卡片：字母徽章（气球色）、标题、limits 摘要、全场通过数、我的状态 pill（已通过/尝试过/未作答），以及快捷提交（卡片内弹窗）/ 评测记录（携带题目筛选跳评测页）/ 查看题目入口。

## 核心类型/函数

**props**：`problem: ContestProblem`。**emits**：`open: []`（标题与「查看题目」按钮）。

| 名称 | 签名 | 用途 |
|------|------|------|
| `quickSubmitOpen` | ref | 快捷提交弹窗开关；「快捷提交」按钮打开 `QuickSubmitDialog`（弹窗内完成提交与评测跟踪，**不离开题目总览**） |
| `FALLBACK_PALETTE` | 6 色常量 | 徽章回退调色板：HOJ 气球色可能为空串（组织者未配置），按题号序号循环取色——**同一题号恒定同色**，轮询刷新时徽章不跳变 |
| `displayIdOrdinal` | `(displayId) => number` | 字母题号 A/B/C… → 0/1/2…；纯数字题号按数值−1；其余回退 0 |
| `badgeColor` / `title` | computed | 气球色优先，空则回退调色板；标题空则 `Problem {displayId}` |
| `limits` | computed | `problemStore.limitsOf(displayId)`；null = 后端获取失败 |
| `pill` | computed | 我的状态：`statusOf(problemId) === 1` 或榜单 `myRow.submissionInfo[displayId].isAc` **任一为真即判 AC**（两个数据源可能各自缺失）；AC 时优先展示榜单里的 AC 用时（`rankAc && acTime != null` → `AC 00:08`，零额外请求），否则「已通过」；status=2 → 「尝试过」；其余「未作答」 |

## 直接依赖

- `vue` / `vue-router`（评测记录 `router-link` → Submissions 路由）
- 组件：`QuickSubmitDialog`（`v-if` 挂载，保证同一时刻至多一个实例）
- `@/types/contest`（仅类型）
- `@/stores/problemStore`（limits / 我的状态）、`@/stores/rankStore`（myRow）
- `@/utils/limits`（`formatLimitsSummary`）、`@/utils/rank`（`formatRankTime`）

## 被依赖

- `views/ProblemSetView.vue` — 卡片网格（open → 跳解题页；快捷提交弹窗由卡片自持，父级无处理器）

## 逻辑流程

```
props.problem（首屏即有 ac/total）→ 立即渲染徽章/标题/通过数
problemStore.limits 渐进到达 → limits 摘要补齐：
  加载中 → 脉冲骨架；失败（null 且非加载中）→ 定格 `—`，不编造默认值
rankStore.myRow / problemStore.myStatus 到达 → pill 状态跃迁（未作答 → 尝试过 → 已通过/AC 用时）
「快捷提交」→ quickSubmitOpen = true → QuickSubmitDialog（提交 + 评测跟踪在弹窗内闭环）
「评测记录」→ router-link Submissions?problem=displayId（评测页深链自动按本题筛选）
```

设计要点：

- **AC 判定取两源并集**：`get_user_problem_status` 与榜单「我的行」刷新时机不同
  （30s vs 10s 轮询），任一源先到即可点亮，避免「榜单已 AC 但卡片还显示尝试过」；
  AC 文案优先用榜单 `acTime`（设计稿的「AC 00:08」），仅在榜单侧无 AC 信息时退「已通过」。
- limits 缺失显示 `—` 而非假默认值（HOJ-Problem-Limits-API.md §9.5 口径，与
  `utils/limits` 占位符约定一致）。
- **快捷提交弹窗归卡片自持**：原 `quickSubmit` emit（父级跳解题页 + `?focus=1`）已移除，
  弹窗直接挂在卡片内，选手不离开题目总览即可提交并看到评测结果。
- 组件只经 store 取数，不触 service/bridge；图标内联 SVG。
