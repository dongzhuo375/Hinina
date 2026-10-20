# PluginNotifications

全局纯文本插件通知出口，挂载时订阅 pluginStore，卸载时取消订阅；异步订阅晚于卸载返回时立即清理。显示插件标识、消息和关闭按钮，通过 Vue 插值转义，不使用 v-html。`aria-live="polite"` 提供读屏提示。

业务调用遵循 Component → Store → Service → Bridge → Rust；通知不能扩展为任意插件 UI 或代码执行入口。
