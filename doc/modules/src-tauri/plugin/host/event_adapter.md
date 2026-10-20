# PluginEventAdapter

CoreEvent → PluginEvent 的唯一白名单/裁剪点，不创建 channel 或持有 Service。TokenRotated 始终返回 None，登录丢弃用户 ID，公告只给数量，工作区不带路径或内容。

adapt_if 先投影，再按 Host 授权/声明筛选，再递增每订阅 sequence；过滤不会产生假缺口。adapt 是不附加订阅筛选的底层投影入口，Host 生产路径只用 adapt_if。resync_notice 生成强制控制通知。每订阅序号从 1 起，重新注册重置，不是全局持久序号。

信封 version=2，type 与 Manifest 点分事件名一致。occurredAt 为适配时刻（UTC 毫秒），不可当作提交时间。新事件必须显式加入独立协议和白名单，不得直接序列化 CoreEvent。

测试锁定所有变体投影、终态摘要、版本和序号、过滤、TokenRotated 排除以及敏感字段不进入协议。
