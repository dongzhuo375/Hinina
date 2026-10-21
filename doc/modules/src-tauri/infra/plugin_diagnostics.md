# 插件基础设施安全诊断

plugin_diagnostics 为文件仓库、存储、HTTP、通知与能力工厂提供宿主侧 warn 日志。记录插件标识（非安全ID替换为占位）、静态操作名和错误类别；I/O 仅记 ErrorKind，serde 仅记 classify，reqwest 只分连接/超时/构建/响应体等类别。

禁止格式化源错误或输出URL、路径、HTTP头、请求/响应体、通知正文、源码、配置原文。插件侧仍返回原有稳定 PluginError。存储的 spawn_blocking 继承调用方 dispatcher，保证失败日志可捕获；正常缺失 plugins.json 和未创建的 storage.json 不告警。

回归测试覆盖损坏配置、损坏存储和带敏感内容的 I/O/HTTP 错误，断言阶段、类别可见且原始秘密不出现在日志中。
