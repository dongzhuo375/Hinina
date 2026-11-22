# rank（榜单渲染纯映射）

> 源文件：`src/utils/rank.ts`

## 职责

把服务端返回的原始榜单聚合（errorNum/tryNum/isAC/ACTime…）映射为渲染所需的类别、文案与统计值。规则严格照 `doc/HOJ/HOJ-Contest-Rank-API.md` §8「客户端需要自行补充计算」与 §9「客户端实现注意事项」——这些推导 HOJ 后端不做，集中在此处避免散落在多个 View 里各自实现产生分歧。纯函数、无副作用。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `RankCellKind` | `'first-ac' \| 'ac' \| 'after-ac' \| 'sealed' \| 'wa' \| 'none'` | 单元格样式类别（对应 HOJ 前端 cellClassName 语义） |
| `ResolvedRankCell` | `{ kind; timeText: string \| null; triesText: string \| null }` | 类别 + 两行文案（null = 该行不渲染） |
| `resolveRankCell` | `(cell: RankCell \| undefined) => ResolvedRankCell` | 单元格显示规则（HOJ §8），见下 |
| `formatRankTime` | `(seconds: number) => string` | 比赛进度秒数 → `mm:ss` / `h:mm:ss`；非法值 → `--:--` |
| `resolveDisplayName` | `(row, rankShowName: string) => string` | 按比赛配置取显示名，空值回退 `username` → `uid` |
| `RankRowKind` | `'star' \| 'female' \| 'normal'` | 行样式类别 |
| `resolveRowKind` | `(row) => RankRowKind` | `rank === -1` 打星（优先级最高）> `gender === 'female'` > 普通 |
| `dedupeRankRows` | `(rows) => ContestRankRow[]` | 按 `uid` 去重，保留首次出现的行 |
| `stripPrependedSelfRow` | `(rows, myUid: string \| null, current: number, size: number) => ContestRankRow[]` | 剔除服务端前置的「当前用户」副本，保留自然位置行（正文恢复纯名次序），见下 |
| `resolveMyRow` | `(rows, uid: string \| null) => ContestRankRow \| null` | 定位「我的行」（置顶高亮用） |
| `resolveParticipantCount` | `(rows, total: number) => number` | 去重后非打星行的最大 `rank`；取不到才回退 `total` |
| `formatPenaltyMinutes` | `(totalTimeSeconds: number) => string` | ACM totalTime 秒（含罚时，UI 标签「总用时」）→ 分钟整数（向下取整）；非法 → `--` |
| `resolveParticipantCountFromPage` | `(page: ContestRankPage, myUid: string \| null) => number` | 从整页响应推算真实参与人数（统计卡分母），含「我的前置副本」修正，见下 |
| `RankGroupFilter` | `'all' \| 'official' \| 'star' \| 'female'` | 榜单分组筛选：official 走服务端 `removeStar`；star/female 服务端无对应参数，只筛当前页会跨页漏行，须全量快照后客户端过滤（`rankStore.fetchAllRows`） |
| `mergeRankPages` | `(existing, incoming) => ContestRankRow[]` | 跨页合并快照：当前用户前置副本已由 `stripPrependedSelfRow` 在合并前剔除（`fetchAllRows` 逐页处理），此处处理关注用户等其余跨页同 uid 重复行——已存在的行保留（首次出现优先），新行按序追加；不改入参 |
| `filterRankRowsByGroup` | `(rows, filter) => ContestRankRow[]` | 分组过滤纯函数（全量快照与当前页共用同一判据）：`star` → `rank === -1`；`female` → `gender === 'female'`；其余原样返回 |
| `paginateRankRows` | `(rows, current, size) => ContestRankRow[]` | 客户端分页切片：`current` 钳到 ≥1，越界页返回空数组，`size <= 0` 视为不分页原样返回 |
| `OiCellKind` | `'full' \| 'partial' \| 'zero' \| 'none'` | OI 档位（对应 HOJ §8 的 `oi-100` / `oi-between` / `oi-0`） |
| `ResolvedOiCell` | `{ kind; scoreText; timeText: string \| null; hint }` | OI 单元格：得分文本 + 最优耗时文本 + 悬浮提示 |
| `resolveOiRankCell` | `(cell: RankCell \| undefined, timeInfoMs: number \| undefined) => ResolvedOiCell` | OI 赛制单元格映射，见下 |

## 直接依赖

- `@/types/rank`（仅类型）

## 被依赖

- `stores/rankStore.ts` — `applyPage()` 用 `stripPrependedSelfRow` / `dedupeRankRows` / `resolveMyRow` / `resolveParticipantCountFromPage` 做入页归一；全量快照模式用 `stripPrependedSelfRow`（逐页）+ `mergeRankPages` / `filterRankRowsByGroup` / `paginateRankRows`（`RankGroupFilter` 经 store re-export 给 RankToolbar）
- `components/rank/ScoreboardTable.vue` — `resolveDisplayName` / `resolveRowKind` / `resolveOiRankCell`（OI 榜单）/ `formatPenaltyMinutes` / `formatRankTime`
- `components/rank/RankCell.vue` — `resolveRankCell`（ACM 单元格文案与样式）
- `components/contest/ContestStatsBar.vue` — `formatPenaltyMinutes`
- `components/problem/ProblemCard.vue` — `formatRankTime`
- `utils/__tests__/rank.spec.ts` — 单元测试

## 逻辑流程

**resolveRankCell（HOJ §8 单元格规则）**：

```
cell 为 undefined（该题无记录）        → none（UI 显示空格子/-）
isAc:
  kind = isAfterContest ? after-ac : isFirstAc ? first-ac : ac
  timeText = formatRankTime(acTime ?? NaN)      // acTime 缺失按非法值渲染 '--:--' 而非崩溃
  isAfterContest 时 timeText 前加 `*`            // 赛后提交通过
  triesText = `${errorNum + 1} 试`               // 坑：errorNum 不含本次 AC，显示尝试数要 +1
tryNum != null                                   // 封榜行，优先于 wa 判定：
  → sealed，triesText = `${errorNum}+${tryNum} tries`   // 封榜期间服务端只累加 tryNum、不写 isAC，
                                                 // 且 errorNum 可能同时 >0（封榜前的失败）
errorNum > 0 → wa，triesText = `-${errorNum}`     // ICPC 习惯显示 -罚时次数
其余 → none
```

**前置副本剔除与参与人数为什么两个口径都不能直接用**（HOJ §9.2）：

- 服务端把「当前登录用户」与「关注列表用户」复制一份插到 records 最前面（**每一页**都前置），
  `total` 含这些前置副本，比真实人数偏大；
- 副本不去除会以普通行身份占据正文首行——`dedupeRankRows` 保留首次出现恰好把它保下来并丢掉
  自然位置行，名次列会从「我的名次」开始而不是 1。`stripPrependedSelfRow(rows, myUid, current, size)`
  的剔除规则：myUid 出现 ≥2 次 → 首条必为前置副本，剔除首条（自然位置行保留）；恰好 1 次 →
  位于 `records[0]` 且满足两级判据之一：满页 `length > size` **结构判据**（HOJ 把副本插在
  `limit` 行之外，满页长度即结构证据——同时兜住打星行占位/并列名次导致的 rank 与位置错位，
  此时副本 rank 可能恰好落进本页窗口、仅看窗口会漏剔）；短页（末页）回退**窗口判据**——
  有名次行（rank ≥ 1）rank 不落在本页名次窗口 `[(current−1)×size+1, current×size]` 内。
  **打星行 rank === -1 不构成副本证据**（无自然位置既可解释为 HOJ 前置副本、也可解释为
  无前置服务端如 Hydro 的榜首打星自然行），仅结构判据可剔——HOJ 星队的短页前置副本为
  已知残余。关键词过滤重排后的短页窗口可能对不上，同样是已知残余；uid 为空/未出现
  原样返回。
  **必须在 `dedupeRankRows` 之前调用**：去重会把自然位置行丢掉，届时无法再区分副本与自然行；
  `myRow` 定位（`resolveMyRow`）也用剔除前的原始 records——前置副本正是它每页都存在的意义；
- 不能用「本页最大 rank」：第 1 页 limit=50 时最大 rank 只有 ~50，不是全场人数；
- `resolveParticipantCountFromPage(page, myUid)` 的口径：
  `total − 本页重复行数 − 未被去重捕获的「我的前置副本」`。
  前置副本与其自然位置**同页**出现时 uid 重复，被去重计数捕获；「我」的自然名次
  **不在本页**时，本页只有前置副本这一条（uid 仅出现一次，不构成重复），须额外减 1。
  判据与 `stripPrependedSelfRow` 两级共用：myUid 恰好出现一次、位于 `records[0]`，
  满页 `length > size` 结构判据（兜住打星占位/并列名次的 rank-位置错位），短页回退
  「rank 不落在本页名次窗口内」——仅 rank ≥ 1 的行适用，打星行 rank 本身不构成
  副本证据。
  残余误差：**关注用户**的窗外前置副本与 **HOJ 星队的短页前置副本**无法用同一判据
  识别（口径只修「我」与有名次行），分母仍可能偏大——对展示无实质影响，不为此额外
  请求末页。
  `total ≤ 0` 时才退回 `resolveParticipantCount` 的最大 rank 口径
  （打星 `rank === -1` 不参与排名，须剔除）。

**全量快照模式纯函数**（打星/女生队跨页过滤，编排在 `rankStore.fetchAllRows`）：

```
mergeRankPages：服务端在**每一页**都前置复制当前用户/关注用户（§9.2），
  逐页拉取合并前先经 stripPrependedSelfRow 剔除（否则副本占住快照头部，
  按位置切片的名次窗口整体错位），页与页之间的同 uid 重复行再去重；
  首次出现优先（与 dedupeRankRows 一致），不改入参
filterRankRowsByGroup：star/female 判据与 resolveRowKind 的行样式判定同源；
  all/official 原样返回（official 的过滤在服务端 removeStar 完成）
paginateRankRows：快照过滤后的客户端切片，供 rankStore.visibleRows 分页
```

**resolveOiRankCell（OI 赛制单元格）**：

```
不能复用 resolveRankCell：OI 的 submissionInfo 值是整数得分（Adapter 归一到
RankCell.score，其余字段取默认值），走 ACM 判据会全部落到 none，整张 OI 榜单渲染成一片 `-`

score = cell?.score ?? null；timeText = timeInfoMs > 0 时 formatRankTime(ms/1000 取整)
（timeInfo 单位是**毫秒**，与 ACM acTime 的秒不同）

score 为 null（无提交）→ none，scoreText '-'，timeText 强制 null，hint「暂无提交」
score >= 100 → full（满分）；0 < score < 100 → partial（部分分）；其余 → zero（未得分）
```

设计要点：

- `formatRankTime` 的入参是**相对开赛时刻的进度秒数**而非墙钟时间，故按时长格式渲染
  （8 秒 → `00:08`，3720 秒 → `1:02:00`）。
- `formatPenaltyMinutes` 只适用 ACM（totalTime 秒）；OI 的 totalTime 是毫秒且语义为耗时，
  罚时口径不适用（见 `ContestStatsBar` 的赛制分支）。
- `resolveDisplayName`：HOJ 允许 realname/nickname 为空，未知 `rankShowName` 取值按
  `username` 处理，保证单元格永远有可辨识文本。
- 所有非法值（NaN/负数/缺失）都渲染占位符而不是抛错——赛场上的服务端脏数据不应让 UI 崩溃。
- **规则与样式分离**：判档/文案全部在本模块（纯函数、有测试锁定），组件只做呈现——
  ACM 侧 `RankCell.vue` 按 `RankCellKind` 映射配色，OI 侧 `ScoreboardTable.vue` 按
  `OiCellKind` 映射配色（档位 → CSS），改样式不碰规则。

## 测试

`src/utils/__tests__/rank.spec.ts`（73 例）锁定：ACM 单元格规则（AC 行 errorNum+1、after-ac 加 `*` 且优先于 first-ac、封榜行优先于 wa、acTime 脏数据渲染 `--:--`）、时间/罚时格式化与非法值占位、显示名三级回退、打星行判定优先级、uid 去重不改入参、前置副本剔除（同页出现剔首条留自然行、窗外唯一出现剔除、打星占位 rank-位置错位的满页结构判据剔除、短页窗口回退剔除、短页窗内首条视为自然行保留、非首条保留、打星我、uid 空/未出现、不改入参）、参与人数口径（total−重复行、我的窗外首条前置副本额外减 1、打星 myRow 短页单条不减（rank 不构成证据，HOJ 星队短页副本为残余）、满页打星副本结构判据减 1、窗口边界、短页恰在窗口内不减、非首条不减、rank-位置错位满页结构判据减 1、多关注用户副本的捕获与已知残余误差、不退化为本页最大 rank、total 缺失退回最大 rank）、全量快照纯函数（mergeRankPages 跨页去重/首次出现优先/不改入参、filterRankRowsByGroup 三档判据、paginateRankRows 切片/钳位/越界/不分页边界）、OI 档位与 timeInfo 毫秒换算。
