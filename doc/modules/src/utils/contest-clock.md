# contest-clock（登录页时钟模型）

> 源文件：`src/utils/contest-clock.ts`

## 职责

把「比赛阶段 + 赛程时间 + 当前时间」抽象为纯函数时钟模型（种类 / 剩余秒数 / 是否紧急），并生成天·时·分·秒四段展示值；与比赛外壳共用同一时间契约，避免各消费方各写一套取整与封顶逻辑。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `ContestClock` | `{ kind: 'countdown' \| 'elapsed' \| 'ended' \| 'unavailable'; seconds: number \| null; urgent: boolean }` | 时钟模型：`countdown` = 距开赛倒计时、`elapsed` = 赛内已用时、`ended` = 已结束、`unavailable` = 无赛程信息 |
| `ClockSegment` | `{ key: 'days' \| 'hours' \| 'minutes' \| 'seconds'; label: string; value: string }` | 单段展示值（label = 天/时/分/秒，value = 定宽两位字符串） |
| `getContestClock` | `(phase: ContestPhase, startTime: number \| null, endTime: number \| null, now: number) => ContestClock` | 由阶段与时间推导时钟模型 |
| `getClockSegments` | `(seconds: number \| null) => ClockSegment[]` | 秒数 → 四段展示值；`null` 时全部为 `--` |

模块级常量：`DAY_SECONDS = 24 * 60 * 60`、`URGENT_WITHIN_SECONDS = 15`。

## 直接依赖

- `@/utils/contest`（仅 `ContestPhase` 类型）

## 被依赖

- `components/login/Countdown.vue` — `getContestClock` / `getClockSegments`（登录页倒计时四段与 urgent 变色）
- 比赛外壳侧沿用同一时间契约（阶段判据经 `utils/contest.getContestPhase`）

## 逻辑流程

```
getContestClock(phase, startTime, endTime, now):
  upcoming 且 startTime 非空   → countdown: seconds = max(0, startTime - now)
                                  urgent = seconds <= 15
  running 且 start/end 均非空  → elapsed  : duration = max(0, endTime - startTime)
                                  seconds = min(duration, max(0, now - startTime))
                                  urgent = false
  ended                        → { kind: 'ended',       seconds: null, urgent: false }
  其余（none / 时间缺失）      → { kind: 'unavailable', seconds: null, urgent: false }

getClockSegments(seconds):
  seconds === null → 四段 value 均为 '--'
  否则 [floor(s / 86400), floor((s % 86400) / 3600), floor((s % 3600) / 60), s % 60]
       每段 String(...).padStart(2, '0')
```

设计要点：

- **urgent 阈值收敛到开赛前 15 秒**（`seconds <= 15`，含边界）。曾为「最后 24 小时全程标红」——一整天的告警色没有警示价值，反而让登录页长期处于紧张态；收敛后「变色」即「马上进场」。阈值有单测锁定左/右边界（16 不红 / 15 红 / 14 红）。
- **`elapsed` 秒数封顶到赛程时长**：`min(duration, now - startTime)`，服务端 `endTime` 已过但阶段尚未跃迁时不溢出到荒谬数字。
- **`ended` 与 `unavailable` 分开**：前者是「明确结束」（渲染大字「比赛已结束」），后者是「赛程信息还没拿到」（渲染占位 `--`），两者界面表现必须不同。
- 段值定宽两位（`padStart`）：秒数逐秒变化时四段宽度不跳动。
- 纯函数、零副作用、不依赖 Pinia/IPC，因此可直接单测，也可被登录页之外的时间展示复用。

## 测试

`src/utils/__tests__/contest-clock.spec.ts` 锁定：`countdown` 的 urgent 三档边界（剩 16s 不红、剩 15s 红、剩 14s 红）、`running` 的 `elapsed` 秒数与封顶（now 超过 `endTime` 时等于 duration）、`ended` / `none` 的种类判定、`getClockSegments` 的四段换算与 `null → '--'` 占位。
