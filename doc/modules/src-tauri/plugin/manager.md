# plugin/manager

## 职责
`PluginManager`——管理 enabled 且获批准的插件：启动、停止、重载、状态查询、手动/定期 Resync。启动/停止/重载串行化；每插件独立线程与 QuickJS 实例；AppContext 只持 Send 控制器，JS 值留在 worker。状态仅 starting/running/stopped/failed，不建复杂生命周期状态机。

## 核心类型/函数
- **`PluginStatus`** — 列表视图：插件 ID、状态、失败诊断等稳定字段
- **`PluginManager`**：
  - `new(repository, factory, host, source, access)` — 组合根注入五项：文件仓库、API 工厂、PluginHost、QuerySource、AccessRevision；**共享 Provider 预算在 new() 内部按 limits 常量构造**（`SHARED_QUERIES_PER_MINUTE`/`SHARED_QUERY_CONCURRENCY`），Resync 间隔来自每个插件 approval 的 `resyncIntervalSecs`（测试可用 override 覆盖），二者都不是注入项
  - `set_output(Arc<dyn NotificationOutput>)` — 通知出口
  - `list() -> PluginResult<Vec<PluginStatus>>` — 状态汇总（语义见下）
  - `start(id)` / `stop(id)` / `reload(id)` — 生命周期命令（串行化）
  - `resync(id)` — 手动 Resync 提示
  - `autostart()` — 按 enabled 配置批量启动
  - `cancel_all()` — 应用退出：同步撤销，不等待插件、不干预工作区保存
- （内部）worker 线程、reaper、每代次状态与历史诊断

## 直接依赖
- `plugin::registry::PluginRepository`（配置/清单/源码端口）、`plugin::runtime`（`PluginRuntime` 端口）、`plugin::limits`、`plugin::permission::PluginGrant`
- `core::access::AccessRevision`、`core::event`（经 PluginHost 消费）
- `tokio_util::sync::CancellationToken`、`serde::Serialize`、`tracing`

## 被依赖
- `core::context`（AppContext 持有）、`commands/plugin_cmd.rs`

## 逻辑流程
### 订阅与 Resync
- 仅实际获批 `events.subscribe` 的插件建立事件队列、开放事件回调并接收初始/周期 Resync（默认 60 秒）；没有该授权也可正常激活、停止、重载，手动 resync 因无订阅返回 Unavailable。Host 订阅的 PermissionDenied 映射保留权限语义
- 激活期间没有 Host 订阅：成功后才建立队列并发送初始 Resync，激活期间的业务事件由重查补偿
- 手动和周期 Resync 每次都尝试入队，成功即表示提示实际入队；队列满或关闭沿用注销订阅并返回 Unavailable 的语义
- 事件 handler 失败（`DeliveryOutcome.failed_handlers > 0`）主动入队一次 Resync 重新对账；Resync 通知自身失败不再触发（防失败循环），周期 tick 兜底
- Manager 与 Host 都用 `PluginGrant.allowed_events` 生成权限过滤后的 allowlist，且只附加一次 resync.required

### 启动与失败诊断
- 启动前的清单、配置、授权或 worker 创建失败统一记录 `worker.start` 警告：只含稳定错误与插件标识，不含源文件路径、配置内容或错误栈。重复 start 只拒绝命令，不写 failed 历史
- 启动 12 秒超时立即标记 failed/timeout（**ready 与期限同时到达采用 fail closed**），撤销后等待 worker 退出并回收；无法在停止期限内清理时保持 failed 并记录清理警告
- worker 在发布 ready 结果**之前**快照取消标志：`stop()` 在宿主观察到就绪后立即取消令牌，若之后才读取会把真实激活失败误判为主动停止（状态被改写为 stopped、丢失失败诊断）；测试用 post-ready 钩子在该边界精确取消令牌锁定此行为
- 历史诊断按最近写入保留最多 128 个 ID（长度最多 128 字节）；list 读取时清除已从配置移除的历史记录，活动 worker 的状态另行覆盖；配置仍存在时回收后继续显示 timeout 历史

### 停止与回收
- stop 撤销权限、注销订阅、取消 API 并等待 worker 释放；reload 不能与旧 worker 并存——旧 worker 未结束期间拒绝同 ID 重启
- 正常停止最多等 5 秒，未退出报 timeout 并启动 reaper；stop_inner 超时与 start 超时一致（撤销、标 failed/timeout、启动 reaper）。每代次在锁内标记 reaping，重复 stop 超时只复用现有回收任务
- 原生阻塞延迟退出时，异步 reaper 跟踪同一 worker 代次，线程完成后回收；轮询间隔从 100ms 指数退避到 2s 上限，原生调用永不返回也不会形成高频轮询。迟到退出不能覆盖已锁定的 failed/timeout 诊断；start_worker 替换已结束的 worker 前先把上一代最终状态写入历史，避免与 reaper 竞争时丢失失败诊断
- worker 线程用 `catch_unwind` 隔离插件触发的 panic：记录 `worker.panic` 警告（只含 panic 类型 str/string/other，不格式化 payload——插件可控文本不进结构化日志），该次运行视为 unavailable 失败；**注意全局 panic 钩子先于捕获运行**，同一次 panic 仍会带 payload 写入本地崩溃文件/日志（见 `infra/panic_report.md` 的策略交叉说明），该文件可由用户随「日志与崩溃记录」清理；`QuickJsRuntime` 的 Drop 兜底先释放 Persistent 句柄，避免 rquickjs 在引擎析构后 abort 整个进程
- 日志：失败退出 warn 带稳定错误，正常退出 info，停止超时 warn；外部字符串按 Debug 转义（operation 字段用固定枚举名，不透传插件输入；64 字符截断发生在 `execution_api` 的 `call_json` 审计日志，见 execution_api.md）

### 资源与注入
- Manager 持有跨插件、跨重载的 Provider 查询总预算（120 次/分钟、并发 4），与每次激活的独立预算一起限制插件借宿主凭据访问 OJ，详见 [查询边界](query.md)
- 仓库与 `PluginApiFactory` 由组合根注入，Manager 不直接构造文件仓库、存储或 HTTP
