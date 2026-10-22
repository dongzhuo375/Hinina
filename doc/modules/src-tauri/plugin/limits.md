# 插件宿主资源上限

plugin/limits.rs 是 Runtime、Manager 和查询桥共用的内部常量模块，不向插件开放可调策略。

激活10秒、事件30秒、启动等待为激活期限加2秒、停止5秒。启动截止边界采用fail closed：调用方未及时观察到ready，即使引擎刚完成激活也撤销并记录timeout。事件 handler 失败告警按 (plugin, code) 节流：首次立即输出，`HANDLER_WARN_INTERVAL`（60秒）内静默累计，到期合并输出一条并带累计计数，避免持续失败的 handler 按事件速率刷屏。

Provider单请求最多5秒；submission.get整体扫描最多8秒——单页最坏情形之上留出scope检查与后续快页余量，仍保持在激活期限前可捕获。Runtime的每次宿主调用另以当前入口硬截止前1秒为截止点，及时返回可捕获的timeout，为JS的catch/finally保留处理时间；晚发起的调用也不能越过入口窗口。死循环或继续执行超出硬期限仍使worker失败；单个事件 handler 的异常被隔离，不会让 worker 失败。

桥响应含JSON包络最多1MiB；单native提交投影页最多1MiB；返回给JS的整页另按`MAX_CURSOR_RESPONSE_BYTES`（1MiB减256字节包络余量）校验后才发布游标；每次激活全部保留游标余项的JSON字节合计最多4MiB，同时保留原有32游标/1000条native页上限。这些字节预算不等于整个进程的RSS上限。超限统一返回limit_exceeded。
