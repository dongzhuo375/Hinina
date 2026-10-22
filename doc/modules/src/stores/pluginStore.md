# pluginStore

`usePluginStore` 保存插件状态与短时通知，只依赖 pluginService。start/stop/reload 成功后刷新列表；resync 只提交重查提示。配置文件整体损坏会使 list 拒绝；单插件 approval 非法显示该项 failed。

Store 仅保存已由 pluginService 验证的通知，应用 add/dismiss 事件更新响应式数组；校验、5项上限、6秒过期与定时器清理由 Service 管理。dismiss 请求 Service 释放名额与定时器；subscribe 返回卸载函数，组件负责生命周期；重复 subscribe 会先卸载旧订阅再建立新订阅，组件重挂载/HMR 不泄漏监听器、不产生重复通知。

subscribe 使用单调代次防止并发重叠：两次调用在首个 await 完成前重叠时，迟到的那次结果会被立即卸载，不会成为活动订阅，也不会接管 dismiss 指向。返回的卸载函数只关闭自己创建的订阅（且仅当它仍是活动订阅），因此卸载旧订阅不会影响更新的订阅；代次在卸载时同步推进，迟到的结果同样被丢弃。
