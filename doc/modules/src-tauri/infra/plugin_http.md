# Plugin HTTP Infra

PluginHttp独立reqwest客户端，无OJ Cookie/认证复用、代理、跳转或隐式重试。精确origin+管理员批准、所有DNS结果检查并pin、实际remote_addr验证，有限体积/总超时/并发/速率。私网/HTTP需明确授权，特殊地址不豁免；见[网络规范](../plugin/api/http.md)。

作用域校验调用异步 `ScopedQueries::check`；缓存命中时直接返回，未命中才通过 `spawn_blocking` 读取会话文件，包括发送前、响应检查时的校验，避免同步文件 I/O 阻塞 Tokio 执行线程，也避免热路径上的阻塞池往返。

PinnedTransport 按 origin 与本次已验证 DNS 地址集合缓存 Client，最多32项；每个请求仍重新解析/检查全部地址，并核对实际 remote_addr。DNS 地址变化不会复用旧地址的客户端，始终禁代理、redirect、retry、Cookie/凭证复用。私有 HttpTransport 端口使公开 request 成功路径可用无网络 fixture 验证，生产绑定 PinnedTransport，不开放绕过地址策略的 IPC/JS 入口。

宿主侧失败调用plugin_diagnostics输出warn：保留操作阶段及安全错误类别，不输出原始错误文本、URL、凭据、绝对路径或业务内容；插件侧PluginError保持原契约。

先获取并发许可再消耗速率令牌，并发拒绝不扣令牌；许可归有界请求future所有，取消/超时/丢弃future后归还。测试用阻塞Resolver验证任务取消与作用域撤销后可立即继续请求。remote_addr是在发送后的纵深核查；发送前SSRF边界由全部DNS结果校验和地址固定承担。
