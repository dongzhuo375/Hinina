# 插件安全模型

JS只取得冻结的hinina API；不暴露AppContext、Service、Provider、CoreEvent、Receiver、Session、Token、源代码、任意路径或原生句柄。Runtime内部API Adapter可调用只读Service，JS永远不持有Service对象。

清单声明和代码目录不授予能力。宿主独立配置权限、精确网络origin和私网地址；每次调用重新检查授权，旧worker停止即撤销。作用域以真实Provider endpoint和稳定用户绑定，显式revision防止ABA及在途切换；不依赖有损事件完成安全操作。

采用成熟实现处理复杂边界：rquickjs限制执行、semver验证版本、reqwest/url负责协议与TLS、ipnet/ip_network处理地址、governor限流、fs4锁和atomicwrites持久化。没有网络任意代理/跳转/重试、Cookie或凭证注入。

这是进程内脚本隔离，依赖引擎和Rust/native依赖正确性，不是独立OS进程沙箱；native内存安全缺陷或进程崩溃仍可能影响应用。代码/授权/数据目录由本机管理员维护，不向恶意本地管理员提供隔离承诺。

日志字段统一 plugin_id/plugin_version/runtime/event/operation/permission/request_id；API debug日志仅记操作、成功与请求ID，worker记录状态。禁止记录token/password/authorization/source_code、完整请求/响应体、绝对路径、用户隐私或JS错误栈。普通Service日志沿用原项目策略，插件不接收这些日志。

未完成的业务由插件负责：历史完整性与跨页变化处理、首次AC规则、幂等键、可恢复失败状态、外部重试与服务端去重。本地CAS不能提供远端exactly-once。
