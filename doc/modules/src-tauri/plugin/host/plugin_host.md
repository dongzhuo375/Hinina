# PluginHost

唯一CoreEventBus插件消费者；只负责白名单/权限过滤、安全PluginEventAdapter和有界投递，不运行JS、不加载文件、不管理业务动作。

subscribe(&PluginGrant)需events.subscribe；重复ID拒绝。每插件mpsc容量128，初始Resync先于其他投递；过滤后分配sequence。队列满/关闭即注销单插件，worker发现关闭后停止并撤销调用。注销后放弃已缓冲消息，重启先Resync。

request_resync(id)在队列已缓冲时合并提示，周期不会制造无限积压；账号/OJ/比赛变化的事实通知可附加Resync，即使该领域事件未声明。lost=0不代表完整历史。消费者Startup/Lagged沿用core统一消费者；没有第二套业务事件总线。

Manager负责独立worker、配置授权和启停；见[Runtime](../runtime/mod.md)。单插件故障不阻止其他插件或核心。
