# 受控 HTTP

`http.request(context,{method,url,headers?,body?,timeoutMs?})` 需 network.request，并同时满足清单和 plugins.json 批准的精确 origin。默认只允许公开单播地址上的 HTTPS；HTTP 需 allowHttp=true，RFC1918/ULA 需额外 privateCidrs 批准。规则详见 [Manifest](../host/manifest.md)。

所有 DNS 答案均检查，任一不合规即拒绝；合规地址通过 reqwest.resolve_to_addrs 固定，TLS 仍校验证书与 URL hostname。检查实际 remote_addr。不使用共享 OJ 客户端、Cookie、系统代理、重定向或隐式重试。无法通过 URL 用户信息、危险 headers 或插件代理配置绕开策略；不注入 Hinina 凭证。请求头黑名单为 host/cookie/connection/upgrade/transfer-encoding/content-length/content-type 及 proxy-* 前缀；其中 content-type 由宿主按 body 形态独占决定（JSON body 强制 application/json），拒绝插件自带以避免 reqwest append 语义下发出重复矛盾头。

loopback、link-local、multicast、IPv4 保留/文档/共享空间、IPv4映射IPv6和IPv6过渡/特殊范围拒绝。只接受直接公开 IPv6 unicast 或明确授权 ULA；不把任意 `::/0` 当成特殊地址豁免。

请求/响应体各最多64KiB；可由 Manifest 收紧请求体。总超时最多5秒，包含DNS、连接和流读取；每插件60次/分钟，最多2个并行请求；超限立即失败。返回 UTF-8 body，响应头只含 content-type/retry-after；3xx和5xx正常返回，不自动重试。

作用域变更/停止会取消在途等待，发出前再校验。请求已到外部服务后不能撤销远端副作用，插件必须使用外部幂等键并设计恢复策略。HTTP500与网络超时应由插件按照自身业务处理，核心不重试派发。

清单与宿主批准的origin只接受域名，IPv4/IPv6字面量均拒绝（含公网IP）。运行时DNS解析/地址审查保持不变。HTTP失败只在宿主日志记录阶段及类别，URL、认证头和响应正文不进入诊断。
