# 插件安全模型

> 概念文档（无对应源文件）；各机制的实现细节见对应模块文档。

## 信任边界

- **JS 只取得冻结的 hinina API**：不暴露 AppContext、Service、Provider、CoreEvent、Receiver、Session、Token、源代码、任意路径或原生句柄。Runtime 内部 API Adapter 可调用只读 Service，JS 永远不持有 Service 对象
- **进程内脚本隔离**：依赖引擎（rquickjs）和 Rust/native 依赖的正确性，**不是独立 OS 进程沙箱**；native 内存安全缺陷或进程崩溃仍可能影响应用。代码/授权/数据目录由本机管理员维护，不向恶意本地管理员提供隔离承诺
- 采用成熟实现处理复杂边界：rquickjs 限制执行、semver 验证版本、reqwest/url 负责协议与 TLS、ipnet/ip_network 处理地址、governor 限流、fs4 锁与 atomicwrites 持久化

## 能力授予

- 清单声明和代码目录**不授予能力**：宿主独立配置权限、精确网络 origin 和私网地址（见 [Manifest](host/manifest.md)）；每次调用重新检查授权，旧 worker 停止即撤销
- 作用域以真实 Provider endpoint 和稳定用户绑定，显式 revision 防止 ABA 及在途切换（见 [query](query.md)）；不依赖有损事件完成安全操作
- 网络无任意代理/跳转/重试、无 Cookie、无凭证注入；origin 必须为域名，IP 字面量在清单阶段即拒绝（见 [受控 HTTP](api/http.md)）
- 四个访问变更组件强制构造注入共享栅栏（AccessRevision）

## 资源预算

- 只读 OJ 查询不能借宿主凭据无限访问 Provider：单次激活 60 次/分钟且并发 2，Manager 全部插件合计 120 次/分钟且并发 4；令牌桶允许相应初始突发，共享预算跨重载保留。按每个实际 Provider 请求计费（分页扫描不能只按一次 JS 调用计费）；单请求 5 秒超时，超限立即拒绝。会话身份文件读取通过阻塞线程池执行，前后继续验证作用域撤销
- 桥响应含包络最多 1 MiB、单 native 投影页最多 1 MiB、保留游标余项合计 4 MiB——这些是 JSON 字节预算，仍需与引擎 16 MiB 及并发/条数上限共同约束；引擎分配失败返回 `limit_exceeded`

## 日志与诊断

- 日志字段统一 plugin_id / plugin_version / runtime / event / operation / permission / request_id；API debug 日志仅记操作、成功与请求 ID，worker 记录状态。唯一例外是 `operation="plugin.log"` 行（契约见 [contracts.md §16](../../../plugin/contracts.md)）：额外携带 `level` 与经校验（≤1024B、拒控制字符）、换行/制表拍平且 Debug 转义的插件 `message`，是唯一允许进入日志的插件自由文本
- 禁止记录 token/password/authorization/source_code、完整请求/响应体、绝对路径、用户隐私或 JS 错误栈；普通 Service 日志沿用原项目策略，插件不接收这些日志
- 基础设施诊断统一记录安全操作名、IO ErrorKind、serde 分类或 HTTP 错误类别，禁止源错误字符串/URL/路径/配置和通知正文；非法 plugin_id 替换占位符
- Promise 拒绝跟踪仅保留有界身份摘要并输出计数，不访问或格式化 JS reason；所有宿主 Promise 仍必须 await 或加入入口返回链

## 业务边界（宿主不做的事）

未完成的业务由插件负责：历史完整性与跨页变化处理、首次 AC 规则、幂等键、可恢复失败状态、外部重试与服务端去重。本地 CAS 不能提供远端 exactly-once。
