# PluginEvent / PluginEventEnvelope

当前事件协议版本 **2**，旧 snake_case type 不兼容。Manifest 和 Host API 当前均为版本 2。

```json
{"version":2,"sequence":2,"occurredAt":1780000000000,"event":{"type":"submission.judged","submissionId":"s1","status":"Accepted"}}
```

sequence 在同一订阅内从 1 起，未通过白名单/声明/权限的事件不消耗序号。重新订阅从 1 开始；不能跨重启比较。occurredAt 为转换时刻，不是提交或终态实际发生时刻。事件可能重复，不承诺 exactly-once。

| type | 载荷 | events.subscribe 之外的载荷权限 |
|---|---|---|
| system.oj_switched | ojId | 无 |
| contest.selected | contestId | contest.read |
| announcement.changed | contestId, newCount | contest.read |
| problem.opened | contestId, problemId | contest.read |
| submission.created | submissionId | submission.read |
| submission.judged | submissionId, status | submission.read |
| workspace.saved | workspaceId, revision, automatic | 无，仅元数据，无读写能力 |
| system.config_changed / system.theme_changed | 无 | 无 |
| session.logged_in / session.logged_out / session.expired | ojId | 无，不含用户标识 |
| resync.required | lost | 强制控制通知，无需声明 |

TokenRotated 不在协议内。事件不能承载源码、token、认证头、内部路径、Session、原始 Provider 数据或内部错误。

## Resync

默认60秒周期与手动Resync会发lost=0，账号/OJ/比赛变化也提示重查；首次订阅必有 lost=0；消费者 Startup/重启可能再提示一次。Lagged(n) 提示 lost=n，这个数字统计底层核心事件，而非已订阅事件或漏掉的 AC 数量。提示不承诺补发；查询当前状态才是真值。应用重启和订阅前的历史不会进入队列。

慢插件队列满时直接关闭订阅，不能向已经满的队列承诺一定补一条 Resync。worker 发现关闭后停止；由宿主显式重载时先处理初始 Resync；不要忙循环自动重启。

提交终态查询可多次产生相同 SubmissionJudged。该事件没有 OJ/用户/比赛作用域，只能触发刷新/重新扫描当前上下文。插件不得直接用此 ID 触发派发等副作用，需先由安全查询验证身份/比赛/历史；上下文切换和并发重判仍需重新确认。

JS on/off、多个 handler 串行及超时是 [Runtime 契约](../runtime/mod.md)，当前由 QuickJsRuntime 真实执行；handler 接收 (event, envelope)。单个 handler 抛出的异常不会中断同事件的其他 handler 或后续事件，宿主只记录脱敏失败计数与稳定错误码。
