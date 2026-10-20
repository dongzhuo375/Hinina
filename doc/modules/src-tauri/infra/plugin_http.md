# Plugin HTTP Infra

PluginHttp独立reqwest客户端，无OJ Cookie/认证复用、代理、跳转或隐式重试。精确origin+管理员批准、所有DNS结果检查并pin、实际remote_addr验证，有限体积/总超时/并发/速率。私网/HTTP需明确授权，特殊地址不豁免；见[网络规范](../plugin/api/http.md)。

作用域校验调用异步 `ScopedQueries::check`；其中会话文件读取通过 `spawn_blocking`，包括发送前、响应检查时的校验，避免同步文件 I/O 阻塞 Tokio 执行线程。
