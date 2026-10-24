# Plugin API v2

真实方法以 [hinina.d.ts](../sdk/hinina.d.ts) 为准；由 Runtime 注入的对象已冻结。所有 I/O 方法返回 Promise，失败抛稳定 `{code, permission?}`，无 Rust 栈、原始响应或本地路径。

| 能力 | 调用 | 权限 |
|---|---|---|
| 上下文 | `context.current()` | 无；无事件授权也可启动 |
| 提交 | `submission.get(context, contestId, submissionId)`、`submission.list(context, query)` | submission.read |
| 比赛 | `contest.current(context)`、`contest.get(context, contestId)`、`contest.problems(context)` | contest.read |
| 榜单 | `contest.rank(context, {page, limit})` | contest.rank + contest.read |
| 身份 | `identity.current(context)` | identity.read |
| 存储 | `storage.get/set/delete/compareAndSet` | storage.read / storage.write；CAS 两者都需 |
| HTTP | `http.request(context, request)` | network.request + origin/地址批准 |
| 通知 | `notification.info/success/warn/error(message)` | notification |
| 日志 | `log.debug/info/warn/error(message)` | 无；只写宿主日志文件 |
| 事件 | `events.on(name, (event, envelope) => {})`，返回注销函数 | events.subscribe + 领域读取权限 |

上下文是不透明随机句柄，绑定当前实际 OJ endpoint、稳定用户 ID、比赛及本次授权；TTL 5 分钟，最多同时保留 32 个。同一 scope 与 revision 下 150 秒内的未过期句柄会被 `context.current` 复用（返回同一句柄），因此按"每个 handler 重新获取 context"的习惯不会耗尽上限；句柄超龄、scope 或 revision 变化时才新签发。登录/登出/会话失效、切 OJ/比赛和连接配置变化通过显式 AccessRevision 撤销旧上下文，切走再切回来也不会复活。查询前后验证，HTTP/读取还监听取消；重载重新签发授权，旧实例完全撤销。

普通编辑器配置 update 不改变连接配置时不撤销；配置 reload 保守撤销。修改实例地址不会把已创建的旧 Provider 冒充新地址：查询继续绑定运行中的 endpoint，旧上下文随配置变更撤销，新上下文仍使用实际 endpoint，重启后才应用新地址。

每次桥调用都要通过按激活的准入限流（600 次/分钟），避免脚本在 `context.current`/`identity.current`/`storage.*` 等本地能力上空转；远端 OJ 调用另受 60 次/分钟与并发 2 的查询预算约束。

每次 handler 应重新获取当前 context；遇到 context_expired 停止本次操作，下次 Resync 重查。未登录身份返回 null；提交查询返回 unauthenticated。插件不能指定用户、OJ、Session、权限或真实文件路径。

错误码：permission_denied、invalid_input、context_expired、unauthenticated、not_found、provider_error、limit_exceeded、timeout、unavailable、unsupported、script_error。脚本语法/运行时异常以 script_error 脱敏，超时仍为 timeout；IPC 开发命令同样保留稳定错误对象。Resource 不可见和不存在的提交均返回 null；扫描预算用尽返回 limit_exceeded，不能当成查无记录。

桥参数不能JSON编码（如循环引用或BigInt）时同样拒绝稳定的{code:"invalid_input"}对象，不抛裸TypeError。宿主调用在入口硬截止前1秒返回timeout；submission.get整体扫描最多8秒。插件可捕获API错误继续处理，仍须在激活10秒/事件30秒硬期限内完成。

所有返回Promise的宿主API必须await或纳入activate/handler返回的Promise链，支持await Promise.all；游离调用在入口结束后不保证继续执行或报告拒绝。events.on/注销同步执行。错误载荷仅允许code和可选字符串permission，Rust穷尽序列化测试与前端形态测试共同锁定该契约。
