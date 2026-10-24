# runtime/mod

## 职责
插件执行引擎端口（`PluginRuntime`）与选型说明：Manager 只面向该端口调用，QuickJS 实现见 [quickjs.md](quickjs.md)。

## 核心类型/函数
- **`PluginRuntime`**（trait，生产路径实际使用）— Manager 以 `Box<dyn PluginRuntime>` 持有引擎，只调用：
  - `activate(source, api, config, allowed_events)` — 加载 ES module 并执行 activate
  - `deliver_event(envelope) -> DeliveryOutcome` — 投递事件；`DeliveryOutcome.failed_handlers` 回传失败 handler 数，供 Manager 决定是否触发 Resync 对账
  - `shutdown()` — 停止清理
  - 测试用的 !Send 实现与 QuickJS 实现共用同一契约；事件信封由 PluginHost 订阅产生，**不存在额外的 Runtime Bridge 装配类型**

## 直接依赖
- `plugin::api`（PluginApi 注入、事件信封、错误）
- `async_trait`、`std::sync::Arc`

## 被依赖
- `plugin::manager`（唯一消费者）、`runtime/quickjs.rs`（生产实现）

## 逻辑流程
### 选型
- rquickjs 0.14（QuickJS-NG），仅 std/futures features；不启用 dyn-load/full/custom allocator。与 boa/deno_core 相比，本轮选型重视已有 Promise 桥、引擎中断/内存限制和较小依赖；Windows/MSVC 真实引擎测试通过后才装配
- rquickjs 构建时用 C 编译器（MSVC/gcc/clang）编译内置 QuickJS 源码——构建环境需要 C 编译器（README 环境一节已列前提，该依赖及其构建前提已由仓库所有者确认）。Linux/macOS 的运行时构建与测试尚未验证；未做三种引擎的体积性能基准

### 执行模型
- 入口是离线打包 ES module：`export default async function activate(hinina, config)`。源码 ≤1 MiB，配置 JSON ≤64 KiB 且深冻结；宿主无 resolver/loader——不支持任意 import、原生模块、fs/process/env/fetch/socket/shell；TS 在宿主外经 esbuild 等构建，宿主不执行 TS 源文件
- **异步契约**：所有 Promise 返回的宿主 API 必须 await，或加入 activate/handler 返回的 Promise 链（并发用 await Promise.all）；入口结束后不 drain 游离调用、不保证它们执行或报告拒绝；events.on/注销函数同步执行
- 每插件独立引擎和 OS worker 线程，由持续存在的 Tauri Tokio handle 驱动（没有第二个 Tokio runtime）；内存 16 MiB、栈 256 KiB、activate 10 秒、单事件交付总计 30 秒。同步 JS 由引擎 interrupt 中断，未完成 Promise 由取消和时间上界终止；异常/超时/内存失败停止该插件，其他插件和核心继续
- 同一插件最多 64 个 handlers，按注册顺序串行 await；on 返回注销函数；一次事件的 30 秒预算包含全部 handlers。单个 handler 的未捕获异常只终止该 handler 本次调用——宿主记录脱敏计数与稳定错误码（`event.handler`/`handler_failed`），其余 handler 与后续事件继续、worker 不因此失败；失败告警按 code 节流（首次立即、60 秒内累计、到期合并一条带累计计数）。Manager 据此主动入队一次 Resync 对账（Resync 通知自身不再触发，避免失败循环，周期 tick 兜底）；插件仍应自行 catch 做局部恢复
- 所有 API 经有限 JSON 桥（最多 16 个在途调用）；错误是安全代码、无内部栈。开发调试靠状态和宿主日志——没有 DevTools 逐步调试器或 source map 栈输出

### 生命周期
- 状态仅 starting/running/stopped/failed；无安装器、暂停/恢复、热更新或复杂状态机。start/reload 重新读取宿主配置并校验文件，单插件启动失败不阻止应用启动；reload 等待旧 worker 真正退出再创建新实例
- 停止顺序：撤销本次授权/取消调用 → 注销 Host 订阅 → 放弃缓存事件/等待 Promise → 清理 handlers 并释放引擎。正常 stop 最多等 5 秒，未退出报 timeout 并阻止并存重载；应用退出只做同步撤销、不等待插件、不干预工作区保存关窗流程。文件系统内核调用不能强制回滚，已发送 HTTP 不能撤销
- 周期 Resync 默认 60 秒、可配置 10..3600 秒；每次成功代表提示实际入队，已有业务事件不会令提示被静默跳过。队列容量 128，满/关闭时注销并令 worker 失败退出。首次/Lagged/账号/OJ/比赛事件也触发 Resync——它只是重新查询提示，不代表补发，更不负责气球重试；安全性由显式 AccessRevision 保证，不依赖这些通知
- activate 成功后才建立事件队列，以初始 Resync 补偿激活窗口变化。语法/运行异常为 `script_error`，基础设施为 `unavailable`；超时/撤销保留 `timeout`/`context_expired`（详见 [QuickJS](quickjs.md)）
- 停止超时保持 failed/timeout，继续跟踪旧 worker，完成后由 reaper 回收并保存历史（见 [manager](../manager.md)）
