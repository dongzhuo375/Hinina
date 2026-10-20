# Plugin Manifest

清单对应 manifest.json。Rust 类型是 plugin/host/manifest.rs；纯 validate 不读文件、解析 DNS、比较应用版本或自动授权。

| 字段 | 必填/默认 | 含义 |
|---|---|---|
| id | 必填 | <=128 字节；点分片段以小写字母开头，其余仅小写字母、数字、连字符 |
| name | 必填 | 非空白、<=128 UTF-8 字节 |
| version | 必填 | 插件自身 SemVer 2.0，不是协议版本 |
| manifestVersion | 必填，1 | 清单格式 |
| apiVersion | 必填，1 | Host API 契约 |
| runtime | 必填，js | 未来 JS 引擎入口；不意味着已能执行 |
| entry | 必填 | <=256 字节，以 .js 结束的相对路径；/ 分隔，拒绝空片段、点路径、反斜杠、冒号和空字符 |
| description / author | 空字符串 | 说明元数据 |
| minAppVersion | 可省略/null | 应用最低 SemVer；当前只检查格式，加载器未来检查兼容 |
| permissions | 必填，可空 | 已知权限名数组，拒绝重复/未知值 |
| events | 必填，可空 | 已知事件名数组，拒绝重复/未知值；非空时需声明 events.subscribe |
| network | 默认下表 | 未来网络执行器的策略上限 |
| contributes | 默认 [] | 当前只能为空，不接收尚未实现的扩展点 |
| configSchema | 默认 {} | 当前只能为空对象，不实现插件配置系统 |

未知字段拒绝，不提供旧 extension_points、snake_case 字段或旧事件名别名。重复插件 ID 在 Host 注册时拒绝；未来目录加载器也应先去重。入口存在性、Windows 保留设备名、符号链接/目录联接、canonical 路径留在插件根内及读取上限必须由未来文件仓库再次校验；纯字符串校验不构成文件沙箱。

## 权限

| 权限 | 能力 |
|---|---|
| events.subscribe | 事件接入，不含用户通知 |
| submission.read | 本人提交查询及提交事件载荷 |
| contest.read | 比赛查询，比赛/公告/题目元数据事件 |
| identity.read | 稳定当前身份投影 |
| storage.read / storage.write | 私有 JSON 存储；CAS 同时需两者 |
| network.request | 受控外部 HTTPS 请求 |
| notification | 纯文本用户通知 |

Manifest 只声明需求。Host 通过 PluginGrant::authorize(manifest, approved) 传入已批准子集；不能批准清单外权限。Rust Grant 不能由 JS 反序列化。尚无授权对话框或授权持久化。

## 网络策略

network 默认 allowHosts=[]、httpsOnly=true、maxBodyBytes=65536、timeoutMs=5000。权限包含 network.request 时 allowHosts 不可空；非空白名单必须声明该权限。默认最多 32 个唯一小写 ASCII/Punycode DNS 名称，不支持通配符、URL、端口、IP 字面量、localhost 或 .local。httpsOnly 必须为 true，maxBodyBytes 1..65536，timeoutMs 1..5000。

DNS 名称仍可能解析到内网；此验证不阻止 DNS rebinding 或私网解析。实际请求安全见 [安全规范](../security.md)。

## 示例（仅文档）

可机器校验的同一份 [manifest.example.json](../sdk/manifest.example.json)：

```json
{
  "id": "com.example.balloon-first-ac",
  "name": "首次 AC 气球派发（接口示例）",
  "version": "1.0.0",
  "manifestVersion": 1,
  "runtime": "js",
  "entry": "index.js",
  "apiVersion": 1,
  "permissions": ["events.subscribe", "contest.read", "submission.read", "identity.read", "storage.read", "storage.write", "network.request", "notification"],
  "events": ["submission.judged", "resync.required"],
  "network": {"allowHosts": ["balloon.example.com"], "httpsOnly": true, "maxBodyBytes": 65536, "timeoutMs": 5000},
  "contributes": [],
  "configSchema": {}
}
```

此示例没有入口文件或扫描加载行为，也没有气球业务实现。
