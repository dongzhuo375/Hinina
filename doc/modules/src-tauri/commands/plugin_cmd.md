# Plugin 开发命令

plugin_list/start/stop/reload/resync薄封装PluginManager，输入只有插件ID，没有任意代码、路径、URL或权限注入。输出为脱敏状态或AppResult安全诊断。dev window.hininaPlugins沿Store → Service → Bridge调用；release按审批配置自动加载，无完整管理页。
