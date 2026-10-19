# Countdown（登录页倒计时）

> 源文件：`src/components/login/Countdown.vue`

## 职责

登录页右侧氛围区的赛程时钟：按比赛阶段渲染「距离比赛开始」四段倒计时、赛内已用时或「比赛已结束」大字，并给出阶段化的引导文案与无障碍定时器标签。

## 核心类型/函数

**props**（无 emits）：

| 名称 | 类型 | 用途 |
|------|------|------|
| `phase` | `ContestPhase` | 比赛阶段（由 `utils/contest.getContestPhase` 推导，父级传入） |
| `startTime` / `endTime` | `number \| null` | 赛程起止（epoch 秒），缺失即无赛程信息 |
| `now` | `number` | 当前时间（epoch 秒），由父级每秒刷新驱动 |
| `isLoggedIn` | `boolean` | 引导文案分叉（已登录/未登录话术不同） |

| 名称 | 签名 | 用途 |
|------|------|------|
| `clock` | computed | `getContestClock(phase, startTime, endTime, now)` → 时钟模型 |
| `segments` | computed | `getClockSegments(clock.seconds)` → 天/时/分/秒四段 |
| `heading` | computed | `elapsed`→「比赛进行中」；`ended`→「比赛已结束」；否则「距离比赛开始」（作为 `sr-only` 标题） |
| `guidance` | computed | 阶段 × 登录态引导文案：`upcoming` 已登录「已登录 · 开赛后自动进入」/ 未登录「距离比赛开始还有」；`running` 已登录「正在进入赛场」/ 未登录「比赛进行中 · 登录后即可进入」；`ended`「本场比赛已结束」；其余「赛程信息连接后显示」 |
| `timerLabel` | computed | `phase === 'none'` 时回退为 `guidance`；否则「{heading}：{value}{label} …」拼串，供读屏播报 |

## 直接依赖

- `vue`（`computed`）
- `@/utils/contest`（仅 `ContestPhase` 类型）
- `@/utils/contest-clock`（`getContestClock` / `getClockSegments`）

## 被依赖

- `views/LoginView.vue` — 右侧氛围区时钟（`:phase` / `:start-time` / `:end-time` / `:now` / `:is-logged-in`）

## 逻辑流程

```
phase + startTime/endTime + now → clock（种类 / 秒数 / urgent）
  kind === 'ended' → 渲染大字「比赛已结束」（不渲染 role=timer）
  否则             → 4 列网格渲染四段 + 段间 `/` 分隔符
                     数字色 clock.urgent ? 朱红 #a33b2a : 深灰 #2e3033
                     数字字体取 --login-font-mono + tabular-nums slashed-zero
  末行恒渲染 guidance
```

设计要点：

- **判据全部来自 `utils/contest-clock` 纯函数**：组件不自行算秒数、不自行判紧急，阈值调整只改一处（有单测锁定）。
- **`ended` 与「无赛程」界面表现分立**：前者大字明示结束，后者渲染 `--` 占位段 + 「赛程信息连接后显示」，避免把「信息未到」误报成「已结束」。
- **无障碍**：容器 `role="timer"` + `aria-live="off"`（每秒跳动的数字不打扰读屏）+ `aria-label=timerLabel`；视觉数字与标签均 `aria-hidden`，读屏只播报一次拼串标签；`phase === 'none'` 时标签退化为引导文案（有单测锁定）。
- **`prefers-reduced-motion` 下关闭变色过渡**：告警色变化不做动画。
- 段值定宽两位 + `tabular-nums`：逐秒变化时布局不抖动。

## 测试

`src/components/login/__tests__/Countdown.spec.ts` 锁定：赛程缺失（`phase='none'`）时 `[role="timer"]` 的 `aria-label` 回退为引导文案「赛程信息连接后显示」。
