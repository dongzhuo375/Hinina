# PluginManager

管理enabled且获批准的插件；启动/停止/重载串行化，每插件独立线程与QuickJS实例，AppContext只持Send控制器。只保留starting/running/stopped/failed，避免复杂生命周期状态机。

仅实际获批 `events.subscribe` 的插件建立事件队列、开放事件回调并接收初始/周期 Resync（默认60秒）；没有该授权也可正常激活、停止和重载，手动 resync 因无订阅返回 Unavailable。Host 订阅的 PermissionDenied 映射保留权限语义。stop撤销权限、注销订阅、取消API并等待worker释放，reload不能与旧worker并存。退出同步撤销，不干预工作区保存；详见[Runtime](runtime/mod.md)。

`list()` 返回 `PluginResult<Vec<PluginStatus>>`：缺失 `plugins.json` 仍代表没有配置插件，文件损坏或配置验证失败则返回安全错误，不能伪装为空列表。显式启动、自动启动和重载的启动阶段失败均保留 `failed` 与稳定错误码；成功重启后的 worker 状态覆盖旧诊断。
