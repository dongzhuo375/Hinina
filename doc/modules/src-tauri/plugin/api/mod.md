# Plugin API v2

真实方法以 [hinina.d.ts](../sdk/hinina.d.ts) 为准；由 Runtime 注入的对象已冻结。所有 I/O 方法返回 Promise，失败抛稳定 `{code, permission?}`，无 Rust 栈、原始响应或本地路径。

| 能力 | 调用 | 权限 |
|---|---|---|
| 上下文 | `context.current()` | 无；启动插件仍需 events.subscribe |
| 提交 | `submission.get(context, contestId, submissionId)`、`submission.list(context, query)` | submission.read |
| 比赛 | `contest.current(context)`、`contest.get(context, contestId)` | contest.read |
| 身份 | `identity.current(context)` | identity.read |
| 存储 | `storage.get/set/delete/compareAndSet` | storage.read / storage.write；CAS 两者都需 |
| HTTP | `http.request(context, request)` | network.request + origin/地址批准 |
| 通知 | `notification.info/success/warn/error(message)` | notification |
| 事件 | `events.on(name, (event, envelope) => {})`，返回注销函数 | events.subscribe + 领域读取权限 |

上下文是不透明随机句柄，绑定当前实际 OJ endpoint、稳定用户 ID、比赛及本次授权；最多 32 个，TTL 5 分钟。登录/登出/会话失效、切 OJ/比赛和连接配置变化通过显式 AccessRevision 撤销旧上下文，切走再切回来也不会复活。查询前后验证，HTTP/读取还监听取消；重载重新签发授权，旧实例完全撤销。

普通编辑器配置 update 不改变连接配置时不撤销；配置 reload 保守撤销。修改配置地址不会把已创建的旧 Provider 冒充新地址：不一致时查询返回 unavailable，需宿主重启以重建 Provider。

每次 handler 应重新获取当前 context；遇到 context_expired 停止本次操作，下次 Resync 重查。未登录身份返回 null；提交查询返回 unauthenticated。插件不能指定用户、OJ、Session、权限或真实文件路径。

错误码：permission_denied、invalid_input、context_expired、unauthenticated、not_found、provider_error、limit_exceeded、timeout、unavailable、unsupported。Resource 不可见和不存在的提交均返回 null；扫描预算用尽返回 limit_exceeded，不能当成查无记录。
