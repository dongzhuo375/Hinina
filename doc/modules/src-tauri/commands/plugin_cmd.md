# Plugin 开发命令

plugin_list/start/stop/reload/resync薄封装PluginManager，输入只有插件ID，没有任意代码、路径、URL或权限注入。输出为脱敏状态或 PluginResult 稳定错误 `{code, permission?}`，不再包装为 AppError::Config。dev window.hininaPlugins沿Store → Service → Bridge调用；release按审批配置自动加载，无完整管理页。

`plugin_list` 将配置整体读取/验证失败传回调用方；只有缺失配置才表示空配置。单插件非法 approval 在该项状态报告，不阻塞其他项。启动和重载启动阶段失败还可通过后续列表中的 `failed` 状态及稳定错误码查看。前端 IpcError.code/raw 保留原始错误对象。
