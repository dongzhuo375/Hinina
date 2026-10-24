# host/manifest

## 职责
Plugin Manifest v2 的解析与校验：未知字段、权限、事件均拒绝；`PluginPermission` / `PluginEventSubscription` 枚举与 origin 合法性判定的唯一来源。以 [manifest.example.json](../sdk/manifest.example.json) 为可加载起点。

## 核心类型/函数
- `PluginPermission`（enum）— events.subscribe / submission.read / contest.read / contest.rank / identity.read / storage.read / storage.write / network.request / notification
- `PluginEventSubscription`（enum）— Manifest events 字段的合法事件
- `PluginNetworkPolicy` — allowOrigins / maxBodyBytes / timeoutMs（默认值在 `Default`）
- `PluginManifest` — 全字段清单结构（deny_unknown_fields）
- `ManifestError(&'static str)` — 稳定校验错误（不含动态内容）
- `valid_origin(origin) -> bool` — 规范化域名 origin 判定（拒绝一切 IP 字面量）

## 直接依赖
- `serde`、`std::collections::HashSet`、`thiserror`
- `url`（`valid_origin` 的 origin 解析与域名判定）、`semver`（version/minAppVersion 校验）

## 被依赖
- `plugin::permission`（授权交集计算）、`plugin::registry`（清单加载）、`plugin::host::event_adapter`（事件白名单）
- `infra::plugin_http`（`NetworkApproval::validate` 复用 `valid_origin`）

## 逻辑流程（字段约束）
| 字段 | 含义 |
|---|---|
| id | 1..128 字节，小写字母开头的点分段，每段可含数字和连字符；扫描中重复 ID 拒绝 |
| name / version | 展示名 ≤128 字节；插件自身 SemVer，由 semver crate 验证 |
| manifestVersion / apiVersion | **2** / **2** |
| runtime / entry | `js`；插件目录内 `.js`，拒绝 `..`、反斜杠、绝对路径、Windows ADS；canonical 路径再检查符号链接边界 |
| minAppVersion | 可选最低应用 SemVer；高于宿主则不加载 |
| permissions / events | 声明能力，**不代表已经授权**。events 非空时必须声明 events.subscribe；events 为空时可不声明该权限并正常运行。实际获批 events.subscribe 后才建立事件订阅，Resync 无需单独声明 |
| network.allowOrigins | 精确规范化 origin（如 `https://service.example.com:8443`），不含路径、认证信息、通配符、查询或片段 |
| network.maxBodyBytes / timeoutMs | 请求体 1..65536 字节，总时间 1..5000ms |
| contributes / configSchema | 当前只接受 `[]` / `{}`，没有 UI 扩展或动态配置表单 |
| description / author | 可选描述 |

- Manifest v1/API v1 不再加载，旧 `allowHosts` / `httpsOnly` 必须迁移为精确 origin；没有静默兼容、默认扩权或 runtimeVersion 字段。`PluginEventEnvelope.version` 独立保持 2

## 逻辑流程（宿主配置 plugins.json）
- 数据目录的 `plugins.json` 与代码分离；结构见 [plugins.example.json](../sdk/plugins.example.json)。只启用 `enabled=true` 的 ID；批准权限与声明取**交集**，插件修改清单或重载不会自动获得新权限；批准的网络 origin 也必须仍在清单声明中
- `contest.rank`（全场榜单）是独立于 `contest.read` 的权限：榜单暴露其他队伍的显示名与学校，应单独批准
- 未获批 `events.subscribe` 的插件仍可激活并调用其他已授权 API，但不接收业务事件或初始/周期 Resync，也不能注册事件回调
- 网络额外授权示例（管理员对明文和私网的显式批准，仅示范配置）：

```json
{"origin":"http://balloons.internal:8080","allowHttp":true,"privateCidrs":["10.20.30.0/24"]}
```

- loopback/link-local/保留等特殊地址即使写入 CIDR 也拒绝；公开 HTTPS origin 不需要 privateCidrs；默认没有任何网络批准
- origin 必须使用规范化域名：拒绝所有 IPv4/IPv6 字面量（含公网 IP），没有旧 IP 清单的隐式兼容。域名合法不代表地址已授权——执行时仍检查 DNS 全部结果并固定连接；私网域名需匹配宿主 privateCidrs 批准
