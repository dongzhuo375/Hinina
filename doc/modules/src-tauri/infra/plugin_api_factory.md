# Plugin 能力工厂

`FsPluginApiFactory` 实现 `PluginApiFactory` 端口，由 AppContext 与 `Arc<dyn PluginRepository>` 一起注入 Manager。`create` 为当前 ScopedQueries 装配 JsonPluginStorage、PluginHttp、ExecutionApi；不接收插件指定的任意路径或凭据。

数据根由组合根指定为 plugin-data。工厂持有所有激活共用的存储预算（240 次/分钟，允许相应突发），重载不能重置该总预算；每次激活另有60次/分钟操作预算。Manager 测试可直接替换仓库与工厂，无需真实插件目录。

宿主侧失败调用plugin_diagnostics输出warn：保留操作阶段及安全错误类别，不输出原始错误文本、URL、凭据、绝对路径或业务内容；插件侧PluginError保持原契约。
