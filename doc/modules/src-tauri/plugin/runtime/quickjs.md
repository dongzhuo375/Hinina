# runtime/quickjs

## 职责
`PluginRuntime` 端口的真实实现：单 worker 独占的 QuickJS（rquickjs）引擎适配——受限执行窗口、宿主函数桥、错误分类与引擎生命周期。

## 核心类型/函数
- **`QuickJsRuntime`** — `new(stopped)` / `load(source, api, config, allowed)`（activate） / `deliver(envelope)`（事件投递） / `shutdown()`；内部持有 AsyncContext/AsyncRuntime、deadline、拒绝跟踪、按 code 节流的告警状态
- **`DeliveryOutcome`** — `{failed_handlers}`，由 `deliver` 回传 Manager
- （内部）`Rejections`（有界身份摘要）、`HandlerWarning`（节流状态）、`engine_error`（rquickjs 错误映射）

## 直接依赖
- `plugin::api`、`plugin::execution_api::call_json`、`plugin::limits`（全部期限常量）
- `rquickjs`（AsyncContext/AsyncRuntime/Module/Promise/Persistent/Func）、`tokio_util::sync::CancellationToken`
- `runtime/bootstrap.js`（`include_str!` 内嵌）

## 被依赖
- `plugin::manager`（经 `PluginRuntime` 端口持有）

## 逻辑流程
### 执行窗口与桥
- 宿主只在 activate/事件 handler 的受限执行窗口内驱动调度器；宿主 API Promise 必须由该入口 await 或纳入返回链。入口结束后不为 fire-and-forget 调用 drain 调度器；该限制是开发契约，拒绝诊断不检测"忘记 await"
- `bootstrap.js` 只向插件提供冻结 API，host JSON bridge 引用保持私有；每批最多 16 个在途桥调用。bootstrap 在调用 host 前捕获 JSON.stringify 失败（循环引用、BigInt、抛异常的 toJSON 等），统一拒绝冻结的 `{code:"invalid_input"}`，不透传原始异常消息
- 每次宿主调用绑定**发起时所在入口**的硬截止时间，并提前 1 秒返回稳定 timeout，让 JS 可以 catch/finally；`submission.get` 另有整体 8 秒扫描上限。捕获错误且在剩余窗口内完成不会使 worker 失败；死循环和超过入口硬期限仍按原规则失败。取消/丢弃查询归还并发许可并清理未发布或内部游标；已发出的外部副作用无法撤回

### 期限与资源
- ES module 加载与 activate 最长 10 秒，单事件交付最长 30 秒，内存 16 MiB、栈 256 KiB；引擎 interrupt 配合异步超时覆盖同步死循环和悬挂 Promise。期限统一引用 plugin/limits，Manager 启动等待由激活期限加宽限计算
- 停止时撤销 API、移除订阅、清理 Persistent 句柄并 shutdown；`Drop` 也先释放 Persistent 句柄——覆盖 worker panic 未调用 shutdown 的展开路径，避免 rquickjs 因句柄晚于 runtime 析构而 abort 进程

### 事件投递与错误分类
- 按 handler 隔离：bootstrap 逐个 await 并捕获，返回 `{failed, code}` 摘要（code 受限 `^[a-z][a-z_]{0,39}$`——首字符不许下划线，不可注入日志字段但仍可能由插件提供，按不可信诊断提示处理；非宿主形状一律记为 script_error）。宿主按 code 节流输出脱敏 warn（首次立即、60 秒内累计、到期合并带累计计数）后继续后续 handler 与事件——只有引擎级失败（超时、内存、撤销）才结束 worker
- 引擎 Allocation 错误映射 `limit_exceeded`
- host promise rejection tracker 仅记录 Promise 身份摘要（最多 256 项及溢出标志），同一窗口补装 handler 会移除记录；load/deliver 结束及 shutdown 汇总输出 promise.rejection/unhandled_rejection 警告和计数后清空——不保存或格式化 reason、不调用 JS toString、不输出栈或秘密。未被调度的调用可能尚未产生拒绝，该诊断不保证发现所有游离调用
- 错误码：语法/脚本异常 → `script_error`；截止到达 → `timeout`；停止/作用域撤销 → `context_expired`；初始化或引擎基础设施不可用 → `unavailable`。日志只保留稳定类别，不输出异常消息、栈、源码或路径
