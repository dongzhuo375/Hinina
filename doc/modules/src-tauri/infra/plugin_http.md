# Plugin HTTP Infra

PluginHttp独立reqwest客户端，无OJ Cookie/认证复用、代理、跳转或隐式重试。精确origin+管理员批准、所有DNS结果检查并pin、实际remote_addr验证，有限体积/总超时/并发/速率。私网/HTTP需明确授权，特殊地址不豁免；见[网络规范](../plugin/api/http.md)。

作用域校验调用异步 `ScopedQueries::check`；其中会话文件读取通过 `spawn_blocking`，包括发送前、响应检查时的校验，避免同步文件 I/O 阻塞 Tokio 执行线程。

PinnedTransport 按 origin 与本次已验证 DNS 地址集合缓存 Client，最多32项；每个请求仍重新解析/检查全部地址，并核对实际 remote_addr。DNS 地址变化不会复用旧地址的客户端，始终禁代理、redirect、retry、Cookie/凭证复用。私有 HttpTransport 端口使公开 request 成功路径可用无网络 fixture 验证，生产绑定 PinnedTransport，不开放绕过地址策略的 IPC/JS 入口。
