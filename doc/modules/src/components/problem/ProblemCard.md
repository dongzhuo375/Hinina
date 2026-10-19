# ProblemCard（题目行）

> 源文件：`src/components/problem/ProblemCard.vue`

## 职责

题目总览中的**单行式题目行**（PR #36 由卡片网格改为紧凑目录行）：题号 + 气球色线、标题与 limits 摘要、通过/提交比、我的状态、以及「评测记录 / 快捷提交 / 查看题目」三个动作，并自持快捷提交弹窗。

## 核心类型/函数

**props**：`problem: ContestProblem`。**emits**：`open: []`（题号按钮、标题按钮、「查看题目」按钮）。

| 名称 | 签名 | 用途 |
|------|------|------|
| `quickSubmitOpen` | `ref<boolean>` | 快捷提交弹窗开关；「快捷提交」按钮打开 `QuickSubmitDialog`（弹窗内完成提交与评测跟踪，**不离开题目总览**） |
| `FALLBACK_PALETTE` | 6 色常量 | 徽章回退调色板（低饱和大地色系，与赛场主题一致）：HOJ 气球色可能为空串（组织者未配置），按题号序号循环取色——**同一题号恒定同色**，轮询刷新时徽章不跳变 |
| `displayIdOrdinal` | `(displayId) => number` | 字母题号 A/B/C… → 0/1/2…；纯数字题号按数值−1；其余回退 0 |
| `badgeColor` / `title` | computed | 气球色优先，空则回退调色板；标题空则 `Problem {displayId}` |
| `limits` | computed | `problemStore.limitsOf(displayId)`；null = 后端获取失败 |
| `pill` | computed | 我的状态：`statusOf(problemId) === 1` 或榜单 `myRow.submissionInfo[displayId].isAc` **任一为真即判 AC**（两个数据源可能各自缺失）；AC 时优先展示榜单里的 AC 用时（`rankAc && acTime != null` → `AC 00:08`，零额外请求），否则「已通过」；`status === 2` → 「未通过」；其余「未作答」 |

## 直接依赖

- `vue` / `vue-router`（评测记录 `router-link` → Submissions 路由）
- 组件：`QuickSubmitDialog`（`v-if` 挂载，保证同一时刻至多一个实例）
- `@/types/contest`（仅类型）
- `@/stores/problemStore`（limits / 我的状态）、`@/stores/rankStore`（myRow）
- `@/utils/limits`（`formatLimitsSummary`）、`@/utils/rank`（`formatRankTime`）

## 被依赖

- `views/ProblemSetView.vue` — 题目目录逐行渲染（open → 跳解题页；快捷提交弹窗由本组件自持，父级无处理器）

## 逻辑流程

```
props.problem（首屏即有 ac/total）→ 立即渲染题号/标题/通过比
problemStore.limits 渐进到达 → limits 摘要补齐：
  加载中 → 「正在读取限制…」；失败（null 且非加载中）→ 定格「限制 —」，不编造默认值
rankStore.myRow / problemStore.myStatus 到达 → 状态跃迁（未作答 → 未通过 → 已通过/AC 用时）
「快捷提交」→ quickSubmitOpen = true → QuickSubmitDialog（提交 + 评测跟踪在弹窗内闭环）
「评测记录」→ router-link Submissions?problem=displayId（评测页深链自动按本题筛选）
```

模板结构：`<article class="problem-row">` 五列 grid —— `problem-ordinal`（大号等宽题号按钮 + 底部 2px `balloon-line` 气球色线）、`problem-description`（标题按钮 + limits）、`problem-ratio`（`ac / total` + `eyebrow`「通过 / 提交」）、`problem-state`（符号 + 文案：AC `✓` / 未通过 `·` / 未作答 `—`）、`problem-actions`（`.action-quiet` ×2 + `problem-open`）；≤1100px 收紧列宽与间距，≤800px 动作与状态换行到第二行。

设计要点：

- **AC 判定取两源并集**：`get_user_problem_status` 与榜单「我的行」刷新时机不同
  （30s vs 10s 轮询），任一源先到即可点亮，避免「榜单已 AC 但行内还显示未通过」；
  AC 文案优先用榜单 `acTime`（设计稿的「AC 00:08」），仅在榜单侧无 AC 信息时退「已通过」。
- limits 缺失显示 `—` 而非假默认值（HOJ-Problem-Limits-API.md §9.5 口径，与
  `utils/limits` 占位符约定一致）。
- **卡片改行的动机**：题目数可能上百，卡片网格一屏只能看 6–9 题，赛前扫题成本高；
  行式目录把密度提到一屏 12+ 题，且题号/气球色/通过比在同一列上可比对。
- **气球色回退调色板随主题更换**：原高饱和色（`#e11d48`/`#10b981`/`#2563eb`…）
  与新的中性赛场主题冲突，换成低饱和大地色系；回退策略（按题号序号循环、恒定同色）不变。
- **快捷提交弹窗归本组件自持**：原 `quickSubmit` emit（父级跳解题页 + `?focus=1`）已移除，
  弹窗直接挂在行内，选手不离开题目总览即可提交并看到评测结果。
- 组件只经 store 取数，不触 service/bridge；图标/符号内联（不引图标字体）。
