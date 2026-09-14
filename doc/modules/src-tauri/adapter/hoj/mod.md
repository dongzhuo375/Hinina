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
- `get_json_authed(url)` — GET，经 `HttpClient::get_text_with_headers` 取原始响应体与响应头，处理 token 轮换后交 `parse_hoj_json` 解析。**token 缺失时不报错**，按匿名请求发出：`get-contest-list` 等 `@AnonApi` 接口在登录页（尚无会话）就要能用，且 HOJ 对匿名接口带无效 token 也照常返回 200
- `post_json_authed(url, body)` — POST（JSON body），同样取原始响应体、处理轮换、交 `parse_hoj_json`。榜单轮询、题目状态等高频 POST 场景必须走此方法：若漏掉轮换处理，token 到期后会出现周期性 401
- `parse_hoj_json::<T>(body, url)` — **全部 HOJ 响应的唯一解析入口**：`from_str` → `types::strip_nulls` → `auth_failure_from_body` → `from_value`。两类解析失败都归 `AppError::Serialization`，消息带 URL 与响应体前 200 字符 —— 「不是合法 JSON」通常是网关返回了 HTML 错误页，「字段不匹配」才是 DTO 问题，分开描述才能一眼定位
- `auth_failure_from_body(&Value) -> Option<AppError>` — 识别 HOJ 放在**响应体**里的鉴权失败并翻译成 `AppError::Auth`。判定刻意保守（见「关键实现约定」）
- `preview(body) -> String` — 截取响应体前 200 字符用于错误诊断，按**字符**而非字节截断（响应含中文比赛标题，按字节切会落在 UTF-8 序列中间）
- `require_token()` — 提交等必须登录的操作的前置断言。`get/post_json_authed` 允许匿名，因此需要登录的接口自行断言，给出「请先登录」而不是等服务端返回 401
- `handle_token_rotation(headers)` — GET/POST/submit 共用的轮换逻辑：检测到新 token 时更新内存缓存并发布 `AuthEvent::TokenRefreshed`
- `into_contest(ContestVO) -> Contest` — 映射 helper，比赛列表与比赛详情共用，避免两处映射漂移；`seal_rank_time` 为空串或无法解析时视为未设置（`None`）
- `ContestProvider::get_contest_rank(contest_id, query)` — `POST /api/get-contest-rank`：组装 `ContestRankDTO`（`current_page.max(1)`、`limit.clamp(1, 200)` 防御性收敛、`force_refresh` 恒 false、空白 keyword 过滤为 None、`concerned_list` 空、`external_cid_list` None），响应经 `ContestRankVO::into_rank_row` 归一为 `ContestRankPage`；records 的前置副本去重交由前端处理
- `ProblemProvider::get_user_problem_status(contest_id, problem_ids)` — `POST /api/get-user-problem-status`：空列表直接返回空 map（不发无意义请求）；响应为 `HashMap<pid, serde_json::Value>`，逐项经 `types::coerce_problem_status` 归一为 `0/1/2`

## 关键实现约定
- **登录密码**：HOJ 服务端对收到的密码自行 `SecureUtil.md5()` 后比对，客户端发送**明文密码**（不自行 MD5）。
- **响应体 null 容错**：HOJ 对未设置的字段返回 `null` 而不是省略（实测 `get-contest-list` 的 `sealRank` / `rankShowName` / `sealRankTime` / `count` / `now` / `openPrint` / `gid` 全为 null），而 serde 的 `#[serde(default)]` **只在字段缺失时生效**，显式 null 会报 `invalid type: null, expected a boolean` 并让**整个响应**解析失败。故所有响应统一经 `parse_hoj_json` 先 `strip_nulls` 再类型化解析 —— 放在入口而不是逐字段标注，新增 DTO 字段无需记得处理，也不会再犯同类错误。
- **鉴权失败在响应体里**：HOJ 的鉴权失败不走 HTTP 状态码（实测匿名访问 `get-contest-problem` 返回 HTTP 200 + `{"status":403,"msg":"请您先登录！"}`）。若不在 `parse_hoj_json` 里识别，各调用点会把它包成 Contest / Problem / Submission 变体，而前端 `sessionGuard` 是依据 `variant === 'Auth'` 判定会话失效的 —— token 过期时选手只会看到一堆「比赛数据错误」，永远不会被带回登录页。判定保守：`status == 401` 一律视为会话问题；`status == 403` 仅当消息含「登录 / 登陆 / token / 认证 / 未授权」时才算，否则保留为业务错误（私有赛未注册、需要密码），避免把无权访问误判成会话失效而踢人。
- **错误变体绝不被改写**：补上下文一律用 `AppError::context()`（保留变体），禁止 `AppError::Network(format!("xx 请求失败: {}", e))` 这类重新包装 —— 它会把反序列化失败、认证失败一律改写成「网络错误」，现场看到「网络错误: … 序列化错误: …」自相矛盾的嵌套消息，把 DTO 问题当断网查，还会让 401 不再触发登出。
- **token 轮换**：HOJ 服务端在 token 到期前返回 `Refresh-Token: true` + 新 `Authorization` 头。轮换语义为 HOJ 私有协议，由本模块的 `extract_refreshed_token()` 解析（infra 层仅透传原始响应头）；`get_json_authed` / `post_json_authed` 共用 `handle_token_rotation()`，submit 也走 `post_json_authed`（此前自带一份内联轮换，与共用实现容易漂移）—— 检测到轮换时更新本地 token，并发布 `AuthEvent::TokenRefreshed` 事件，由 AuthService 订阅回写磁盘会话，避免重启后回注过期凭证。
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
2. HOJAdapter 构造 API URL，经 `HttpClient` 的 text 变体发请求，拿回**原始响应体 + 响应头**
3. `handle_token_rotation(headers)` 处理 HOJ 私有轮换协议
4. `parse_hoj_json(body, url)`：`from_str` → `strip_nulls` → `auth_failure_from_body`（体内 401/403 → `AppError::Auth`）→ `from_value` 得到 `ApiResponse<T>`
5. `ApiResponse::into_data()` 校验 `status == 200` 并取出 `data`
6. 适配为领域实体（User/Contest/Problem/ContestRankPage/JudgementResult）
7. 返回 `AppResult`；传输/解析失败经 `AppError::context("HOJ xxx")` 补环节名，**变体保持不变**

## 测试
`src-tauri/src/adapter/hoj/tests/mod_tests.rs` 锁定：`parse_time` 的 ISO/空格分隔格式、闰年与非法输入（空串/过短返回 0）、`parse_samples` 的成对匹配/HTML 实体/`<br>` 换行/仅 input 无 output 容错、`parse_cid` 接受数字 ID 且**非法 ID 报错而非回退 0**、`parse_hoj_json` 的去 null 与两类解析失败均归 `Serialization`（含 URL 与响应体前缀）、`auth_failure_from_body` 的 401 恒判/403 保守判/业务性 403 不误判/成功与其它状态码放行、`preview` 按字符截断且去首尾空白。

**真实响应夹具** `tests/fixtures/contest_list_anon.json`：取自真实 `GET /api/get-contest-list?limit=1000`，仅替换标题/作者/简介等自由文本，完整保留键名、数字、布尔与 null 分布（不含主机名与凭证）。两条测试配套：正向证明真实响应经 `parse_hoj_json` 可解析出 13 场比赛并筛出配置的 contestId；反向证明**不去 null 就必然失败**，防止后来者把 `strip_nulls` 当冗余删掉。（DTO 解析与归一函数的测试见 `tests/types_tests.rs`，对应 `types.md`）
