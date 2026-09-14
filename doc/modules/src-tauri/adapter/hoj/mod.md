# mod

## 职责
HOJ (Hydro Online Judge) 适配器，实现 `AuthProvider`、`ContestProvider`、`ProblemProvider`、`SubmissionProvider` 四个 trait。

## 核心类型/函数
- `HOJAdapter` — 封装 `Arc<HttpClient>` + `base_url` + `RwLock<Option<String>>`（JWT token）+ `Arc<EventBus>`（凭证轮换事件发布）
- `HOJAdapter::new(http, base_url, event_bus)` — 构造；`base_url` 自动去尾斜杠
- `api_url(path)` — 拼接完整 API URL
- `parse_time(s)` — ISO 时间 → Unix 秒级时间戳（纯 std）
- `parse_samples(html)` — HTML `<input>/<output>` 样例 → `Vec<Sample>`（成对匹配）
- `extract_tag_contents(html, tag)` / `unescape_html(s)` — HTML 标签提取与实体反转义
- `extract_refreshed_token(headers)` — HOJ 私有协议：响应头存在 `refresh-token` 时提取新 `authorization` 头作为轮换 token
- `get_json_authed(url)` — 带认证 GET，经 `HttpClient::get_json_with_headers` 取响应头后自动处理 token 轮换

## 关键实现约定
- **登录密码**：HOJ 服务端对收到的密码自行 `SecureUtil.md5()` 后比对，客户端发送**明文密码**（不自行 MD5）。
- **token 轮换**：HOJ 服务端在 token 到期前返回 `Refresh-Token: true` + 新 `Authorization` 头。轮换语义为 HOJ 私有协议，由本模块的 `extract_refreshed_token()` 解析（infra 层仅透传原始响应头）；`get_json_authed` 与 submit 路径在检测到轮换时更新本地 token，并发布 `AuthEvent::TokenRefreshed` 事件，由 AuthService 订阅回写磁盘会话，避免重启后回注过期凭证。
- **token 回注**：`restore_token(token)` 供 `AuthService::get_session()` 在应用重启后回注会话 token。

## 直接依赖
- `infra::http::HttpClient` — 网络请求
- `core::event::event_bus::EventBus` — 凭证轮换事件发布
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
