# PluginRepository

端口在 plugin/registry，包含宿主 PluginConfiguration/PluginApproval 和 LoadedPlugin；I/O 实现在 infra/fs_plugin_repo，不再保留 Unsupported 占位实现。

`configuration()` 读取 `<base_dir>/plugins.json`，缺失时无插件；`load(id)` 只加载已启用批准的ID。默认代码根 `<base_dir>/plugins`，管理员可在 plugins.json 的 codeRoot 指定绝对开发路径。最多扫描128个目录；重复目标ID拒绝、未知清单/最低应用版本拒绝、入口canonical路径检查、源码大小限制。无关损坏插件不阻止有效插件加载。

清单和源码可读不等于授权；读取批准与声明的交集，执行前由Manager创建独立授权代次。入口加载后以内存源码执行；本地能修改宿主配置/安装目录的管理员在信任边界内，本实现不抵抗管理员同时替换磁盘对象的攻击。

配置整体 JSON/根字段/目录约束失败才使 configuration 整体失败；每个 approval 独立解析校验，错误进入宿主私有 invalid_plugins，Manager 列表逐项报告。load(id) 只拒绝目标的错误，非法权限、网络或 resync 间隔的无关插件不会阻塞好插件。Manager 通过 Arc<dyn PluginRepository> 使用端口，具体 FsPluginRepository 由 AppContext 选择。
