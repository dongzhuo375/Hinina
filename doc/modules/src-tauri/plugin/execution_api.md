# plugin/execution_api

ExecutionApi把ScopedQueries、插件私有存储、独立HTTP、文本NotificationOutput和宿主日志出口装配为PluginApi。存储与HTTP以 `PluginStorageApi`/`PluginHttpApi` 端口持有，具体实现由 `FsPluginApiFactory` 在组合根注入，本模块不导入 infra 类型；每次激活的实例是 `ExecutionApi::new` 的私有依赖组合，测试可直接替换端口。每种能力重新检查授权；call_json接受有限operation和JSON，返回稳定白名单错误，不透传内部实体或栈。

日志出口（`LogApi`）不需要权限：只写宿主自己的日志文件，不读宿主数据。写入前校验单条 ≤1024 字节并拍平换行/制表，消息以 Debug 转义落盘，行内字段固定为 plugin_id/plugin_version/runtime/operation/level/message；每激活 30 条/分钟，超限返回 limit_exceeded 而非静默丢弃。

Runtime最多16个在途桥调用。每次桥调用还须通过按激活的准入限流（600 次/分钟），覆盖不经过Provider预算的本地能力，防止脚本在context/identity/storage上空转；超限返回limit_exceeded且不进入分发。API日志只包含固定身份/操作/权限/成功/请求ID，不记录参数或数据。参见[API](api/mod.md)。

桥输入上限（140000字节输入、64字节operation名）与 `quickjs` 的无界检查共用 `plugin/limits` 常量，两处判定不会漂移。桥输出含成功包络最多1MiB，DTO与最终包络分别经流式编码检查；超限输出小型limit_exceeded错误，审计success为false。compareAndSet审计同时标记storage.read和storage.write，operation使用Debug转义并截断；回归验证日志不含存储值。
