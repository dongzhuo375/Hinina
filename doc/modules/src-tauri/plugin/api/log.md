# log

debug/info/warn/error(message) 无需权限：日志只写宿主自己的 `logs/hinina.log`，不读取任何宿主数据，因此 Manifest 与 `plugins.json` 都无需声明。单条 1..1024 UTF-8 字节，禁止非换行/制表控制字符；换行/制表写入前拍平为空格，消息以 Debug 转义落盘，日志行不可被伪造。每激活 30 条/分钟，超限返回 `limit_exceeded` 而非静默丢弃。

级别映射宿主 tracing 级别：release 默认 `info` 会过滤 `debug` 级插件日志，开发构建或 `RUST_LOG=debug` 可见全部。不注入 `console` 对象，对象由插件自行 `JSON.stringify`。日志是排错通道，不是业务凭据；不写 Token、用户名、源码或响应正文。
