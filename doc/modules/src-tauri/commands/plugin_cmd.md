# Plugin 开发命令

plugin_list/start/stop/reload/resync薄封装PluginManager，输入只有插件ID，没有任意代码、路径、URL或权限注入。输出为脱敏状态或AppResult安全诊断。dev window.hininaPlugins沿Store → Service → Bridge调用；release按审批配置自动加载，无完整管理页。

`plugin_list` 将配置读取/验证失败通过 `AppResult` 传回调用方；只有缺失配置才表示空配置。启动和重载启动阶段失败还可通过后续列表中的 `failed` 状态及稳定错误码查看。
