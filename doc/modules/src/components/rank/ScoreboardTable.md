# ScoreboardTable（榜单表格）

> 源文件：`src/components/rank/ScoreboardTable.vue`

## 职责

榜单主表格：粘性表头 + 粘性「我的行」+ 粘性前两列（排名/选手），ACM 与 OI 两套单元格分流渲染，题目列头带气球色徽章与全场通过数。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `isAcm` | computed | `contest.contestType !== 1`；**contest 未加载时按 ACM 处理**（Hinina 面向 ICPC 场景） |
| `rankShowName` / `problems` | computed | 显示名规则（回退 `username`）与题目列头来源 |
| `headRef` / `headHeight` / `headObserver` | ref/ResizeObserver | **表头实测高度**：粘性「我的行」要贴在表头正下方（`top: headHeight px`），写死像素会随字体/缩放/列头换行漂移，故用 ResizeObserver 持续实测（初值 45，卸载时 disconnect） |
| `BodyRow` | interface `{ row, isMe }` | 正文行 = 粘性「我的行」（unshift 到最前）+ `rankStore.visibleRows` |
| `rowTint` / `stickyTint` | fn | 行背景：打星/女生队按 §8 `userCellClassName` 着色；粘性单元格另补**不透明**实底（见设计要点） |
| `MY_ROW_EDGE` / `COLUMN_EDGE` / `MY_ROW_COLUMN_EDGE` | 常量 | 「我的行」紫色下划线与选手列右竖线，全部用**单元格内阴影**实现 |
| `rankLabel` / `isPodium` | fn | `rank === -1` 打星显示 `*`（ICPC 习惯）；我的行加 `#` 前缀；前 3 名琥珀色圆形徽章 |
| `solvedText` / `totalTimeText` | fn | 解题列：ACM=AC 题数，OI=总得分（null → `—`）；总用时列见下 |
| `BALLOON_FALLBACK` / `balloonColor` | fn | 题目未配色（color 空串）时按列序号循环取 10 色回退，保证列头永远可辨 |
| `OI_BOX` / `oiCellView` / `oiCellsByUid` / `oiCells` | — | OI 档位 → 配色映射 + 按行预计算的单元格视图（见设计要点） |

## 直接依赖

- `vue`
- `@/components/rank/RankCell.vue`（ACM 单元格）
- `@/stores/contestStore`（赛制/rankShowName/题目列头）、`@/stores/rankStore`（visibleRows/myRow）
- `@/types/rank`（仅类型）
- `@/utils/rank`（`formatPenaltyMinutes` / `formatRankTime` / `resolveDisplayName` / `resolveOiRankCell` / `resolveRowKind` + `OiCellKind` 类型）

## 被依赖

- `views/RankView.vue` — 表格主体（分页/刷新状态底栏在 RankView）

## 逻辑流程

```
rankStore.visibleRows（已去重/过滤）+ myRow → bodyRows（我的行置顶 isMe=true）
ACM 分支：每题 <RankCell :cell="row.submissionInfo[displayId]" />（判档全在 utils/rank）
OI 分支：oiCellsByUid 按行预计算 resolveOiRankCell(cell, timeInfo[displayId])
         → 档位 → OI_BOX 配色 + 得分/耗时两行文案 + hint

总用时列（两套 VO 单位不同，混用会把 OI 用时放大 1000 倍）：
  ACM：totalTime = 总罚时（秒）→ formatPenaltyMinutes 换算分钟整数
  OI ：totalTime = AC 提交耗时之和（毫秒）→ ÷1000 后 formatRankTime；
       为 0（无 AC）显示 `—` 而不是 `00:00`（0 会被误读成「用时极短」）
```

设计要点：

- **粘性单元格必须自带不透明背景**：`position: sticky` 的 td 被单独提升绘制，画在 tr 上的
  行背景不跟着横向移动，右侧题目格滚动时会从底下透出来；行 hover 同理（tr 的 hover 背景
  作用不到被提升的 td），需用 `group-hover` 在单元格上补等价实底色。
- **「我的行」下划线用内阴影而非 tr border**：`border-collapse: collapse` 下边框属于合并
  边框模型，Chromium 不让它跟随 sticky 行绘制，滚动时分隔线会消失；`shadow-[inset_0_-2px_0_…]`
  画在单元格自身上始终可见。非粘性单元格也要补同款内阴影，否则紫线只画出前两列。
- **粘性「我的行」不重复**：它在正文的自然位置照常渲染（rows 已按 uid 去重），与
  Codeforces 等榜单的浮标行一致；HOJ 前置副本保证任意页都拿得到 myRow（§9.2）。
- **OI 单元格不复用 RankCell.vue**：OI 的 submissionInfo 值是整数得分，走 ACM 判据会全部
  落到 none、整张榜单一片 `-`；判档与文本由 `utils/rank.resolveOiRankCell`（纯函数、有测试）
  给出，**组件只做档位 → CSS 的样式映射**。
- `oiCellsByUid` 按行预计算：模板里逐格调函数会在每次重渲染时重复构造对象（每行 N 题），
  computed 缓存后只在数据变化时重算。
- 表头高度 ResizeObserver 实测（见上）；空榜单显示「当前筛选条件下没有榜单记录」整行占位。
