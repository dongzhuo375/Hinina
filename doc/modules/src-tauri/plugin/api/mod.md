# Plugin API 契约

所有 trait 是预留接口，不存在生产实现。Rust PluginApi 为能力 trait 集合；Runtime Bridge 未来注入绑定 PluginGrant 的实现，JS 不能传入 plugin_id 或自行更改授权。声明见 [hinina.d.ts](../sdk/hinina.d.ts)。

| JS 能力（未来） | Rust trait | 权限/关键边界 |
|---|---|---|
| currentContext() | ContextApi | 仅不透明查询句柄，不泄露身份 |
| submission.get(context, contestId, submissionId) | SubmissionApi | submission.read，强制本人和比赛范围 |
| submission.list(context, query) | SubmissionApi | 分页，problemId 为可选真实题号；未完整遍历不能认定首次 AC |
| contest.current/get(context, ...) | ContestApi | contest.read；投影只含 ID/title/起止毫秒 |
| identity.current(context) | IdentityApi | identity.read；只含 ojId/userKey，无 Token/用户名 |
| storage.get/set/delete/compareAndSet | PluginStorageApi | 绑定插件命名空间，读/写分别授权，CAS 同时需读写 |
| http.request(context, request) | PluginHttpApi | network.request，独立客户端及 [安全要求](../security.md) |
| notification.info/success/warn/error | NotificationApi::notify | notification，纯文本 <=512 UTF-8 字节 |
| events.on/off | Host 队列 + 未来 Runtime handler 表 | events.subscribe 与逐事件载荷权限 |

## 查询与投影

currentContext 签发句柄，绑定插件授权代次、当前 OJ/用户/比赛。宿主 Adapter 在异步调用前后验证句柄；上下文变化返回 context_expired，插件重新获取上下文并重扫。句柄可从 wire 反序列化，但字符串本身绝不构成授权。当前类型不执行签发/失效。

PluginSubmission = submissionId/contestId/problemId/problemDisplayId/status/submittedAt。status 使用稳定 JudgementStatus 名称，UTC 秒从内部转换为毫秒，不能拿 occurredAt 代替。详情缺 contestId，因此实现必须用宿主比赛范围和本人列表/授权验证归属，不能直接对内部详情删几个字段就返回。

列表 query 为 contestId、可选 problemId、page、pageSize。ID <=256 UTF-8 字节、page>=1、pageSize=1..100。结果 records/page/pageSize/total/hasMore，每次调用有限，排序按 submittedAt 再按 submissionId 字符串序（不假设各 OJ ID 为数字）。只表达一页，不承诺完整历史或跨页一致快照。首次 AC 必须遍历全部相关历史并再次确认；缺页、受 OJ 隐藏策略限制或并发变化时保守不派发。

## 私有存储

get 返回 {state:'missing'} 或 {state:'value',value:JSON}；JSON null 是存在的值。CAS 的 expected 使用同一联合类型，成功比较/写入应在同一进程插件命名空间临界区中完成，持久化完成后再返回 true。键 1..256 UTF-8 字节且无控制字符；键是数据，不能当文件路径拼接。value 的 JSON 编码 <=65536 字节；未来实现还须 1 MiB/插件总配额、原子写、损坏恢复及重启测试。delete 缺失键成功。卸载/停止保留数据。

当前只有值形状和大小检查，不存在隔离/落盘/CAS 实现；单 Host 原子性不保证多个客户端或外部服务只执行一次。

## HTTP / 通知

HTTP request 接受 GET/POST/PUT/PATCH/DELETE、URL、有限 headers、可选 JSON body/timeoutMs。形状校验 URL<=2048、headers<=32/8192 字节、body<=65536、timeout=1..5000ms；Host/Cookie/连接控制/Proxy-* 头拒绝。网络执行器未来还须用 Manifest 更小上限裁剪并执行 DNS/IP/HTTPS/白名单校验。响应 ok/status/有限头/UTF-8 文本 body；仅公开 content-type/retry-after，body 最大 65536 字节，非 2xx 正常返回 ok=false，不自动重试 POST。

notification 使用纯文本，字面 HTML 不解释；空白/非法控制字符/超长消息拒绝。当前没有前端通知出口。

## 错误

PluginError 序列化为 {code:...}，permission_denied 额外带 permission。错误码：permission_denied、invalid_input、context_expired、unauthenticated、not_found、provider_error、limit_exceeded、timeout、unavailable、unsupported。无权限或不可见的提交/比赛详情用 null 防止探测存在性；权限本身不足仍明确拒绝。内部 AppError 只通过 host/api_error 的白名单映射，不传递 Display/Debug 文本或内部堆栈。

Rust API doc/JSDoc 标注只描述契约，不能代替执行器权限检查或证明能力已可用。
