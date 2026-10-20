# Plugin Manifest v2

以 [manifest.example.json](../sdk/manifest.example.json) 为可加载起点。未知字段、权限、事件均拒绝。

| 字段 | 含义 |
|---|---|
| id | 1..128 字节，小写字母开头的点分段，每段可含数字和连字符；扫描中重复 ID 拒绝 |
| name / version | 展示名 <=128 字节；插件自身 SemVer，由 semver crate 验证 |
| manifestVersion | **2**，清单格式 |
| apiVersion | **2**，可调用 Host API |
| runtime / entry | `js`；插件目录内 `.js`，拒绝 `..`、反斜杠、绝对路径、Windows ADS；canonical 路径再检查符号链接边界 |
| minAppVersion | 可选最低应用 SemVer；高于宿主则不加载 |
| permissions / events | 声明能力；不代表已经授权。events 非空时必须声明 events.subscribe；events 为空时可不声明该权限并正常运行。实际获批 events.subscribe 后才建立事件订阅，Resync 无需单独声明 |
| network.allowOrigins | 精确规范化 origin，例如 `https://service.example.com:8443`，不含路径、认证信息、通配符、查询或片段 |
| network.maxBodyBytes / timeoutMs | 请求体 1..65536 字节，总时间 1..5000ms |
| contributes / configSchema | 当前只接受 `[]` / `{}`，没有 UI 扩展或动态配置表单 |
| description / author | 可选描述 |

Manifest v1/API v1 不再加载，旧 `allowHosts` / `httpsOnly` 必须迁移为精确 origin。没有静默兼容、默认扩权或 runtimeVersion 字段。PluginEventEnvelope.version 独立保持 2。

宿主配置在数据目录的 `plugins.json`，与代码分离；结构见 [plugins.example.json](../sdk/plugins.example.json)。只启用 enabled=true 的 ID。批准权限与声明取交集，插件修改清单或重载不会自动获得新权限。批准的网络 origin 也必须仍在清单声明中。

未获批 `events.subscribe` 的插件仍可激活并调用其他已授权 API，但不接收业务事件或初始/周期 Resync，也不能注册事件回调。

网络额外授权可写为：

```json
{"origin":"http://balloons.internal:8080","allowHttp":true,"privateCidrs":["10.20.30.0/24"]}
```

这是管理员对明文和私网的显式批准，仅示范配置，不会实现气球业务。loopback/link-local/保留等特殊地址即使写入 CIDR 也拒绝。公开 HTTPS origin 不需要 privateCidrs。默认没有任何网络批准。
