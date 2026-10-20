# pluginStore

`usePluginStore` 保存插件状态与短时通知，只依赖 pluginService。start/stop/reload 成功后刷新列表；resync 只提交重查提示。配置文件整体损坏会使 list 拒绝；单插件 approval 非法显示该项 failed。

通知校验 pluginId、级别和消息长度，最多保留5项，6秒后移除；dismiss 显式关闭。subscribe 返回卸载函数，组件负责生命周期。
