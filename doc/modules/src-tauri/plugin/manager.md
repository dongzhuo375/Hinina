# PluginManager

管理enabled且获批准的插件；启动/停止/重载串行化，每插件独立线程与QuickJS实例，AppContext只持Send控制器。只保留starting/running/stopped/failed，避免复杂生命周期状态机。

定期Resync默认60秒；stop撤销权限、注销订阅、取消API并等待worker释放，reload不能与旧worker并存。退出同步撤销，不干预工作区保存；详见[Runtime](runtime/mod.md)。
