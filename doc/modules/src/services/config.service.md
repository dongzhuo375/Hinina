# config.service（前端配置服务）

> 源文件：`src/services/config.service.ts`

## 职责

前端读取 Rust 端 `AppConfig` 的唯一入口，并派生出各层需要的展示/调度参数，屏蔽 IPC 细节与配置字段兜底逻辑。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `PollSchedule` | `{ intervalMs: number; timeoutMs: number }` | 评测轮询调度参数 |
| `ConfigService.getConfig` | `() => Promise<AppConfig>` | 读取配置（进程内缓存，并发共享同一次 IPC） |
| `ConfigService.invalidate` | `() => void` | 使缓存失效（预留配置热重载） |
| `ConfigService.getOjBaseUrl` | `() => Promise<string>` | OJ 基址，用于题面/简介相对图片 URL 改写；失败返回空串 |
| `ConfigService.getPollSchedule` | `() => Promise<PollSchedule>` | 轮询间隔与总超时；失败或非法配置回退 2s / 300s |
| `configService` | 单例 | 全局唯一实例 |

## 直接依赖

- `@/bridge/config.bridge`（`get_config`）
- `@/types/config`（仅类型）

## 被依赖

- `services/contest.service.ts` — 登录页匿名简报需要 `contestId` 与 OJ 基址
- `stores/submissionStore.ts` — `startPolling()` 读取轮询调度参数
- `components/problem/ProblemStatement.vue` — 题面图片基址

## 逻辑流程

```
getConfig()
  ├─ 缓存命中 → 直接返回同一 Promise（并发去重）
  └─ 未命中   → config.bridge.getConfig()
                 ├─ 成功 → 写入缓存
                 └─ 失败 → 清空缓存（下次调用重试）并抛出

getOjBaseUrl() / getPollSchedule()
  └─ 内部吞掉异常并返回安全兜底值：
     基址缺失只影响图片显示、轮询参数缺失只影响节奏，均不应阻断主流程
```

设计要点：

- **分层约定**：View / Store 不得直接调用 `config.bridge`，配置读取一律经本服务。
- **不引入 store**：配置是应用级只读数据，无跨视图状态同步需求；缓存 Promise 而非结果，
  使登录页简报、题面图片基址、轮询参数三处调用共享一次 IPC。
- 兜底值与 Rust `core::entity::config` 的默认值保持一致（`poll_interval=2s`、`poll_timeout=300s`）。
- v0.x 无设置界面，运行期配置稳定；接入热重载后由调用方触发 `invalidate()`。
