# Plugin HTTP Infra

PluginHttp独立reqwest客户端，无OJ Cookie/认证复用、代理、跳转或隐式重试。精确origin+管理员批准、所有DNS结果检查并pin、实际remote_addr验证，有限体积/总超时/并发/速率。私网/HTTP需明确授权，特殊地址不豁免；见[网络规范](../plugin/api/http.md)。
