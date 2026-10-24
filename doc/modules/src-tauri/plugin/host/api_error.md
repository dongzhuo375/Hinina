# host/api_error

## 职责
AppError → PluginError 的脱敏映射，Host 内部适配边界；保证内部错误文本不越过桥。

## 核心类型/函数
- `pub fn redact_app_error(error: &AppError) -> PluginError` — Auth → Unauthenticated；Contest/Problem/Submission → ProviderError；其余 → Unavailable；丢弃全部内部文本

## 直接依赖
- `core::error::AppError`
- `plugin::api::PluginError`

## 被依赖
- `plugin::query` / `plugin::manager`（内部错误压缩点）

## 逻辑流程
- Adapter 应根据**明确业务语义**产生 NotFound/ContextExpired 等，不以内部字符串匹配、也不把 AppError 直接序列化给 JS
- 测试用包含 token、认证头及绝对路径的错误验证 wire 和 Display 不泄露
