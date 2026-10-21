# QuickJS 运行时

宿主只在 activate/事件 handler 的受限执行窗口内驱动调度器；所有宿主 API Promise 必须由该入口 await 或纳入返回的 Promise 链。入口结束后不会为 fire-and-forget 调用 drain 调度器，不承诺执行未完成调用。该限制是开发契约，拒绝诊断不检测“忘记await”；events.on 的注册/注销为同步操作。

`QuickJsRuntime` 由单个独立 worker 独占，使用应用 Tokio handle 驱动异步 Promise；不把 JS 引擎、值或原生函数放入 AppContext。`bootstrap.js` 只向插件提供冻结 API，host JSON bridge 引用保持私有；每批最多16个在途桥调用。

ES module 加载与 activate 最长10秒，单事件交付最长30秒，内存16MiB、栈256KiB。引擎 interrupt 配合异步超时覆盖同步死循环和悬挂 Promise。停止时撤销 API、移除订阅、清理 Persistent 句柄并 shutdown。

引擎Allocation错误映射limit_exceeded。期限统一引用plugin/limits，Manager启动等待由激活期限加宽限计算。

host promise rejection tracker仅记录Promise身份的摘要（最多256项及溢出标志），同一窗口补装handler会移除记录。load/deliver结束及shutdown汇总输出promise.rejection/unhandled_rejection警告和计数，随后清空；不保存或格式化reason，不调用JS toString，不输出栈或秘密。未被调度的调用可能尚未产生拒绝，因此该诊断不保证发现所有游离调用。

语法/脚本异常返回稳定 `script_error`，截止时间到达仍为 `timeout`，停止/作用域撤销为 `context_expired`，初始化或引擎基础设施不可用为 `unavailable`。日志只保留稳定类别，不输出异常消息、栈、源码或路径。Manager 激活成功后才注册事件队列，初始 Resync 补偿激活期间状态变化。
