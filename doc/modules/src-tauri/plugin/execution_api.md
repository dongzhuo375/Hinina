# plugin/execution_api

## 职责
把 ScopedQueries、插件私有存储、独立 HTTP、文本 NotificationOutput 和宿主日志出口装配为插件可见的 `PluginApi`，并实现有限 JSON 桥（`call_json`）：接受有限 operation 与 JSON 参数，返回稳定白名单错误，不透传内部实体或栈。

## 核心类型/函数
- **`ExecutionApi`** — 每次激活一个实例（`ExecutionApi::new` 的私有依赖组合），字段全部为端口：
  - `queries: Arc<ScopedQueries>`、`storage: Arc<dyn PluginStorageApi>`、`http: Arc<dyn PluginHttpApi>`、`output: Arc<dyn NotificationOutput>`——存储/HTTP/输出由组合根（`FsPluginApiFactory`）注入，本模块不导入 infra 类型，测试可直接替换端口
  - 三个限流器：`notifications`（10/min）、`logs`（30/min）、`calls`（600/min 准入）
- **`PluginApiFactory`** — 组合根注入端口（`create(queries, approval, output)`）
- **`NotificationOutput`** — 纯文本输出端口（`emit(plugin_id, level, message)`）；前端无输出必须显式失败
- `impl ContextApi / IdentityApi / ContestApi / SubmissionApi / PluginStorageApi / PluginHttpApi / NotificationApi / LogApi for ExecutionApi` — 每种能力转发前重新检查授权
- **`call_json(api, operation, input) -> String`** — 桥入口：输入限长 → 准入限流 → JSON 解析 → `dispatch` 分发 → 有界编码响应
- **`dispatch`（私有）** — operation → 方法映射表（context/identity/contest/submission/storage/http/notification/log.write）

## 直接依赖
- `plugin::api`（各能力 trait 与 DTO）
- `plugin::bounded_json`（DTO 与包络的有界编码）
- `plugin::host::manifest::PluginPermission`
- `plugin::limits`（`MAX_BRIDGE_INPUT_BYTES`、`MAX_BRIDGE_OPERATION_BYTES`、`MAX_BRIDGE_RESPONSE_BYTES`、`PLUGIN_LOGS_PER_MINUTE`）
- `plugin::query::ScopedQueries`
- `governor`（速率限流）、`serde_json`、`tracing`、`async_trait`

## 被依赖
- `plugin::runtime::quickjs`（宿主函数 `host(operation, input)` 的唯一实现方）
- `infra::plugin_api_factory`（组合根装配）

## 逻辑流程
- **日志出口（`LogApi::write`）**：不需要权限——只写宿主自己的日志文件，不读宿主数据。写入前按当前订阅者检查级别（`tracing::enabled!`），被过滤的调用直接成功且不扣额度；通过者校验单条 ≤1024 字节并扣 30 条/分钟独立预算，超限返回 `limit_exceeded` 而非静默丢弃。落盘前拍平换行/制表（无需拍平时借用原串），消息以 Debug 转义，行内字段固定为 plugin_id / plugin_version / runtime / operation / level / message
- **准入限流**：每次桥调用须通过按激活的 600 次/分钟准入（`admit`），覆盖不经过 Provider 预算的本地能力，防止脚本在 context/identity/storage 上空转；超限返回 `limit_exceeded` 且不进入分发。Runtime 另限 16 个在途桥调用
- **桥尺寸**：输入上限 140 000 字节、operation 名 64 字节——与 `quickjs` 的无界检查共用 `plugin/limits` 常量，两处判定不会漂移；输出含成功包络最多 1 MiB，DTO 与最终包络分别流式编码检查，超限输出小型 `limit_exceeded` 错误、审计 success 为 false
- **审计**：API 日志只含固定身份/操作/权限/成功/请求 ID，不记录参数或数据；权限映射按**完整 operation** 匹配（`contest.rank` → 自身权限，其余 `contest.*` → contest.read；`storage.compareAndSet` 同时标记 storage.read 与 storage.write）；operation Debug 转义并截断，回归验证日志不含存储值。参见 [API](api/mod.md)
