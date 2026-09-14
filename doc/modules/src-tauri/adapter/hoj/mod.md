# mod

## 职责
HOJ (Hydro Online Judge) 适配器，实现 `AuthProvider`、`ContestProvider`、`ProblemProvider`、`SubmissionProvider` 四个 trait。

## 核心类型/函数
- `HOJAdapter` — 封装 `Arc<HttpClient>` + `base_url` + `RwLock<Option<String>>`（JWT token）+ `Arc<EventBus>`（凭证轮换事件发布）
- `HOJAdapter::new(http, base_url, event_bus)` — 构造；`base_url` 自动去尾斜杠
- `api_url(path)` — 拼接完整 API URL
- `parse_cid(contest_id) -> AppResult<i64>` — 比赛 ID 解析（HOJ 的 cid 是数字，Hinina 内部统一用字符串传递）。**非法 ID 必须报错而不是回退 0**：HOJ 以 `cid = 0` 表示「非比赛场景」，静默回退会让比赛中的提交落到练习题库——不计入榜单，选手在赛场上无从察觉。`get_contest_rank` / `get_user_problem_status` / `submit` 共用
- `parse_time(s)` — ISO 时间 → Unix 秒级时间戳（纯 std）
- `parse_samples(html)` — HTML `<input>/<output>` 样例 → `Vec<Sample>`（成对匹配）
- `extract_tag_contents(html, tag)` / `unescape_html(s)` — HTML 标签提取与实体反转义
- `extract_refreshed_token(headers)` — HOJ 私有协议：响应头存在 `refresh-token` 时提取新 `authorization` 头作为轮换 token
- `get_json_authed(url)` — 带认证 GET，经 `HttpClient::get_json_with_headers` 取响应头后自动处理 token 轮换
- `post_json_authed(url, body)` — 带认证 POST（JSON body），经 `HttpClient::post_json_with_headers` 同样处理轮换。榜单轮询、题目状态等高频 POST 场景必须走此方法：若漏掉轮换处理，token 到期后会出现周期性 401
- `handle_token_rotation(headers)` — GET/POST 共用的轮换逻辑：检测到新 token 时更新内存缓存并发布 `AuthEvent::TokenRefreshed`（submit 路径因需前置校验 token 并把错误映射为 `AppError::Submission`，未走 `post_json_authed`，轮换仍内联处理、语义相同）
- `into_contest(ContestVO) -> Contest` — 映射 helper，比赛列表与比赛详情共用，避免两处映射漂移；`seal_rank_time` 为空串或无法解析时视为未设置（`None`）
- `ContestProvider::get_contest_rank(contest_id, query)` — `POST /api/get-contest-rank`：组装 `ContestRankDTO`（`current_page.max(1)`、`limit.clamp(1, 200)` 防御性收敛、`force_refresh` 恒 false、空白 keyword 过滤为 None、`concerned_list` 空、`external_cid_list` None），响应经 `ContestRankVO::into_rank_row` 归一为 `ContestRankPage`；records 的前置副本去重交由前端处理
- `ProblemProvider::get_user_problem_status(contest_id, problem_ids)` — `POST /api/get-user-problem-status`：空列表直接返回空 map（不发无意义请求）；响应为 `HashMap<pid, serde_json::Value>`，逐项经 `types::coerce_problem_status` 归一为 `0/1/2`

## 关键实现约定
- **登录密码**：HOJ 服务端对收到的密码自行 `SecureUtil.md5()` 后比对，客户端发送**明文密码**（不自行 MD5）。
- **token 轮换**：HOJ 服务端在 token 到期前返回 `Refresh-Token: true` + 新 `Authorization` 头。轮换语义为 HOJ 私有协议，由本模块的 `extract_refreshed_token()` 解析（infra 层仅透传原始响应头）；`get_json_authed` / `post_json_authed` 共用 `handle_token_rotation()`，submit 路径内联同等逻辑 —— 检测到轮换时更新本地 token，并发布 `AuthEvent::TokenRefreshed` 事件，由 AuthService 订阅回写磁盘会话，避免重启后回注过期凭证。
- **榜单请求 DTO 约定**：`force_refresh` 恒为 false —— 非比赛创建者/超管传 true 会被服务端忽略，封榜状态应由 `Contest::seal_rank` + `seal_rank_time` 自行判断（HOJ-Contest-Rank-API.md §9.3）。
- **token 回注**：`restore_token(token)` 供 `AuthService::get_session()` 在应用重启后回注会话 token。

## 直接依赖
- `infra::http::HttpClient` — 网络请求
- `core::event::event_bus::EventBus` — 凭证轮换事件发布
- `adapter::hoj::types` — DTO 类型 + 状态码映射 + 榜单/题目状态归一函数
- `core::entity::*` — 领域实体（含 `rank::{ContestRankPage, RankQuery}`）
- `core::error::AppError` — 统一错误

## 被依赖
- `core::context.rs` — AppContext::init() 创建并注册到 ProviderRegistry

## 逻辑流程
1. Service 调用 trait 方法（如 `login()`）
2. HOJAdapter 构造 API URL + 发送 HTTP 请求
3. 解析 `ApiResponse<T>` 统一响应包装
4. 适配为领域实体（User/Contest/Problem/ContestRankPage/JudgementResult）
5. 返回 `AppResult`

## 测试
`src-tauri/src/adapter/hoj/tests/mod_tests.rs` 锁定：`parse_time` 的 ISO/空格分隔格式、闰年与非法输入（空串/过短返回 0）、`parse_samples` 的成对匹配/HTML 实体/`<br>` 换行/仅 input 无 output 容错、`parse_cid` 接受数字 ID 且**非法 ID 报错而非回退 0**。（DTO 解析与归一函数的测试见 `tests/types_tests.rs`，对应 `types.md`）
