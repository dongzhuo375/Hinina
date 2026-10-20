# PluginHost

单个 CoreEventBus 消费者，接收核心事实并向每插件队列交付安全投影；不是可发布业务事实的总线。

- new(bus) 只构造 Host；start() 在 Tokio 上下文启动一次，重复调用返回 AlreadyStarted。
- subscribe(&PluginGrant) 返回 mpsc Receiver<PluginEventEnvelope>，未授予 events.subscribe 返回 PermissionDenied；重复 ID 返回 AlreadySubscribed。
- 初始 Resync 在注册锁内先入空队列，再插入订阅表，杜绝并发重复注册方收到额外通知或 seq=2 先于 seq=1。
- 每插件容量固定 128（包含 Resync）；外部无法注入同步回调或无界 sender。PluginEventSink 为宿主私有构造的队列出口，不再是可实现的 trait。
- 声明和授予的领域权限共同决定投递集合；过滤后分配序号。Resync 无需事件声明，始终尝试投递。
- 入队和注销使用同一锁，临界区只有转换/try_send，无用户代码、await 或 I/O。满/关闭返回失败，移除对应订阅并丢弃其 sender；其他订阅继续。
- unsubscribe(id) 幂等，返回后不再入队；已缓存消息仍可能读出，未来 worker 停止时必须主动丢弃。
- 接收端关闭最迟在下一条核心事件/Resync 时从表中移除。队列满不自动重连、不积累重试任务；未来 Manager/worker 显式重新授权接入，先处理初始 Resync。

消费者闭包只捕获订阅表，不延长总线生命。保留 core::event 的 Lagged/Closed/监督重启语义。关闭总线不代表历史事件可重放。

测试覆盖并发重复注册、过滤不耗序号、慢/关闭消费者隔离、控制通知溢出、重连 Resync、注销后不再入队、启动一次、真实 broadcast 投递和 Lagged。任意 JS handler 异常/死循环隔离尚未实现，因为当前没有执行器。
