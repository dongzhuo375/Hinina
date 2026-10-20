# API 错误适配

redact_app_error 属于 Host 内部适配边界。Auth → Unauthenticated，Contest/Problem/Submission → ProviderError，其余 → Unavailable；丢弃全部内部文本。测试用包含 token、认证头及绝对路径的错误验证 wire 和 Display 不泄露。

未来 Adapter 仍需根据明确业务语义产生 NotFound/ContextExpired 等，不以内部字符串匹配或把 AppError 直接序列化给 JS。
