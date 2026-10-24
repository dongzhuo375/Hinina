# api/log

## 职责
宿主日志出口：插件排错的唯一通道，写入宿主自己的 `logs/hinina.log`。

## 核心类型/函数
- `MAX_LOG_BYTES: usize = PLUGIN_LOG_BYTES`（1024）
- `PluginLogLevel` — debug / info / warn / error（映射宿主 tracing 级别；开发构建或 `RUST_LOG=debug` 可见全部）
- `validate_log_message(message)` — 单条 1..1024 UTF-8 字节，禁止非换行/制表控制字符
- `sanitize_log_message(message) -> Cow<str>` — 换行/制表拍平为空格（无需拍平时借用原串不分配）；消息以 Debug 转义落盘，日志行不可被伪造
- `LogApi`（trait）— `write(level, message)`

## 直接依赖
- `plugin::limits::PLUGIN_LOG_BYTES`、`plugin::api::PluginResult`
- `async_trait`、`serde`、`std::borrow::Cow`

## 被依赖
- `plugin::execution_api`（`LogApi::write` 实现：级别过滤不扣额度 + 30/min 预算 + 六字段日志行）

## 逻辑流程
- **无需权限**：日志只写宿主自己的日志文件，不读取任何宿主数据，因此 Manifest 与 `plugins.json` 都无需声明
- **预算**：每激活 30 条/分钟，超限返回 `limit_exceeded` 而非静默丢弃；预算只对宿主实际写入的行计费——级别被宿主过滤的调用（如 release 默认 info 下的 debug）直接成功返回、不扣额度，避免刷屏的过滤行挤掉后续重要日志
- **不注入 `console` 对象**：对象由插件自行 `JSON.stringify`；日志是排错通道，不是业务凭据——不写 Token、用户名、源码或响应正文
