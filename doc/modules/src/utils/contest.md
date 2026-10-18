# contest（比赛阶段与权威赛程推导）

> 源文件：`src/utils/contest.ts`

## 职责

以纯函数集中「比赛阶段」判据与「权威赛程选择」，供登录页、路由守卫与比赛外壳共用，避免同一规则在多处各写一份。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `ContestPhase` | `'none' \| 'upcoming' \| 'running' \| 'ended'` | 比赛阶段枚举（`none` = 比赛信息缺失） |
| `getAuthoritativeSchedule` | `(contest: Contest \| null \| undefined, brief: Contest \| null \| undefined) => Contest \| null` | 在「认证比赛」与「匿名简报」之间选出权威赛程 |
| `getContestPhase` | `(contest: Contest \| null \| undefined, nowSecs: number) => ContestPhase` | 由赛程实体与当前时间推导阶段 |

## 直接依赖

- `@/types/contest`（仅类型）

## 被依赖

- `views/LoginView.vue` — `getAuthoritativeSchedule` / `getContestPhase`（`schedule` / `contestPhase` / `canEnter`）
- `router/index.ts` — 守卫的赛程阶段闸门（深链进入工作台前判 `phase === 'running'`）
- `components/login/Countdown.vue` 与 `components/layout/TopBar.vue` — 阶段判据的展示消费方（时钟模型见 `utils/contest-clock`）

## 逻辑流程

```
getAuthoritativeSchedule(contest, brief):
  无 contest → brief ?? null
  无 brief   → contest
  两者都有   → id 相同 ? contest（认证数据更完整/更新） : brief（反映服务端当前赛程）

getContestPhase(contest, nowSecs):
  contest 为空                      → none
  status === 1 或 now >= endTime    → ended      （服务端 status 仅在"已结束"上作为权威判据）
  now >= startTime                  → running
  其余                               → upcoming
```

设计要点：

- 服务端 `status`（-1=未开始，0=进行中，1=已结束）是列表接口的快照，可能过期；
  开始/进行中的切换依赖调用方传入的本地时间戳，使倒计时归零瞬间即可触发阶段跃迁。
- **`getAuthoritativeSchedule` 解决「缓存的认证赛程过期/跨场」问题**：登录页同时持有
  认证态的 `contestStore.contest`（可能来自上一次会话、甚至是另一场比赛）与匿名的
  `contestStore.brief`（服务端当前赛程）。**ID 不同即视为换场，以简报为准** ——
  否则守卫会拿旧赛程判阶段，把选手拦在/放进错误的赛场。
- **已删除 `hasContestStarted`**：旧语义是「running ∨ ended」，会让已结束的比赛也能进场；
  现由调用方直接判 `phase === 'running'`，语义更窄也更明确（见 `views/LoginView.md` 的 `canEnter`）。
- 纯函数、无副作用、不依赖 Pinia/IPC，因此可在任意层复用并易于单元测试。

## 测试

`src/utils/__tests__/contest.spec.ts` 锁定：四阶段判定与边界（含左/右边界归属）、服务端 `status=1` 优先判定已结束、开始/进行中切换以本地时钟为准（status 是可能过期的快照）；`getAuthoritativeSchedule` 的同 id 取认证数据、ID 不同取简报、仅一份/都无时的回退。
