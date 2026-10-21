# Plugin Notification Infra

TauriPluginNotifications只发经验证的纯文本通知到plugin-notification。前端PluginNotifications经Store/Service/Bridge接收并文本插值展示；易失，不保证确认送达。见[通知规范](../plugin/api/notification.md)。

宿主侧失败调用plugin_diagnostics输出warn：保留操作阶段及安全错误类别，不输出原始错误文本、URL、凭据、绝对路径或业务内容；插件侧PluginError保持原契约。
