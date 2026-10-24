# api/http

## 职责
受控 HTTP 能力契约与形状校验：精确 origin、DNS 校验并固定、无 Cookie/代理/跳转/重试的隔离出站请求。

## 核心类型/函数
- `MAX_HTTP_BODY_BYTES: usize = 65_536` — 请求/响应体宿主硬上限；Manifest 可进一步收紧请求体
- `PluginHttpMethod` — GET / POST / PUT / PATCH / DELETE
- `PluginHttpRequest` — method、url、headers（BTreeMap）、body（JSON）、timeoutMs；`validate_shape()` 校验：URL ≤2048 B、headers ≤32 个且合计 ≤8192 B、timeout 1..=5000ms、黑名单头与控制字符拒绝（错误 InvalidInput/LimitExceeded）
- `PluginHttpResponse` — ok、status、headers（仅 content-type/retry-after）、body（有限长 UTF-8；不暴露 Set-Cookie）
- `PluginHttpApi`（trait）— `request(context, request)`；非 2xx 作为响应返回，网络失败 `unavailable`、超时 `timeout`、超限 `limit_exceeded`
- `NetworkApproval` — origin + `allow_http` + `private_cidrs`（宿主批准，validate 拒绝 IP 字面量与非法 CIDR）

## 直接依赖
- `plugin::api`（错误/上下文）、`async_trait`、`serde`、`std::collections::BTreeMap`

## 被依赖
- `plugin::execution_api`（能力转发）
- `infra::plugin_http`（trait 实现：执行器在 infra，见 [Architecture](../../../../Architecture.md)）

## 逻辑流程
### 批准与地址策略
- 需要 `network.request`，并同时满足清单（`network.allowOrigins`）和 plugins.json 批准的精确 origin。默认只允许公开单播地址上的 HTTPS；HTTP 需 `allowHttp=true`，RFC1918/ULA 需额外 `privateCidrs` 批准（规则详见 [Manifest](../host/manifest.md)）
- **所有 DNS 答案均检查**，任一不合规即拒绝；合规地址经 reqwest `resolve_to_addrs` 固定，TLS 仍校验证书与 URL hostname；发送后核对实际 `remote_addr`。loopback、link-local、multicast、IPv4 保留/文档/共享空间、IPv4 映射 IPv6 和 IPv6 过渡/特殊范围拒绝；只接受直接公开 IPv6 unicast 或明确授权 ULA，不把任意 `::/0` 当成特殊地址豁免
- 不使用共享 OJ 客户端、Cookie、系统代理、重定向或隐式重试；无法通过 URL 用户信息、危险 headers 或插件代理配置绕开策略；不注入 Hinina 凭证
- **请求头黑名单**：host / cookie / connection / upgrade / transfer-encoding / content-length / content-type 及 `proxy-*` 前缀；其中 content-type 由宿主按 body 形态独占决定（JSON body 强制 application/json），拒绝插件自带以避免 reqwest append 语义下发出重复矛盾头
- 清单与宿主批准的 origin 只接受域名，IPv4/IPv6 字面量均拒绝（含公网 IP）；运行时 DNS 解析/地址审查保持不变

### 限额与语义
- 请求/响应体各最多 64 KiB；总超时最多 5 秒（含 DNS、连接和流读取）；每插件 60 次/分钟、最多 2 个并行请求，超限立即失败
- 返回 UTF-8 body，响应头只含 content-type/retry-after；3xx 和 5xx 正常返回，不自动重试
- 作用域变更/停止会取消在途等待，发出前再校验。请求已到外部服务后不能撤销远端副作用——插件必须使用外部幂等键并设计恢复策略；HTTP 500 与网络超时由插件按自身业务处理，核心不重试派发
- HTTP 失败只在宿主日志记录阶段及类别，URL、认证头和响应正文不进入诊断
