# Plugin Permission

PluginGrant::authorize 验证 Manifest，接受宿主已经批准的权限子集；批准未声明权限返回 InvalidInput。Grant 持有不可变清单/权限快照，不可从 JS 反序列化。

require 检查授予集合。仅声明 submission.read 不会获得查询能力；仅通知权限 notification 不能订阅事件。Host 注册需 events.subscribe，单条载荷另按 required_permission 过滤（提交 → submission.read，比赛/公告/题目 → contest.read）。工作区/系统/会话事件只含白名单元数据。

未来 API Adapter 每次调用仍需 require，CAS 同时需 storage.read/write。授权 UI、授权记录、撤销代次与上下文失效尚未实现。当前 Host 的授权撤销方式是显式 unsubscribe；重新接入需重新构造 Grant，不修改存量快照。

[全部权限名与策略](../host/manifest.md)。
