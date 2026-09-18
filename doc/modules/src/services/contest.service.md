# contest.service（比赛服务）

> 源文件：`src/services/contest.service.ts`

## 职责

比赛领域的服务层：登录后「加载配置的比赛 + 题目列表」与登录页「匿名比赛简报」两条链路的编排；简报链路把 config → contestRef → 匿名列表筛选的组合逻辑收敛在此，Store 只做状态映射。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `ContestBriefResult` | `{ status: 'ok'; contest: Contest \| null; baseUrl } \| { status: 'unconfigured'; baseUrl }` | 简报加载结果。**`ok` 时 `contest` 仍可能为 null**（配置的 ID 不在列表中）；网络/IPC 异常不在枚举内，直接上抛由调用方归一化 |
| `ContestService.loadConfiguredContest` | `() => Promise<{ contest; problems }>` | 转发 bridge `load_configured_contest`（后端读 `oj.contest_ref`，返回 ContestBundle） |
| `ContestService.loadContestBrief` | `() => Promise<ContestBriefResult>` | 匿名简报编排，见逻辑流程 |
| `contestService` | 单例 | 全局唯一实例 |

## 直接依赖

- `@/bridge/contest.bridge`（`loadConfiguredContest` / `listContests`）
- `@/services/config.service`（读取 `oj.contestRef`，经 `activeOjBaseUrl` 解析当前实例基址）
- `@/types/contest`（仅类型）

## 被依赖

- `stores/contestStore.ts` — 唯一调用方（`loadContest` / `loadBrief`）

## 逻辑流程

```
loadContestBrief():
  configService.getConfig() → baseUrl = activeOjBaseUrl(config)，contestRef = oj.contestRef.trim()
  contestRef 未配置（空串）→ { status: 'unconfigured', baseUrl }   // 配置缺失是提示态，不是错误
  contestBridge.listContests()（匿名接口，无需会话）
  → 按 id === contestRef 筛选 → { status: 'ok', contest: 命中 ?? null, baseUrl }
  任一 IPC 异常 → 上抛（contestStore.loadBrief 捕获后归一为 briefState='failed'）
```

设计要点：

- **`unconfigured` 与 `failed` 分离**：前者是「还没配置比赛 ID」的引导态（登录页提示完成
  配置），后者是网络/服务端故障（登录页提供重试）；混在一起会让排障指引失真。
- 同时返回 `baseUrl`：登录页简报与题面渲染都需要把相对图片 URL 改写为 OJ 绝对地址
  （`utils/markdown`），在服务层一次取齐避免调用方再发一次配置读取。
- 简报走**匿名**比赛列表接口：登录页尚无会话，不能依赖需认证的 `load_configured_contest`。
- 无状态、无缓存：比赛列表缓存归后端（ContestService TTL），前端不重复建缓存层。
