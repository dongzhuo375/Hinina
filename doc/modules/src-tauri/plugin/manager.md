# PluginManager

管理enabled且获批准的插件；启动/停止/重载串行化，每插件独立线程与QuickJS实例，AppContext只持Send控制器。只保留starting/running/stopped/failed，避免复杂生命周期状态机。

仅实际获批 `events.subscribe` 的插件建立事件队列、开放事件回调并接收初始/周期 Resync（默认60秒）；没有该授权也可正常激活、停止和重载，手动 resync 因无订阅返回 Unavailable。Host 订阅的 PermissionDenied 映射保留权限语义。stop撤销权限、注销订阅、取消API并等待worker释放，reload不能与旧worker并存。退出同步撤销，不干预工作区保存；详见[Runtime](runtime/mod.md)。

`list()` 返回 `PluginResult<Vec<PluginStatus>>`：缺失 `plugins.json` 仍代表没有配置插件，文件损坏或配置验证失败则返回安全错误，不能伪装为空列表。显式启动、自动启动和重载的启动阶段失败均保留 `failed` 与稳定错误码；成功重启后的 worker 状态覆盖旧诊断。

历史诊断按最近写入保留最多 128 个 ID（长度最多 128 字节），列表读取时清除已从配置移除的历史记录；活动 worker 的状态另行覆盖。启动前的清单、配置、授权或 worker 创建失败统一记录 `worker.start` 警告，只含稳定错误与插件标识，不含源文件路径、配置内容或错误栈。

Manager 持有跨插件、跨重载的 Provider 查询总预算；与每次激活的独立预算一起限制插件借宿主凭据访问 OJ，详见[查询边界](query.md)。手动和周期 Resync 每次都尝试入队，成功即表示提示实际入队；队列满或关闭沿用注销订阅并返回 Unavailable 的语义。事件 handler 失败时（`DeliveryOutcome.failed_handlers > 0`）主动入队一次 Resync 重新对账，避免插件带发散状态运行到下次周期 Resync；Resync 通知自身失败不再触发，防止失败循环，周期 tick 仍兜底。

仓库与 PluginApiFactory 由组合根注入，Manager 不直接构造文件仓库、存储或 HTTP。激活期间没有 Host 订阅，成功后才建立队列并发送初始 Resync；激活期间的业务事件由重查补偿。启动12秒超时立即标记 failed/timeout，撤销后等待 worker 退出并回收；无法在停止期限内清理时保持 failed，并记录清理警告。重复 start 只拒绝命令，不写 failed 历史。失败退出用 warn 并带稳定错误，正常退出用 info，停止5秒超时同样记录 warn；外部字符串按 Debug 转义，operation 日志限制64字符。

原生阻塞延迟退出时，异步reaper继续跟踪同一worker代次，线程完成后回收；轮询间隔从100ms指数退避到2s上限，原生调用永不返回也不会形成高频轮询。旧worker尚未结束期间拒绝同ID重启，不能与新worker并存。迟到退出也不能覆盖已锁定的failed/timeout诊断。

worker线程用`catch_unwind`隔离插件触发的panic：捕获后记录`worker.panic`警告（只含panic类型str/string/other，不格式化payload），并把该次运行视为unavailable失败；`QuickJsRuntime`的Drop兜底先释放Persistent句柄，避免rquickjs在引擎析构后abort整个进程。start_worker替换已结束的worker前先把上一代最终状态写入历史，避免与reaper竞争时丢失失败诊断。

stop_inner超时与start超时一致：撤销权限与订阅、标记failed/timeout、启动reaper。线程未结束仍拒绝同ID启动；reaper按同一代次保存最终历史再回收，避免后台退出覆盖新代次状态。配置仍存在时list在回收后继续显示timeout历史。

worker 在发布 ready 结果**之前**快照取消标志：`stop()` 在宿主观察到就绪后立即取消令牌，若之后才读取，会把真实的激活失败误判为主动停止，状态被改写为 stopped 并丢失失败诊断。测试通过 post-ready 钩子在该边界精确取消令牌来锁定此行为。

每个worker代次在锁内标记reaping，重复stop超时只复用现有回收任务。启动ready与期限同时到达时采用fail closed并保留timeout；统一期限见[limits](limits.md)。Manager与Host都使用PluginGrant.allowed_events生成权限过滤后的allowlist，并只附加一次resync.required。
