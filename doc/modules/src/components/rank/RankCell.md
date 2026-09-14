# RankCell（ACM 榜单单元格）

> 源文件：`src/components/rank/RankCell.vue`

## 职责

ACM 赛制榜单的单题单元格：把 `utils/rank.resolveRankCell` 给出的类别映射为配色与两行文案，附悬浮口径说明；本组件**不实现任何判档规则**。

## 核心类型/函数

**props**：`cell: RankCell | undefined`（undefined = 该题无任何提交记录）。无 emits。

| 名称 | 签名 | 用途 |
|------|------|------|
| `resolved` | computed | `resolveRankCell(props.cell)` → `{ kind, timeText, triesText }` |
| `SKIN` | `Record<RankCellKind, {box, top, bottom}>` | 类别 → 配色，色值一律取 `global.css` 榜单状态色变量（与设计稿 arena.* 对齐）；`after-ac` 复用 `ac` 配色——赛后通过与赛中通过同级展示，差异只由 `*` 前缀与 title 承担（HOJ §8 isAfterContest） |
| `lines` | computed | 两行文案：AC 类 =「通过时间 / 尝试次数」；`wa`/`sealed` 无时间可显示，把尝试次数**提到首行**（`-4` 大字 + 「未通过」/「封榜」小字），否则首行空掉、单元格高度塌陷导致整行错位 |
| `hint` | computed | 悬浮说明：一血/已通过/赛后通过（不计入罚时）/封榜（结果赛后揭晓）/尝试 n 次未通过/暂无提交——赛场上选手需要一眼看懂格子里数字的口径 |

## 直接依赖

- `vue`
- `@/types/rank`（仅类型）
- `@/utils/rank`（`resolveRankCell` + `RankCellKind` 类型）

## 被依赖

- `components/rank/ScoreboardTable.vue` — ACM 分支逐题渲染

## 逻辑流程

```
props.cell → resolveRankCell（纯函数，规则见 utils/rank.md）
  kind = none → 渲染居中 `-`（无底色）
  其余 → SKIN[kind] 底色盒 + lines 两行文案 + hint title
  first-ac 额外渲染闪电内联 SVG 角标
```

设计要点：

- **规则与样式彻底分离**：errorNum+1、封榜优先、`*` 前缀等全部判据在 `utils/rank`
  （有单元测试锁定），组件只做「类别 → 配色 + 文案排布」，改样式不会碰规则、改规则不会碰样式。
- OI 赛制不走本组件（submissionInfo 值是整数得分，判据完全不同），由 ScoreboardTable
  的 OI 分支 + `resolveOiRankCell` 处理。
- 图标内联 SVG（离线客户端不引外部字体/CDN）。
