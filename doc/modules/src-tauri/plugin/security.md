# 插件安全边界与错误处理

当前可执行安全边界是 Manifest 验证、PluginGrant、事件白名单/裁剪及有界交付。下述网络/文件/运行时安全为未来实现要求，不是当前功能宣称。

## 能力边界

未来 JS 只接触 PluginEventEnvelope、独立 API DTO 和宿主签发查询句柄；不注入 CoreEvent、broadcast Receiver、AppContext、Service、Provider、Session、Token 或内部实体。JS 错误对象无宿主内部堆栈；Host 日志也不输出任意异常文本。恶意 native Rust 代码不在此 JS 沙箱模型内。

事件是已发生事实，不能替代保存配置/工作区、持久化 Session、清缓存或切 OJ 的显式核心动作。Plugin API 仅做外围增强，插件失效不改变业务正确性。

当前提交事件缺少事实所属 OJ/账号/比赛，故只能视为刷新提醒。未来上下文在每次异步查询前后和副作用请求前检查，切换失效后重扫；进行中的已发 HTTP 不承诺远端回滚。外部派发依赖业务键/幂等和重新确认，不依赖事件 ID 直接查询。

## 网络执行器后续要求

独立创建不带 OJ Cookie/Token 的 reqwest Client；禁止复用当前启用 cookie_store 的 infra HttpClient。仅 HTTPS:443，拒绝 URL userinfo、fragment、IP literal、localhost/私网/链路本地/回环/保留地址和非全局 IPv6。精确 allowHosts，禁通配符；不能只验证字符串，需要验证 DNS 的全部候选地址并绑定连接到已验证 IP，防 DNS rebinding。禁自动重定向和系统/插件代理；若未来允许重定向，每跳重新授权与解析。

请求/响应流读取前及过程中执行体积限制（两者 <=64 KiB，Manifest 可收紧请求）、连接/总超时 <=5s、每插件 60 次/分钟限制；不自动注入 Session，不公开 Set-Cookie，不接受代理/连接控制头。非 2xx 返回状态，不自动重试；插件自行使用外部幂等键和有限退避策略。当前 validate_shape 只检查形状和体积，不判断 URL/DNS 安全。

## 存储与界面后续要求

私有命名空间绑定授予实例，不接受真实路径或 JS 指定插件 ID。键只作数据编码，验证 canonical 目录边界、原子持久化、1 MiB 总配额、64 KiB 单值上限和并发 CAS；停止保留数据。渲染通知只使用文本节点，不使用 innerHTML；限制 512 UTF-8 字节。Runtime 资源预算和中断见运行时规范。

## 日志

插件日志统一包含 plugin_id、plugin_version、runtime、event、operation、permission、request_id；不适用字段用空字符串，runtime=js。事件名使用协议点分名。Host 注册/注销/队列关闭已经遵循这些字段；纯验证函数不主动记录输入。API 实际执行日志留待 Adapter 实现。

禁止 token/password/authorization、源码、完整请求/响应体、绝对路径和用户隐私字段。不记录任意 Provider/JS 错误原文。内部 AppError 只经 redact_app_error 分类；JS Promise 仅返回稳定 PluginError。

## 必须后续补齐的执行测试

私有存储隔离/CAS/重启持久化、HTTPS/白名单/DNS/IP/IPv6/redirect/proxy/超时/流大小/限流、通知纯文本渲染、JS 死循环中断/内存/Promise/异常、气球业务的失败重试/重启/幂等。当前没有这些执行器，因此不提供虚假端到端通过结果。
