# RankToolbar（榜单工具条）

> 源文件：`src/components/rank/RankToolbar.vue`

## 职责

榜单页工具条：队伍/学校搜索（300ms 防抖）、分组切换（全场总榜 / 正式参赛队 / 打星队 / 女生队）、OI 计分规则徽章、按赛制切换的单元格图例。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `input` | ref | 搜索框**本地态**，与 `rankStore.keyword`（服务端已生效关键词）解耦，防抖后才提交 |
| `watch(input)` | — | 300ms 防抖 → trim → 与已生效关键词相同则跳过 → `rankStore.setKeyword`（失败静默，原因已由 store 写入 error，RankView 统一展示；全量快照模式下 store 内部携带关键词重拉快照） |
| `GROUPS` | 常量数组 | 四个分组的 value/label（`RankGroupFilter`） |
| `selectGroup(filter)` | fn | `rankStore.setGroupFilter`：official 触发服务端重新请求（removeStar）；star/female 进入 store 的**全量快照模式**（跨页拉取后客户端过滤，见 `rankStore.md`） |
| `oiScoreTypeText` | computed | OI 计分规则徽章文案：`contest.oiRankScoreType`（比赛属性，**服务端只读**，文档 §2.4）`Highest` →「得分规则：最高分」、`Recent` →「得分规则：最近提交」、未知取值原样展示；ACM 比赛或未返回时为 null（不渲染） |
| `ACM_LEGEND` / `OI_LEGEND` / `legend` | computed | 图例；`contest.contestType === 1` → OI 图例（满分/部分分/未得分），否则 ACM 图例（一血/通过/未通过/封榜） |

## 直接依赖

- `vue`
- `@/stores/contestStore`（赛制）、`@/stores/rankStore`（keyword/groupFilter + `RankGroupFilter` 类型）

## 被依赖

- `views/RankView.vue` — 头部工具条

## 逻辑流程

```
键入 → input（本地）→ 300ms 防抖 → setKeyword → 服务端全量重过滤 → 回到第 1 页
点击分组 → setGroupFilter → official 重新请求（removeStar）/ star·female 全量快照模式
           （store 跨页拉取后客户端过滤；快照就位前 visibleRows 退化为筛当前页）
OI 比赛（contestType===1）且返回 oiRankScoreType → 图例左侧渲染计分规则徽章
图例色块直接复用单元格的配色变量（--color-first-ac / ac / wa / frozen…）
```

设计要点：

- **300ms 防抖的必要性**：HOJ 内榜是「全量重算后再分页」（文档 §9.9），逐字符触发请求
  会在赛场上把 OJ 打爆；且服务端 keyword 会重置到第 1 页，抖动过程中反复跳页会让表格闪烁。
- **OI 计分规则徽章只读展示**：`oiRankScoreType` 是比赛属性（服务端只读），客户端无权
  更改，徽章 + title 悬浮解释「最高分 = 取每题最高分；最近提交 = 取最后一次提交的得分」，
  帮助选手理解榜单得分口径；未知取值原样展示不猜测语义。
- **ACM 图例不含「待评测 (Pending)」**：榜单接口的 `submissionInfo` 没有 pending 字段，
  `resolveRankCell` 也不会产出该档位——列一个表格里永远不会出现的色块只会误导选手
  （设计稿里有，实现按数据能力收敛）。
- 图例 = 表格里真实出现的样子：色块直接取单元格同一套 CSS 变量，保证「图例所见即表格所得」。
- 搜索框 title 提示服务端匹配范围（只按学校或榜单显示名，HOJ §9.4），管理选手预期。
- 防抖定时器 onBeforeUnmount 清理；组件只经 store，不触 service/bridge。
