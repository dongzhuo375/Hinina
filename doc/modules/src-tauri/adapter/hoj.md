# hoj

> 源文件：`src-tauri/src/adapter/hoj/mod.rs`（+ `types.rs` / `error.rs`）

## 职责

HOJ 服务端适配器：把 `core/provider` 的四个 Provider trait（`AuthProvider` / `ContestProvider` / `ProblemProvider` / `SubmissionProvider`）翻译为 HOJ 的 HTTP API。项目约定 OJ 特定行为只进 adapter 层；HOJ 的 API 参考资料在 `doc/HOJ/`，状态码码表与提交接口语义的踩坑记录见 Architecture.md「OJ 状态码表」「提交接口的题目标识各 OJ 不同」两条。

## 核心类型/函数

| 名称 | 用途 |
|------|------|
| `HOJAdapter` | 持 `Arc<HttpClient>`、`base_url`（不含尾部 `/api`）、JWT token（`RwLock<Option<String>>`）、`OjId`、`CoreEventBus`、`Arc<dyn SessionRepository>`；实现四个 Provider trait |
| `HOJAdapter::ID = "HOJ"` | OJ 身份常量（会话文件名 `sessions/HOJ.json`，取值须与历史会话文件兼容） |
| `HojFactory` | `AdapterFactory` 实现（`id()` 返回 `HOJAdapter::ID`），按配置构造 `HOJAdapter` |
| token 轮换 | 响应头 `Refresh-Token` 出现时：新 token 经 `SessionRepository` **显式落盘** → 落盘**成功后**发 `CoreEvent::TokenRotated` → 后续请求换用；适配器自己落盘的原因见 `session_repo` 字段注（持久性不挂在异步事件投递上，且真实凭证不进事件流） |
| `types.rs` | HOJ 响应体到领域类型的投影；`map_status` 评测码表与 `is_terminal_status` 终态判据必须等价（逐码测试锁定） |
| `error.rs` | `HOJError`（ApiError / HttpError / JsonError / Token 缺失等）到 `AppError` 的归一 |

## 直接依赖

- `infra::http`（`HttpClient`）、`core::provider` 四 trait、`core::repository::session_repo`（`SessionRepository`）、`core::event`（`CoreEventBus`）

## 逻辑流程

请求统一带 `Authorization: Bearer <token>`（登录请求除外）；登录从响应头提取 token；提交以**展示题号**为 pid（HOJ 按 `contest_problem.display_id` 查库，查不到返回 HTTP 500——不重试且带响应体摘录）。

## 契约测试（M2）

`tests/mod_tests.rs` 的契约测试段（评审 #8 起并入单文件）经 `test_support::loopback` 打真实网络栈：登录凭证与响应头 token 提取、裸 `Authorization`（无 Bearer）、`Refresh-Token` 轮换（新 token 落盘 + `TokenRotated` 事件 + 后续请求换用）、提交体以展示题号为 pid、服务端 500 不重试且带摘录。
