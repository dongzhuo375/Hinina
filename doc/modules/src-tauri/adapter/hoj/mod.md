# mod

## 职责
HOJ (Hydro Online Judge) 适配器，实现 `AuthProvider`、`ContestProvider`、`ProblemProvider`、`SubmissionProvider` 四个 trait。

## 核心类型/函数
- `HOJAdapter` — 封装 `Arc<HttpClient>` + `base_url` + `RwLock<Option<String>>`（JWT token）
- `api_url(path)` — 拼接完整 API URL
- `parse_time(s)` — ISO 时间 → Unix 秒级时间戳（纯 std）
- `parse_samples(html)` — HTML `<pre>` 样例 → `Vec<Sample>`

## 直接依赖
- `infra::http::HttpClient` — 网络请求
- `adapter::hoj::types` — DTO 类型 + 状态码映射
- `core::entity::*` — 领域实体
- `core::error::AppError` — 统一错误

## 被依赖
- `core::context.rs` — AppContext::init() 创建并注册到 ProviderRegistry

## 逻辑流程
1. Service 调用 trait 方法（如 `login()`）
2. HOJAdapter 构造 API URL + 发送 HTTP 请求
3. 解析 `ApiResponse<T>` 统一响应包装
4. 适配为领域实体（User/Contest/Problem/JudgementResult）
5. 返回 `AppResult`
