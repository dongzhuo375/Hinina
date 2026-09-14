# contest（比赛阶段推导）

> 源文件：`src/utils/contest.ts`

## 职责

以纯函数形式集中"比赛阶段"判据，供登录页（决定是否进入赛场）与顶部栏（状态标签/倒计时文案）共用，避免同一规则在多处各写一份。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `ContestPhase` | `'none' \| 'upcoming' \| 'running' \| 'ended'` | 比赛阶段枚举（`none` = 比赛信息缺失） |
| `getContestPhase` | `(contest: Contest \| null \| undefined, nowSecs: number) => ContestPhase` | 由比赛实体与当前时间推导阶段 |
| `hasContestStarted` | `(phase: ContestPhase) => boolean` | 阶段是否已进入进行中/已结束，即能否进入赛场 |

## 直接依赖

- `@/types/contest`（仅类型）

## 被依赖

- `views/LoginView.vue` — `contestPhase` / `canEnter`（驱动登录后是否自动进入赛场）
- `components/layout/AppHeader.vue` — `timeStatus` / `timeStatusType`

## 逻辑流程

```
contest 为空                      → none
status === 1 或 now >= endTime    → ended      （服务端 status 仅在"已结束"上作为权威判据）
now >= startTime                  → running
其余                               → upcoming
```

设计要点：

- 服务端 `status`（-1=未开始，0=进行中，1=已结束）是列表接口的快照，可能过期；
  开始/进行中的切换依赖调用方传入的本地时间戳，使倒计时归零瞬间即可触发阶段跃迁。
- 纯函数、无副作用、不依赖 Pinia/IPC，因此可在任意层复用并易于单元测试。

## 测试

`src/utils/__tests__/contest.spec.ts` 锁定：四阶段判定与边界（含左/右边界归属）、服务端 `status=1` 优先判定已结束、开始/进行中切换以本地时钟为准（status 是可能过期的快照）、`hasContestStarted` 仅进行中/已结束算已开始。
