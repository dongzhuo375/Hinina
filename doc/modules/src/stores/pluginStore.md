# pluginStore

`usePluginStore` 保存插件状态与短时通知，只依赖 pluginService。start/stop/reload 成功后刷新列表；resync 只提交重查提示。配置文件整体损坏会使 list 拒绝；单插件 approval 非法显示该项 failed。

Store 仅保存已由 pluginService 验证的通知，应用 add/dismiss 事件更新响应式数组；校验、5项上限、6秒过期与定时器清理由 Service 管理。dismiss 请求 Service 释放名额与定时器；subscribe 返回卸载函数，组件负责生命周期。
