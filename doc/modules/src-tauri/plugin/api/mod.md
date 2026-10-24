# Plugin API v2（api/mod）

## 职责
插件可见能力面的模块入口：声明 `PluginApi` 组合 trait 并 re-export 错误/DTO。真实方法以 [hinina.d.ts](../sdk/hinina.d.ts) 为准；由 Runtime 注入的对象已冻结。

## 核心类型/函数
- `pub trait PluginApi: ContextApi + IdentityApi + ContestApi + SubmissionApi + PluginStorageApi + PluginHttpApi + NotificationApi + LogApi` — 能力组合 trait
- `pub use error::{PluginError, PluginResult}` — 错误契约（见 [error.md](error.md)）

## 能力与权限总表
| 能力 | 调用 | 权限 |
|---|---|---|
| 上下文 | `context.current()` | 无；无事件授权也可启动 |
| 提交 | `submission.get(context, contestId, submissionId)`、`submission.list(context, query)` | submission.read |
| 比赛 | `contest.current/get/problems` | contest.read |
| 榜单 | `contest.rank(context, {page, limit})` | contest.rank + contest.read |
| 身份 | `identity.current(context)` | identity.read |
| 存储 | `storage.get/set/delete/compareAndSet` | storage.read / storage.write；CAS 两者都需 |
| HTTP | `http.request(context, request)` | network.request + origin/地址批准 |
| 通知 | `notification.info/success/warn/error(message)` | notification |
| 日志 | `log.debug/info/warn/error(message)` | 无；只写宿主日志文件 |
| 事件 | `events.on(name, (event, envelope) => {})`，返回注销函数 | events.subscribe + 领域读取权限 |

## 直接依赖
- 各能力子模块（contest/error/event/http/identity/log/notification/storage/submission/ui/workspace）
- `async_trait`、`serde`

## 被依赖
- `plugin::execution_api`（`PluginApi` 的唯一生产实现）
- `plugin::runtime`（注入点）、`plugin::query`（DTO 生产方）

## 逻辑流程（上下文与调用纪律）
- **上下文**：不透明随机句柄，绑定当前实际 OJ endpoint、稳定用户 ID、比赛及本次授权；TTL 5 分钟、最多同时 32 个。同 scope+revision 下 150 秒内的未过期句柄被 `context.current` 复用（返回同一句柄），"每个 handler 重新获取 context"不会耗尽上限；句柄超龄、scope 或 revision 变化时才新签发。登录/登出/会话失效、切 OJ/比赛、连接配置变化通过显式 AccessRevision 撤销旧上下文（切走再切回也不复活）；查询前后验证，HTTP/读取还监听取消；重载重新签发授权，旧实例完全撤销
- **配置边界**：普通编辑器配置 update 不改变连接配置时不撤销；配置 reload 保守撤销。修改实例地址不会把旧 Provider 冒充新地址——查询继续绑定运行中的 endpoint，旧上下文随配置变更撤销，新上下文仍用实际 endpoint，重启后才应用新地址
- **限流**：每次桥调用过按激活的准入限流（600 次/分钟），防脚本在本地能力上空转；远端 OJ 调用另受 60 次/分钟、并发 2 的查询预算约束
- **调用纪律**：每次 handler 重新获取当前 context；遇到 `context_expired` 停止本次操作，下次 Resync 重查。未登录身份返回 null；提交查询返回 `unauthenticated`。插件不能指定用户、OJ、Session、权限或真实文件路径
- **错误码**：permission_denied / invalid_input / context_expired / unauthenticated / not_found / provider_error / limit_exceeded / timeout / unavailable / unsupported / script_error。脚本异常以 script_error 脱敏，超时仍为 timeout；IPC 开发命令同样保留稳定错误对象。Resource 不可见与不存在的提交返回 null；扫描预算用尽返回 `limit_exceeded`，不能当成查无记录
- **桥语义**：参数不能 JSON 编码（循环引用、BigInt）时拒绝稳定 `{code:"invalid_input"}`，不抛裸 TypeError；宿主调用在入口硬截止前 1 秒返回 timeout；`submission.get` 整体扫描最多 8 秒。插件可捕获 API 错误继续处理，仍须在激活 10s / 事件 30s 硬期限内完成
- **异步契约**：所有返回 Promise 的宿主 API 必须 await 或纳入 activate/handler 返回的 Promise 链，支持 `await Promise.all`；游离调用在入口结束后不保证继续执行或报告拒绝。`events.on`/注销同步执行。错误载荷仅允许 code 和可选字符串 permission——Rust 穷尽序列化测试与前端形态测试共同锁定该契约
