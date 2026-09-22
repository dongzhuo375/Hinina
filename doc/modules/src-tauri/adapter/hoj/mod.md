# mod

## 职责
HOJ 适配器，实现 `AuthProvider`、`ContestProvider`、`ProblemProvider`、`SubmissionProvider` 四个 trait；并提供工厂 `HojFactory`（静态单例 `FACTORY`，`adapter::factories()` 清单成员）供组合根按配置实例构造。

## 核心类型/函数
- `HOJAdapter` — 封装 `Arc<HttpClient>` + `base_url` + `RwLock<Option<String>>`（JWT token）+ `Arc<CoreEventBus>`（凭证轮换的脱敏事实通知）+ `Arc<dyn SessionRepository>`（轮换时**显式**回写磁盘会话）
- `HOJAdapter::ID`（关联常量 `"HOJ"`）— HOJ 的 OJ 身份标识（会话文件名 = `sessions/{ID}.json`，值须与历史枚举 Debug 输出一致以兼容既有会话文件）
- `HOJAdapter::new(http, base_url, oj_id, event_bus, session_repo)` — 构造；`base_url` 自动去尾斜杠
- `api_url(path)` — 拼接完整 API URL
- `parse_cid(contest_id) -> AppResult<i64>` — 比赛 ID 解析（HOJ 的 cid 是数字，Hinina 内部统一用字符串传递）。**非法 ID 必须报错而不是回退 0**：HOJ 以 `cid = 0` 表示「非比赛场景」，静默回退会让比赛中的提交落到练习题库——不计入榜单，选手在赛场上无从察觉。`get_contest_rank` / `get_user_problem_status` / `submit` 共用
- `parse_time(s)` — 已**上提到 `adapter::time`**（与 Hydro 共用同一份实现，见 `adapter/time.md`）。此前这里按固定 19 字符取位，把实测格式 `2026-09-21T16:00:00.000+0000` 的偏移后缀截断忽略 —— UTC 部署下碰巧正确，非 UTC 部署会整体偏移且静默（P64 / P30）
- `parse_samples(html)` — HTML `<input>/<output>` 样例 → `Vec<Sample>`（成对匹配）
- `extract_tag_contents(html, tag)` / `unescape_html(s)` — HTML 标签提取与实体反转义
- `extract_refreshed_token(headers)` — HOJ 私有协议：响应头存在 `refresh-token` 时提取新 `authorization` 头作为轮换 token
- `get_json_authed(url)` — GET，经 `HttpClient::get_text_with_headers` 取原始响应体与响应头，处理 token 轮换后交 `parse_hoj_json` 解析。**token 缺失时不报错**，按匿名请求发出：`get-contest-list` 等 `@AnonApi` 接口在登录页（尚无会话）就要能用，且 HOJ 对匿名接口带无效 token 也照常返回 200。请求头经 `auth_headers(token)` 组装（HOJ 约定：JWT 直接放 `Authorization` 头、无 `Bearer` 前缀；token 缺失/含非法头字符时为空 map，按匿名发出）—— 认证方式是 Adapter 层概念，infra 只收通用 `HeaderMap`
- `post_json_authed(url, body)` — POST（JSON body），同样取原始响应体、处理轮换、交 `parse_hoj_json`。榜单轮询、题目状态等高频 POST 场景必须走此方法：若漏掉轮换处理，token 到期后会出现周期性 401
- `parse_hoj_json::<T>(body, url)` — **全部 HOJ 响应的唯一解析入口**：`from_str` → `types::strip_nulls` → `auth_failure_from_body` → `from_value`。两类解析失败都归 `AppError::Serialization`，消息带 URL 与响应体前 200 字符 —— 「不是合法 JSON」通常是网关返回了 HTML 错误页，「字段不匹配」才是 DTO 问题，分开描述才能一眼定位
- `auth_failure_from_body(&Value) -> Option<AppError>` — 识别 HOJ 放在**响应体**里的鉴权失败并翻译成 `AppError::Auth`。判定刻意保守（见「关键实现约定」）
- `preview(body) -> String` — 截取响应体前 200 字符用于错误诊断，按**字符**而非字节截断（响应含中文比赛标题，按字节切会落在 UTF-8 序列中间）
- `require_token()` — 提交等必须登录的操作的前置断言。`get/post_json_authed` 允许匿名，因此需要登录的接口自行断言，给出「请先登录」而不是等服务端返回 401
- `session_validity_from_response(result: AppResult<ApiResponse<Value>>) -> AppResult<bool>`（私有**纯函数**）— 把 `get-user-auth-info` 的调用结果映射为三态契约的 `Ok(true)` / `Ok(false)` / `Err`。抽成纯函数是为了让「哪些情况算服务端**明确**判定失效」可被单元测试锁定 —— 这条判据直接决定选手会不会在赛前被一次网络抖动踢回登录页。四条分支：
  - `Ok(resp)` 且 `resp.is_success()`（体内 `status == 200`）→ `Ok(true)`：服务端确认有效
  - `Ok(resp)` 但体内非 200（400 参数错误 / 500 服务端异常等）→ `Err(AppError::Unknown("HOJ 会话校验返回非成功状态 status={n}"))`：既不是成功也不是鉴权失败，**无法据此断定会话状态**，按「无法判定」上抛而不是清会话
  - `Err(AppError::Auth(msg))` → `Ok(false)`：服务端明确判定失效。**两条来源都要认** —— ① HTTP 401（infra `status_error` 映射为 `Auth`）；② HTTP 200 + 体内 `status=401`/`403`+登录提示（`parse_hoj_json` 的 `auth_failure_from_body` 映射为 `Auth`）。实测 HOJ 两种报法都存在，漏掉任一条都会让真过期的 token 永不登出
  - 其它 `Err(e)` → `Err(e.context("HOJ 会话校验"))`：网络异常、超时、5xx、响应解析失败一律属「无法判定」，**变体保留**后由 `AuthService` 映射为 `SessionValidity::Unknown` 并保留本地会话
- `handle_token_rotation(headers)` — GET/POST/submit 共用的轮换逻辑，顺序有意且每一步都不能省：① 先更新内存 token（后续请求立刻用新凭证）；② 再经 `session_repo.rotate_token(&oj_id, &new_token)` **显式**把新 token 写回磁盘会话（持久性保证，必须当场完成）；③ **仅在落盘成功（`Ok(true)`）后**才发布 `CoreEvent::TokenRotated { oj_id }`（**只带 OJ 标识**）—— `Ok(false)`（无磁盘会话，轮换发生在登录落盘之前的极端时序）与 `Err`（写盘失败）都**不发布**：本事件的语义是「已轮换且已落盘」，未落盘却发布等于让审计轨迹记录一件没发生的事。两条未发布路径只记日志（写盘失败 `warn`）—— HTTP 请求本身已成功，不能因落盘失败把它变成错误
- `into_contest(ContestVO) -> Contest` — 映射 helper，比赛列表与比赛详情共用，避免两处映射漂移；`seal_rank_time` 为空串或无法解析时视为未设置（`None`）；`oi_rank_score_type` 直接透传（OI 榜单计分规则 "Recent"/"Highest"，非 OI 赛为 `None`）
- `into_announcement(AnnouncementVO) -> Announcement` — 公告映射：id 转字符串、`username` → `author`、时间经 `parse_time` 转秒级时间戳、`content` 为 null 时回退空串
- `into_problem(ProblemInfoVO) -> Problem` — 题目详情映射：id 转字符串、null 描述字段回退空串、`examples` 经 `parse_samples` 拆样例、**`languages`（允许提交语言显示名列表）原样携带**（前端语言选择器的权威来源，不得丢弃）
- `into_submission_record(JudgeVO) -> SubmissionRecord` — 提交列表条目映射：`submit_id`/`pid` 转字符串、状态经 `map_status`、`time`/`memory`/`length` 评测未完成时为 null（去 null 后为 0），负值经 `.max(0)` 钳制后转 `u64`
- `into_submission_detail(types::SubmissionDetail) -> SubmissionDetail` — 提交详情映射（含 code / errorMessage / judger / oiRankScore）；`code` 为 null（未开分享或权限不足）时回退空串（Service 层随后会尝试回落到本地源码快照），`errorMessage` 经 `normalize_error_message` 过滤掉服务端占位文案（详情面板显示空而不是整页报错）
- `into_judgement_result(&types::SubmissionDetail) -> JudgementResult`（私有**纯函数**）— `get_judgement` 的轮询投影：状态经 `map_status`，非终态（`is_terminal_status` 为 false）原样透传且 score/time/memory 清零，终态携带完整指标；**两条路径都带上 `normalize_error_message` 后的错误信息**。抽成纯函数便于单测锁定透传契约
- `into_judge_case(JudgeCaseDTO) -> JudgeCase` — 测试点映射：宽松字段全部落默认值；`status` 缺失按 `-1` 走 `map_status` 归入 `Unknown`
- `AuthProvider::validate_session()` — HOJ 无专门的 session 校验接口，改用需认证的 `GET /api/get-user-auth-info` 间接验证。本地无 token 时**不发请求**直接 `Ok(false)`（本地即可判定失效，不是网络问题）；否则 `Self::session_validity_from_response(self.get_json_authed(&url).await)`。**必须走 `get_json_authed` 而不是 raw `http.client()`**：只有前者才提供去 null 解析、体内鉴权失败识别（`auth_failure_from_body`）、token 轮换处理与 5xx 退避重试；直连 raw client 时既不解析响应体（拿不到体内 401/403）、也不处理轮换头，且会把网络异常与「服务端判定失效」混为一谈
- `ContestProvider::get_contest_rank(contest_id, query)` — `POST /api/get-contest-rank`：组装 `ContestRankDTO`（`current_page.max(1)`、`limit.clamp(1, 200)` 防御性收敛、`force_refresh` 恒 false、空白 keyword 过滤为 None、`concerned_list` 空、`external_cid_list` None），响应经 `ContestRankVO::into_rank_row` 归一为 `ContestRankPage`；records 的前置副本去重交由前端处理
- `ContestProvider::list_announcements(contest_id, current_page, limit)` — `GET /api/get-contest-announcement?cid=&limit=&currentPage=`（分页参数 `.max(1)` 收敛），响应 `PageResult<AnnouncementVO>` 逐条经 `into_announcement` 归一为 `AnnouncementPage`。**不做缓存**：公告可能含裁判组临场规则变更
- `ProblemProvider::get_problem(contest_id, problem_id)` — `GET /api/get-contest-problem-details?cid=&displayId=`：响应 `ProblemInfoVO` 经 `into_problem` 映射为 `Problem`，携带服务端返回的 `languages` 允许语言列表（缺失/null 落空列表）
- `ProblemProvider::get_user_problem_status(contest_id, problem_ids)` — `POST /api/get-user-problem-status`：空列表直接返回空 map（不发无意义请求）；请求体带 **`isContestProblemList = true`**（`false` 时服务端只统计「非比赛提交」，比赛内的提交一律不计 —— 实测 1012：`false → {"1000":{"status":-10}}` 显示未提交，`true → {"1000":{"status":0}}` 已 AC）；响应为 `HashMap<pid, serde_json::Value>`，逐项经 `types::extract_problem_status_code` 取原始码后 `types::normalize_problem_status` 归一为 `0/1/2`
- `SubmissionProvider::submit(contest_id, problem_id, display_id, language, source_code)` — `POST /api/submit-problem-judge`：`require_token()` 前置断言（给「请先登录」而不是等服务端 401）；请求体 `SubmitRequest { pid, language, code, cid, tid: None, gid: None, isRemote: false }`，其中 **`pid` 取 `display_id`（比赛内展示题号，如 `"A"`）**，缺失时才退回 `problem_id` —— 服务端拿它查 `contest_problem.display_id`，查不到会 `contestProblem.getId()` NPE 返回 HTTP 500（实测传数字 pid 必 500）。响应 `JudgeVO` 的 `submitId` 转字符串返回
- `SubmissionProvider::list_contest_submissions(query)` — `GET /api/contest-submissions`：固定携带 `beforeContestSubmit=false`（**必传**：赛前提交不计入榜单，混入会误导选手）与 `completeProblemID=true`（让 `displayPid` 返回完整展示 ID）；`onlyMine` 取自 query（Command 层强制 true）；`problemID`（题目展示 ID）与 `status`（HOJ 状态码）为可选筛选，None/空串时不出现在查询串里。响应 `PageResult<JudgeVO>` 逐条经 `into_submission_record` 归一为 `SubmissionPage`
- `SubmissionProvider::get_judgement(submit_id)` — `GET /api/get-submission-detail?submitId=`：轮询投影，经 `into_judgement_result` 映射为 `JudgementResult`。**非终态原样透传**（5→Pending、6→Compiling、7→Running、9→Pending，不再折叠为 Running），与 `get_submission_detail` 的 `map_status` 输出一致 —— 同一排队提交在两处展示同一状态；前端终态判据 `isTerminalStatus` 以「非 Pending/Compiling/Running 即终态」收敛轮询，语义不受影响（后端 `SubmissionService::get_judgement` 单次查询同样按三态判非终态：非终态不发 `Judged` 事件、原样透传，是否继续轮询由前端 poller 决定）
- `SubmissionProvider::get_submission_detail(submit_id)` — `GET /api/get-submission-detail?submitId=`：与 `get_judgement` 同一端点，但投影为完整实体 `SubmissionDetail`（含 code / errorMessage / judger），经 `into_submission_detail` 映射
- `SubmissionProvider::get_submission_cases(submit_id)` — `GET /api/get-all-case-result?submitId=`：响应 `JudgeCaseVO` 的两个列表经 `types::lenient_case_list` 逐条宽松转换（单条测试点/单个分组形态异常只跳过该条，绝不让整个响应解析失败 —— 测试点面板降级展示好过整页报错），归一为 `SubmissionCases { cases, sub_tasks, mode }`
- `HojFactory`（`impl AdapterFactory`，静态单例 `pub static FACTORY`）— HOJ 工厂（`adapter::factories()` 清单成员）：`id()` 返回 `HOJAdapter::ID`；`build(&deps, base_url)` 构造 `HOJAdapter` 后以 `ProviderSet::full` 包装四个 trait 实现 —— 注册侧聚合，组合根对每个 OJ 只见一行 `factory.build(&deps, base_url)`

## 关键实现约定
- **登录密码**：HOJ 服务端对收到的密码自行 `SecureUtil.md5()` 后比对，客户端发送**明文密码**（不自行 MD5）。
- **响应体 null 容错**：HOJ 对未设置的字段返回 `null` 而不是省略（实测 `get-contest-list` 的 `sealRank` / `rankShowName` / `sealRankTime` / `count` / `now` / `openPrint` / `gid` 全为 null），而 serde 的 `#[serde(default)]` **只在字段缺失时生效**，显式 null 会报 `invalid type: null, expected a boolean` 并让**整个响应**解析失败。故所有响应统一经 `parse_hoj_json` 先 `strip_nulls` 再类型化解析 —— 放在入口而不是逐字段标注，新增 DTO 字段无需记得处理，也不会再犯同类错误。
- **鉴权失败在响应体里**：HOJ 的鉴权失败不走 HTTP 状态码（实测匿名访问 `get-contest-problem` 返回 HTTP 200 + `{"status":403,"msg":"请您先登录！"}`）。若不在 `parse_hoj_json` 里识别，各调用点会把它包成 Contest / Problem / Submission 变体，而前端 `sessionGuard` 是依据 `variant === 'Auth'` 判定会话失效的 —— token 过期时选手只会看到一堆「比赛数据错误」，永远不会被带回登录页。判定保守：`status == 401` 一律视为会话问题；`status == 403` 仅当消息含「登录 / 登陆 / token / 认证 / 未授权」时才算，否则保留为业务错误（私有赛未注册、需要密码），避免把无权访问误判成会话失效而踢人。
- **三态契约：网络异常绝不可折成 `Ok(false)`**：`AuthProvider::validate_session` 的返回值语义是 `Ok(true)` 有效 / `Ok(false)` 服务端**明确**判定失效 / `Err(_)` 无法判定。`Ok(false)` 会让 `AuthService` 清磁盘会话 + 发布 `SessionExpired` → 前端 `invalidateSession` → 登出踢回登录页；因此把网络抖动、超时、5xx、解析失败折成 `Ok(false)` 的代价是**赛前一次断网就把选手踢回登录页**，而反复重登还可能触发 HOJ 的暴力破解锁定（同 IP + 同用户名 30 分钟 20 次）—— 恰好是本项目要防的场景。反之，若把真正的 401 当成「无法判定」上抛，则 token 过期后永不登出，选手被卡在比赛页反复失败。两个方向都不能错，故判据抽成纯函数 `session_validity_from_response` 并由单元测试锁定（见「测试」）。
- **错误变体绝不被改写**：补上下文一律用 `AppError::context()`（保留变体），禁止 `AppError::Network(format!("xx 请求失败: {}", e))` 这类重新包装 —— 它会把反序列化失败、认证失败一律改写成「网络错误」，现场看到「网络错误: … 序列化错误: …」自相矛盾的嵌套消息，把 DTO 问题当断网查，还会让 401 不再触发登出。
- **token 轮换**：HOJ 服务端在 token 到期前返回 `Refresh-Token: true` + 新 `Authorization` 头。轮换语义为 HOJ 私有协议，由本模块的 `extract_refreshed_token()` 解析（infra 层仅透传原始响应头）；`get_json_authed` / `post_json_authed` 共用 `handle_token_rotation()`，submit 也走 `post_json_authed`（此前自带一份内联轮换，与共用实现容易漂移）—— 检测到轮换时更新本地 token、**显式**经 `SessionRepository::rotate_token` 回写磁盘会话（避免重启后回注过期凭证），**落盘成功后**才发布 `CoreEvent::TokenRotated`（只带 OJ 标识，**token 绝不进入事件流**）。为什么由适配器自己落盘：轮换发生在 HTTP 响应处理的当场，新 token 只在这一次响应里出现过；把它交给应用层的事件订阅者去写盘，等于把持久性保证挂在异步投递上（消费者可能落后、可能不存在），而且要把真实凭证塞进事件流。
- **榜单请求 DTO 约定**：`force_refresh` 恒为 false —— 非比赛创建者/超管传 true 会被服务端忽略，封榜状态应由 `Contest::seal_rank` + `seal_rank_time` 自行判断（HOJ-Contest-Rank-API.md §9.3）。
- **token 回注**：`restore_token(token)` 供 `AuthService::get_session()` 在应用重启后回注会话 token。

## 直接依赖
- `infra::http::HttpClient` — 网络请求
- `core::event::core_event::CoreEvent` — 凭证轮换的脱敏事实通知（`TokenRotated { oj_id }`）
- `core::event::core_event_bus::CoreEventBus` — 事件发布
- `core::repository::session_repo::SessionRepository` — 轮换后的凭证**显式**回写磁盘会话
- `adapter::hoj::types` — DTO 类型 + 状态码映射 + 榜单/题目状态归一函数
- `core::entity::*` — 领域实体（含 `announcement::{Announcement, AnnouncementPage}`、`rank::{ContestRankPage, RankQuery}`、`submission::{SubmissionRecord, SubmissionPage, SubmissionQuery, SubmissionDetail, SubmissionCases, JudgeCase, SubTaskCases, ...}`）
- `core::error::{AppError, AppResult}` — 统一错误
- `serde_json::Value` — 会话校验只关心 `ApiResponse` 的 `status`，不解析 `data`

## 被依赖
- `core::context.rs` — AppContext::init() 经 `HojFactory::build` 构造 `ProviderSet` 注册到 ProviderRegistry（active 未注册时的回退目标取自 `list_available()` 首项，不再引用 `HOJAdapter::ID`）
- `adapter::factories()` — 工厂清单引用 `hoj::FACTORY`

## 逻辑流程
1. Service 调用 trait 方法（如 `login()`）
2. HOJAdapter 构造 API URL，经 `HttpClient` 的 text 变体发请求，拿回**原始响应体 + 响应头**
3. `handle_token_rotation(headers)` 处理 HOJ 私有轮换协议
4. `parse_hoj_json(body, url)`：`from_str` → `strip_nulls` → `auth_failure_from_body`（体内 401/403 → `AppError::Auth`）→ `from_value` 得到 `ApiResponse<T>`
5. `ApiResponse::into_data()` 校验 `status == 200` 并取出 `data`
6. 适配为领域实体（User/Contest/Problem/ContestRankPage/JudgementResult/AnnouncementPage/SubmissionPage/SubmissionDetail/SubmissionCases）
7. 返回 `AppResult`；传输/解析失败经 `AppError::context("HOJ xxx")` 补环节名，**变体保持不变**

**会话校验专用流程**（`validate_session`，三态契约）：

```
本地无 token ─────────────────────────────► Ok(false)   （不发请求，本地即可判定）
有 token → get_json_authed("/get-user-auth-info")
         → session_validity_from_response(...)
             ├─ Ok(体内 status=200) ────────► Ok(true)    服务端确认有效
             ├─ Err(Auth) ─────────────────► Ok(false)   服务端明确判定失效
             │    （HTTP 401 或 HTTP 200 + 体内 401/403+登录提示）
             ├─ Ok(体内 status≠200) ────────► Err(Unknown) 无法判定
             └─ Err(其它) ─────────────────► Err(e.context("HOJ 会话校验"))  无法判定
                                              （网络/超时/5xx/解析失败，变体保留）
```

`AuthService::validate_session` 据此把 `Ok(true)`→`Valid`、`Ok(false)`→清会话 + `SessionExpired` + `Invalid`、`Err(_)`→`Unknown` 并**保留**本地会话。

## 测试
`src-tauri/src/adapter/hoj/tests/mod_tests.rs` 锁定：`parse_time` 委托给共用实现后的行为（真实夹具串 `...16:00:00.000+0000` 与显式 UTC 等价、**非零偏移必须真的换算**、非零填充可解析、空串/残缺日期返回 0）、`parse_samples` 的成对匹配/HTML 实体/`<br>` 换行/仅 input 无 output 容错、`parse_cid` 接受数字 ID 且**非法 ID 报错而非回退 0**、`parse_hoj_json` 的去 null 与两类解析失败均归 `Serialization`（含 URL 与响应体前缀）、`auth_failure_from_body` 的 401 恒判/403 保守判/业务性 403 不误判/成功与其它状态码放行、`preview` 按字符截断且去首尾空白。

**会话三态判据**（`session_validity_from_response`，5 项 `validity_*` 测试）：`validity_success_is_valid`（体内 200 → `Ok(true)`）、`validity_auth_error_is_definitively_invalid`（HTTP 401 与体内 403「请您先登录！」**两条 `Auth` 来源都**→ `Ok(false)`）、`validity_network_error_propagates_as_unknown_not_invalid`（回归项：网络异常必须上抛且**变体保留为 `Network`**、消息带「HOJ 会话校验」环节名，此前 `Err(_) => Ok(false)` 会让 `SessionValidity::Unknown` 分支对 HOJ 成为死代码）、`validity_serialization_error_propagates_not_invalid`（DTO 与服务端不匹配不代表 token 失效）、`validity_non_success_body_status_is_unknown`（体内 400/500 → `Unknown` 变体）。

**联调实测事实**：`GET /api/get-user-auth-info` 对伪 token / 无 token / 空 token 一律返回 **HTTP 401**（无 token 时体内还带 `{"status":401,"msg":"请您先登录！"}`），即该端点走 HTTP 状态码而非体内 `status` —— 与 `get-contest-problem`（HTTP 200 + 体内 403）报法不同，故两条 `Auth` 来源都必须认。三条路径实测：无 token → `Ok(false)`（不发请求）；伪 token → `Ok(false)`；指向不可达地址模拟断网 → `Err(Network("HOJ 会话校验: GET 请求失败 …"))`，耗时约 9s（传输错误有 2 次指数退避重试，见 `infra/http.md`）。

**真实响应夹具** `tests/fixtures/contest_list_anon.json`：取自真实 `GET /api/get-contest-list?limit=1000`，仅替换标题/作者/简介等自由文本，完整保留键名、数字、布尔与 null 分布（不含主机名与凭证）。两条测试配套：正向证明真实响应经 `parse_hoj_json` 可解析出 13 场比赛并筛出配置的 contestId；反向证明**不去 null 就必然失败**，防止后来者把 `strip_nulls` 当冗余删掉。（DTO 解析与归一函数的测试见 `tests/types_tests.rs`，对应 `types.md`）

**公告 / 提交列表 / 测试点夹具（P57 待联调）**：`tests/fixtures/announcement_list.json` / `contest_submissions.json` / `case_result.json` 三个夹具按 `doc/HOJ/HOJ-API-Documentation.md` §3.7 / §3.9 / §5.6 的响应形状**手工构造**（保留 HOJ 的显式 null 风格与 `status:200` 信封），字段名与真实服务端的出入需在联调时校正 —— 每个配套测试都标注「按文档构造，未经真实联调校正 —— P57 待联调清单」。覆盖：公告夹具解析 + `into_announcement` 映射（id 转字符串、username→author、时间戳与 `parse_time` 同源）与 null content/updateTime 降级（并反向锁定不去 null 必然失败）；提交列表夹具解析 + `into_submission_record` 映射（含 `status=13 → PartiallyAccepted`，P41 不再折算 AC；null 数值全部落 0）与同样的反向去 null 锁定；测试点夹具解析 + `into_judge_case` 映射、畸形条目（非对象/字段类型异常）只跳过不致命（SubTask 形态文档不完整，宽松转换是刻意设计）；提交详情内联 JSON → 完整实体映射（CE 的 errorMessage、评测未完成时 time/memory 为 null 落 0）与 null code 降级空串；`into_judgement_result` 轮询投影契约（状态码 0 → `Pending` 原样透传且指标清零、1 → `Running`、终态 5 → `Accepted` 携带 score/time/memory）；`ContestVO.oiRankScoreType` → `Contest.oi_rank_score_type` 映射（字段缺失为 None）；题目详情内联 JSON（按 §4.2 形状构造）→ `into_problem` 映射：`languages` 允许语言列表原样透传，字段缺失 / 显式 null 时落空列表（前端回退内置默认）。
