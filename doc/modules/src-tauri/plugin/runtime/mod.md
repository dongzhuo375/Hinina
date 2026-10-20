# Plugin Runtime 接入契约

当前只有 PluginRuntime trait 和 PluginRuntimeBridge，无 JS 引擎、模块加载或 handler 执行。测试中的 LocalRuntime 是验证 !Send 可接入的测试替身，不是 JS 支持。没有直接执行 TypeScript 的承诺。

## 最小接口与顺序

PluginRuntime 使用 async_trait(?Send)，允许引擎由专属 OS 线程或 Tokio LocalSet 持有。异步 I/O 仍使用宿主 Tokio，不新增第二套异步 runtime。PluginRuntimeBridge 只携带 Arc<dyn PluginApi> 和插件信封有界 Receiver。

未来入口规范是离线打包的 JS ES module，默认导出 async activate(hinina)，不做 CommonJS/TS 编译或任意模块解析。顺序：验证清单/入口 → 宿主批准权限 → 创建独立引擎/API Adapter → Host subscribe（初始 Resync 已入队）→ activate 注册 handler → 串行消费队列。activate 失败注销队列并 shutdown；先完成 handler 注册再消费初始 Resync。

- activate(source, api)：加载入口并注册监听；失败只影响此插件。
- deliver_event(envelope)：在同一插件按注册顺序 await handler，多个 handler 共享同一串行序列。单 handler 抛错/超时记安全诊断后停止该插件，不终止 Host。
- shutdown()：幂等释放引擎/handler/取消在途请求，停止后不再触发 handler；不删除私有存储。

停止由宿主显式 unsubscribe → 关闭/丢弃接收端缓存 → shutdown → 释放引擎。队列满关闭后 worker 同样停止，重新接入由未来宿主明确发起，首条 Resync；不自动无限重启。无需 Discovered/Validated/Loaded/Suspended 等完整状态机。

JS events.on 支持多个 handler 并返回注销函数；stop 清空监听器。future worker 必须用引擎中断预算（每 handler/activate 默认 1s）、内存上限（初始目标 16 MiB）和有界异步在途调用，不能仅用 Tokio timeout 包住同步 JS，后者无法打断死循环。

## 引擎评估（尚未做依赖构建或体积测量）

| 候选 | 与当前项目的潜在契合 | 落地前必须实测 |
|---|---|---|
| rquickjs | QuickJS 能力小、显式注入，适合有限外围 API | Windows C 工具链、Promise/Tokio 桥、线程归属、interrupt/memory limit、打包体积 |
| boa_engine | Rust 实现，避免 QuickJS C 源码构建路径 | 所选版本的异步 job queue、超时中断、JS 特性、内存限制和实际体积 |
| deno_core | V8 isolate/异步资源机制可供更丰富能力 | V8 预编译分发/下载、Windows 构建与包体成本、op 权限封装、终止和隔离 |

不提前选定依赖版本；先用真实 Windows/Tauri/Rust 工具链验证上述项再决定。当前简单需求优先评估 rquickjs/Boa；deno_core 的更大集成面只有在前两者不能满足时再考虑。此比较是待验证的工程取舍，不是性能或安全基准。

引擎不得默认暴露 fs、child_process、process.env、任意 fetch/socket/native module/shell；只注入 hinina 能力。真正的 JS 中断、Promise 与异常隔离必须由后续执行器测试验证。
