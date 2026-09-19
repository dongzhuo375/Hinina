# submission（评测状态判据、文案映射与展示格式化）

> 源文件：`src/utils/submission.ts`

## 职责

提供"评测是否结束"的单一判据（决定轮询何时停止）、评测状态的文案/缩写/语义色调唯一映射表（驱动评测页、详情页、最新记录 pill 的状态渲染），以及提交数据的展示格式化纯函数（时间/内存/代码长度/语言映射，评测页 / 提交详情页 / 最新记录 pill / 快捷提交弹窗共用）。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `isTerminalStatus` | `(status: JudgementStatus) => boolean` | 状态是否为终态（终态即停止轮询） |
| `isJudging` | `(status: JudgementStatus) => boolean` | 是否仍在评测中（= 非终态，评测页温和刷新的判据） |
| `findFirstFailedCase` | `(result: SubmissionCases) => JudgeCase \| null` | 首个非 Accepted 测试点（「Test N」失败提示数据源）：先查平铺 `cases`；子任务制下 `cases` 常为空，此时按 groupNum、组内按 seq 展开 `subTasks` 查找（L1：避免提示静默消失） |
| `StatusTone` | `'ac' \| 'wa' \| 'tle' \| 'pending' \| 'system' \| 'neutral'` | 状态语义色调（组件层映射到具体样式类） |
| `statusLabel` | `(status) => string` | 状态原词（HOJ 文案，架构约束「状态文案以接口返回为准」） |
| `statusAbbr` | `(status) => string` | 缩写（AC/WA/TLE…，紧凑 pill 用） |
| `statusTone` | `(status) => StatusTone` | 语义色调 |
| `STATUS_OPTIONS` | `{ value: number; label: string }[]` | 评测页状态筛选下拉（value = **HOJ 原始状态码**，与后端 `status` 查询参数对齐；只列高频状态）。11 项：`0` AC / `-1` WA / `1` TLE / `2` MLE / `3` RE / `-2` CE / `-3` PE / `8` PA / `5` Pending / `6` Compiling / `7` Judging |
| `STATUS_META` / `NON_TERMINAL_STATUSES` | 模块内私有 | 变体 → 文案/缩写/色调 的唯一映射表（含 `NotSubmitted` `NS/neutral`、`Cancelled` `CANC/系统色`）；非终态集合 `Pending`/`Compiling`/`Running` —— HOJ 的 4 个非终态码收敛为 3 个变体（5 Pending 与 9 Submitting → `Pending`、6 → `Compiling`、7 Judging → `Running`） |

**展示格式化纯函数**（评测页 / 提交详情页 / 最新记录 pill 共用）：

| 名称 | 签名 | 用途 |
|------|------|------|
| `formatClock` | `(epochSecs: number) => string` | epoch 秒 → **本地时区** `HH:MM:SS`（提交时间列），补零两位 |
| `formatDurationHms` | `(totalSecs: number) => string` | 秒数时长 → `HH:MM:SS`（赛时相对时间）；负值（赛前提交）/非有限值钳制 `--:--:--`，不显示负号时长 |
| `formatMemoryKb` | `(kb: number) => string` | 内存：<1024 显示 `N KB`（取整），≥1024 换算 `X.X MB`（1 位小数）；非正值（评测中/未回填）显示 `-` |
| `formatCodeLength` | `(bytes: number) => string` | 代码长度（字节）→ `X.X KB`（1 位小数）；非法/负值 `-` |
| `formatMsToSeconds` | `(ms: number) => string` | 毫秒 → 秒保留两位小数（最新记录 pill 失败测试点的 `2.01s` 口径）；非法/负值 `-` |

（语言 → Monaco id 的映射已迁至 `utils/language.monacoIdOf` —— 语言域唯一权威模块。）

## 直接依赖

- `@/types/submission`（仅类型）

## 被依赖

- `stores/submissionStore.ts` — `pollOnce()` 终态判定
- `views/SubmissionsView.vue` — 状态 pill 渲染、筛选下拉（`STATUS_OPTIONS`）与时间/内存/代码长度格式化
- `views/SubmissionDetailView.vue` — 状态渲染与格式化（只读代码视图的语言映射走 `utils/language.monacoIdOf`）
- `components/editor/EditorConsoleBar.vue` — 最新记录 pill（`statusAbbr` / `statusTone` / `formatMsToSeconds` / `isTerminalStatus` / `findFirstFailedCase`）
- `components/problem/QuickSubmitDialog.vue` — 弹窗内评测状态行（`statusLabel` / `statusTone` / `formatMemoryKb` / `isTerminalStatus`）
- `utils/__tests__/submission.spec.ts` — 单元测试

## 逻辑流程

```
status ∈ {Pending, Compiling, Running} → 非终态，继续轮询
其余（含全部扩展状态与 Unknown）        → 终态，停止轮询
```

设计要点：

- 判据与 Rust `adapter::hoj::types::is_terminal_status`（非终态 = HOJ 码 **5 Pending / 6 Compiling / 7 Judging / 9 Submitting**）严格对齐；这 4 个码经 `map_status` 收敛为 3 个变体（5 与 9 → `Pending`、6 → `Compiling`、7 → `Running`）。
- **`STATUS_OPTIONS` 的码值必须是 HOJ 真实码（含负数）**：它有两个消费方 —— 后端 HOJ 查询参数与 `adapter::hydro::hoj_status_to_hydro` 的码表翻译，映射错了不仅筛错 HOJ，还会翻成另一个 Hydro 状态。历史上整表按「0 起顺排」写错，后果是选手选「Accepted」实际筛的是 HOJ 的 **Pending（码 5）**；且 `-10 Not Submitted` / `8 PA` 在 Hydro 码表中不存在，后端据此**显式报错而非静默忽略筛选**（静默忽略会让选手以为「筛出来的就是全部」）。
- `JudgementStatus` 已扩展覆盖 HOJ 全部状态码（20 个变体；PE/OLE/SE/RJE/SF/PA/FREQ/UE 拥有独立变体，本轮又补上 `NotSubmitted`(-10) / `Cancelled`(-4)），不再折叠进 Unknown；`Unknown` 仍必须视为终态，防止无法识别的状态码被无限轮询。
- 新增状态只改 `STATUS_META` 一处；未收录变体兜底为变体名本身/`neutral`，避免两端升级不同步时前端崩溃。
- 格式化函数对非法值（NaN/Infinity/负数/未回填的 0）一律返回占位符（`-` / `--:--:--`）而非抛错或显示假 0——评测中的行没有耗时/内存可言。
- 纯函数、无副作用，便于单元测试与跨层复用。

## 测试

`src/utils/__tests__/submission.spec.ts` 锁定：`Unknown` 必须视为终态、终态清单与 `JudgementStatus` 全量取值（20 个）一一对应（借 `Record<JudgementStatus, boolean>` 穷尽映射，新增枚举值时直接类型报错，逼迫显式归类）；格式化纯函数——`formatClock` 本地时区与补零、`formatDurationHms` 负值/非有限值钳制、`formatMemoryKb` KB/MB 分界与非正值 `-`、`formatCodeLength` 字节换算、`formatMsToSeconds` 两位小数；`findFirstFailedCase` 平铺优先/全 AC 返回 null/子任务制按 groupNum+seq 展开/双空返回 null。（语言映射测试见 `utils/__tests__/language.spec.ts`。）
