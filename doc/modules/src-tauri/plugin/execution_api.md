# plugin/execution_api

ExecutionApi把ScopedQueries、插件私有JSON、独立HTTP和文本NotificationOutput装配为PluginApi。每种能力重新检查授权；call_json接受有限operation和JSON，返回稳定白名单错误，不透传内部实体或栈。

Runtime最多16个在途桥调用。API日志只包含固定身份/操作/权限/成功/请求ID，不记录参数或数据。参见[API](api/mod.md)。

桥输出含成功包络最多1MiB，DTO与最终包络分别经流式编码检查；超限输出小型limit_exceeded错误，审计success为false。compareAndSet审计同时标记storage.read和storage.write，operation使用Debug转义并截断；回归验证日志不含存储值。
