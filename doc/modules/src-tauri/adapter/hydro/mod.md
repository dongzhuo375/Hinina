# mod

## 职责
Hydro（上游 Hydro OJ，`packages/hydrooj`）适配器，实现 `AuthProvider`、`ContestProvider`、`ProblemProvider`、`SubmissionProvider` 四个 trait，并以 `HydroFactory` 身份接入 `adapter::factories()`（OJ 身份 = `HydroAdapter::ID`，会话文件 `sessions/Hydro.json`）。

> 与 `adapter/hoj` 的区别：HOJ 是 Hydro 的衍生版并自带 REST + JWT 层，两者是**两套协议**。本适配器走 Hydro 的**传统 Handler 路由 + `Accept: application/json`**（Hydro 没有统一 REST API；JSON-RPC `/d/:domainId/api/:op` 只注册了 user/users/domain/problem 几个查询，没有题目列表、记录查询与题目状态，故不采用）。

## 核心类型/函数
- `HydroAdapter` — 封装 `Arc<HttpClient>` + `base_url` + `RwLock<Option<String>>`（会话 sid）+ `RwLock<Option<HydroUser>>`（身份缓存）+ `TtlCache<String, Vec<String>>`（比赛题目顺序缓存）。**不持有 EventBus**：Hydro 没有 HOJ 的 `Refresh-Token` 轮换协议，sid 由服务端滑动续期且值不变，不存在「凭证轮换需回写磁盘」的场景
- `HydroAdapter::ID` — `"Hydro"`（与 `HydroFactory::id()` 一致；决定会话文件名，并与 `oj.instances[].id` 匹配）
- `HydroAdapter::new(http, base_url)` — 构造；`base_url` 自动去尾斜杠。多域部署的 `/d/:domainId` 前缀暂不支持（缺口 D4：`OjInstance.options` 目前传不进适配器），按**系统域**拼路径
- `HydroFactory` / `static FACTORY` — 工厂：`build()` 经 `ProviderSet::full` 聚合四能力（只消费 `deps.http_client`）
- `HydroResponse`（私有）— 一次请求的原始结果（状态码 + 响应头 + 响应体），**只由 POST 通道产生**：
  - `into_value()` — 协议层判定，顺序刻意是「**状态码 → 错误包络 → 登录重定向 → 类型化解析**」。先看状态码才能把网关的 HTML 错误页报成 `HTTP 502` 而不是「响应不是合法 JSON」（把排障引向错误方向）；再认错误包络是因为 Hydro 用 HTTP 状态码承载 `error.code`，而**登录重定向却是 HTTP 200**
  - `parse_value(body, url)`（关联函数）— 协议层判定 + 解析的**共用入口**（GET/POST 两条通道都走它）：非法 JSON → `Serialization`（带 URL 与前 200 字符）；错误包络 → `AppError`；JSON 化登录重定向 → `Auth`
  - `into_json::<T>()` — `into_value()` + 反序列化
- `http_status_error(url, status)` — HTTP 状态码 → `AppError`，镜像 `infra::http::status_error` 的判据（**401 → `Auth`**、403 保持 `Network`）。**两条通道共用**（raw 变体不做状态码映射，故这一步归适配器）
- `now_secs()` — 当前 UTC 秒
- **请求入口（按通道分工）**：
  - `headers(inject_user) -> HeaderMap` — 组装 `Accept: application/json` + `Authorization: Bearer <sid>`（有会话时）+ 可选 `X-Hydro-Inject: UserContext`
  - `get_raw(path, inject_user)` / `get_value` / `get_json::<T>` — GET，走 infra raw 变体（保留 5xx 退避重试）
  - `post_raw(path, body)` / `post_json::<T>(path, body)` — POST，走 infra raw 变体；无 body 时发 `{}`（Hydro 读 `this.request.body`，空对象等价于「无参数」，比 `null` 更不易触发解析歧义）- `probe_user_context()`（私有）— **严格版**用户探测，三态：`Ok(Some)` 服务端确认已登录 / `Ok(None)` 注入生效但 `_id == 0`（明确匿名）/ `Err(_)` 无法判定（网络异常、错误包络、**或响应里根本没有 `UserContext` 字段** = 部署不支持注入头）
- `current_user()`（私有）— **宽松版**用户探测：把「无法判定」折成 `Ok(None)`，供登录与提交列表筛选使用（拿不到 uid 时走降级路径，不让主流程失败）
- `contest_pids(tid)` / `cached_pids` / `store_pids` / `pids_of(tid, tdoc)`（私有）— 比赛题目顺序（`tdoc.pids`），带 60s TTL 缓存
- `resolve_problem_id(contest_id, problem_id)`（私有）— **展示字母 → 真实 pid**：按 Hydro 自身规则 `tdoc.pids[parseInt(letter, 36) - 10]`（A→下标 0 … Z→下标 25）；非单字母入参原样返回；顺序表拿不到该下标时也原样返回并记 warn
- `is_acm_rule(rule)` / `contest_type_of(rule)` / `oi_rank_score_type_of(rule)`（私有纯函数）— 赛制归一
- `contest_status(start, end, now)`（私有纯函数）— `-1` 未开始 / `0` 进行中 / `1` 已结束；`now` 由调用方注入以便单测锁定边界；时间缺失按「进行中」处理
- `into_contest(TdocVO)` / `map_contest_problems(vo, pids)` / `into_problem(&PdocVO)` / `into_judgement_result(&RdocVO)` / `into_submission_detail(&RdocVO, udoc, pdoc)` / `into_submission_record(...)` / `into_judge_case(&TestCaseVO, index)` / `group_sub_tasks(&[JudgeCase])`（私有映射 helper）
- `require_session()` / `restore_token` / `clear_identity()` — 会话状态管理

### 四 trait 实现要点
- `AuthProvider::login(username, password)` — `POST /login`（JSON body：`uname`/`password`/`rememberme: true`）。Hydro 的登录响应**体里没有 token**（只有 `{"url":"/"}`），会话靠 `Set-Cookie: sid=<32 位随机串>` 下发；带 `Accept: application/json` 时重定向被序列化为 HTTP 200 + `{"url":...}`（**不会 302**），Set-Cookie 因此留在同一次响应里。**密码原样发送**（服务端自行哈希比对）。登录后经 `probe_user_context` 取 uid/uname；探测失败则降级为「用输入的用户名 + 空 uid」并记 warn，不让一次探测失败否定已建立的会话
- `AuthProvider::logout()` — `POST /logout`（`GET` 只渲染确认页，不做登出）；远端结果一律忽略，以清除本地身份为主
- `AuthProvider::validate_session()` — 本地无 sid 直接 `Ok(false)`；否则 `probe_user_context()`：`Ok(Some)`→`Ok(true)`、`Ok(None)`→`Ok(false)`（并清身份）、`Err(Auth)`→`Ok(false)`、**其余 `Err` 一律上抛**（网络/5xx/解析失败/部署不支持注入头都属「无法判定」，折成 `Ok(false)` 会在赛前一次断网就把选手踢回登录页）
- `AuthProvider::restore_token(token)` — 回注磁盘会话里的裸 sid；`send` 发送时补 `Bearer ` 前缀
- `ContestProvider::list_contests()` — `GET /contest?page=N` 按 `tpcount` 翻页（页大小由服务端 `pagination.contest` 决定，客户端无法指定；上限 `MAX_CONTEST_PAGES = 20` 页防御）
- `ContestProvider::get_contest(id)` — `GET /contest/:tid`，顺带刷新题目顺序缓存
- `ContestProvider::list_contest_problems(id)` — `GET /contest/:tid/problems`；展示字母由 `tdoc.pids` 下标派生（`pdict` 按 docId 键且 JS 数字键会被重排，不能靠键顺序推字母）；`cid` 只能填 0（缺口 D1）
- `ContestProvider::get_contest_rank(id, query)` — `GET /contest/:tid/scoreboard`，单元格矩阵经 `types::scoreboard_rank_page` 归一。**`RankQuery` 的字段基本被忽略**：Hydro 榜单是整榜算完一次性返回，服务端不接受分页/关键词/打星/赛后提交参数（缺口 D10）
- `ContestProvider::list_announcements(...)` — **恒返回空页**：Hydro 没有公告接口，其对应能力是**答疑（clarification）**，语义与结构都不同（答疑是「提问 + 裁判回复」的会话，公告是单向广播）。不把答疑伪装成公告 —— 那会让选手把裁判的定向回复误读成全场公告（缺口 D11，后续应在 Provider 层新增答疑能力而不是做有损映射）
- `ProblemProvider::list_problems(id)` — 与 `list_contest_problems` 同源，返回摘要（无题面/limits/语言）
- `ProblemProvider::get_problem(contest_id, problem_id)` — `problem_id` 经 `resolve_problem_id` 换算后请求 `GET /p/:pid?tid=`；`pdoc.content`（单块 Markdown）→ `description`，limits 取 `config.timeMin/Max`、`memoryMin/Max`，`config.langs` 经 `types::lang_display` 翻译为 HOJ 显示名
- `ProblemProvider::get_user_problem_status(contest_id, problem_ids)` — `GET /contest/:tid/problems` 的 `psdict`；入参是**真实 pid** 而 `psdict` 以 **docId** 为键，故先经 `pdict` 反查；未提交的题不出现在返回 map 中
- `SubmissionProvider::submit(contest_id, problem_id, language, code)` — `POST /p/:pid/submit`（`lang` + `code` + `tid`）；语言经 `types::lang_key` 从显示名反查回 key；**刻意不做字母换算**（万一某题 pid 恰好是单字母，换算会把提交打到另一道题上，这个失败模式不可接受）；响应无 `rid` 时明确报错（比赛隐藏本人记录时服务端返回 `tid`，缺口 D12）
- `SubmissionProvider::get_judgement(id)` / `get_submission_detail(id)` — 同一端点 `GET /record/:rid` 的两种投影，对齐 HOJ 侧拆分
- `SubmissionProvider::list_contest_submissions(query)` — `GET /record?page=&tid=&uidOrName=&pid=&status=`。`only_mine` → `uidOrName=<探测到的 uid>`（拿不到 uid 时报 `Auth`）；`problem_display_id` 直接传字母（服务端按 `tdoc.pids` 解析）；`status` 经 `types::hoj_status_to_hydro` 翻译，无对应语义时明确报错而不是静默忽略筛选；`total`/`pages` 按假定页大小 `ASSUMED_RECORD_PAGE_SIZE = 100` 推导（Hydro 不返回总条数，缺口 D8）
- `SubmissionProvider::get_submission_cases(id)` — 同端点的 `rdoc.testCases`；子任务分组由 `testCases[].subtaskId` 完成（**不解析 `rdoc.subtasks`**：文档未给出其结构，由明确存在的字段分组既确定又不依赖猜测）；`mode` 取题目 `config.type` 透传

## 关键实现约定
- **必须发送 `Accept: application/json`**：Hydro 的响应分支是 `request.json || response.redirect || ?noTemplate=1 || !response.template`，不带该头会渲染 HTML 模板 —— 这是取 JSON 的唯一手段。请求头在适配器里用 `headers()` 组装（infra 只做透传，不假设认证方式）
- **两条通道都走 infra 的 raw 变体**（`get_text_raw` / `post_text_raw`：任意状态码都返回 status + headers + body）：Hydro 的用户可见错误全在响应体包络里（`LoginError` / `OpcountExceededError` / `PermissionError`），登录还要读 `Set-Cookie`，而非 raw 变体会在非 2xx 时丢弃响应体。GET 的 5xx 仍由 infra 退避重试（耗尽后返回响应）；POST 不重试。代价是 **401 不再由 infra 自动映射为 `Auth`** —— 由本适配器的 `http_status_error` 承担（判据与 infra 的 `status_error` 逐条对齐，否则会话守卫失效）
- **错误包络没有 message**：`{"error":{"name","params","code"}}`（`message` 是原型上的非枚举 getter，不参与序列化），文案必须由调用方或前端自行组织；`code` 即 HTTP 状态码
- **未登录的 JSON 化重定向**：Hydro 在 `onerror` 里对未登录的 `PermissionError`/`PrivilegeError` 重定向到 `/login?redirect=...`，JSON 模式下是 **HTTP 200 + `{"url":"/login?..."}`**。不识别它会把「会话过期」当成「成功但数据为空」，前端永远回不到登录页 → `types::login_redirect_url` 只认指向 `/login` 的 url（`{"url":"/"}`、文件下载签名链接不算）
- **`Authorization` 头一旦出现即完全覆盖 Cookie**：即使格式不对也不回退到 sid Cookie，故只在持有 sid 时附加该头（服务端取空格分隔的第 2 段，scheme 名不参与校验）
- **sid 失效会静默降级为匿名**：Hydro 对无效 sid 不报错，只把 `UserContext._id` 置 0 → 会话失效的判据是 `_id == 0`，而不是 HTTP 状态码
- **注入头不被支持时不得判失效**：若响应里根本没有 `UserContext` 字段（部署版本不支持 `X-Hydro-Inject`），必须按「无法判定」上抛（缺口 D18）—— 否则一次版本差异就会把全部在线选手踢回登录页
- **三态契约**：见 `AuthProvider::validate_session` 的分支说明；判据与 HOJ 侧同源，测试锁定在 `tests/mod_tests.rs`
- **不发送 `Referer`**：Hydro 的 POST 在带 `Referer` 且 host 与请求 host 不符时抛 `CsrfTokenError`(403)；reqwest 默认不发送该头，故天然安全（不要在后续改动里加上 Referer）
- **展示字母由 `tdoc.pids` 下标派生**：依据 `record_main` 的 `tdoc.pids[parseInt(pid, 36) - 10]`（A=0 … Z=25）；超过 26 题退回十进制序号
- **`cid` 填 0**：Hydro 的比赛 ID 是 24 位 hex ObjectId，装不进 `ContestProblem.cid: i64`（缺口 D1，需改 entity 字段类型）
- **不缓存领域数据**：唯一的缓存是 `tid → tdoc.pids` 的顺序表（`TtlCache`，60s TTL / 32 容量），属于「协议换算所需的映射」而非业务数据 —— 前端「题目 limits 渐进填充」会对每道题各调一次 `get_problem`，不缓存会为一场 12 题的比赛重复发 12 次 `/contest/:tid`（Hydro 全局限流仅 100 请求/5 秒）。空列表不缓存（多为异常响应）。键是比赛 ID，天然带 OJ 作用域（适配器实例按 OJ 实例构造）

## 直接依赖
- `infra::http::HttpClient` — 网络请求（两条通道都走其 raw 变体，不再直连 `client()`）
- `infra::cache::TtlCache` — 比赛题目顺序缓存（TTL + 容量淘汰由原语负责）
- `adapter::{AdapterDeps, AdapterFactory}` — 工厂契约（`build()` 接收 deps 与 base_url）
- `adapter::hydro::types` — DTO、响应归一化、状态码/语言映射、榜单单元格矩阵解析
- `adapter::hydro::error` — 错误包络 → `AppError` 的变体判定
- `core::provider::registry::ProviderSet` — 注册侧聚合（`ProviderSet::full`）
- `core::entity::*` — 领域实体（含 `announcement::AnnouncementPage`、`rank::{ContestRankPage, RankQuery}`、`submission::{...}`）
- `core::error::{AppError, AppResult}` — 统一错误
- `reqwest`（`StatusCode`/`HeaderMap`/`HeaderValue`）、`serde_json::Value`、`tracing`

## 被依赖
- `adapter::mod::factories()` — 清单成员（`&hydro::FACTORY`）
- `core::context.rs` — `AppContext::init()` 遍历工厂清单，按 `oj.instances`（enabled）经 `HydroFactory::build` 构造并注册

## 逻辑流程
1. Service 调用 trait 方法（如 `login()`）
2. 组装路径与请求头（`headers()`）→ GET 走 `get_json`（infra，含退避重试）/ POST 走 `post_json`（原始 reqwest）
3. 协议层判定（`HydroResponse::parse_value`，两条通道共用）：非法 JSON → `Serialization`；错误包络 → `AppError`；JSON 化登录重定向 → `Auth`；POST 额外先按状态码判定（非 2xx 且无包络 → `http_status_error`）
4. 反序列化为 DTO（`into_json` / `get_json`）
5. 映射为领域实体（User/Contest/ContestProblem/Problem/ContestRankPage/JudgementResult/SubmissionRecord/SubmissionDetail/SubmissionCases/AnnouncementPage）
6. 返回 `AppResult`；补上下文一律 `AppError::context()`，**变体保持不变**

**会话校验专用流程**（三态契约）：

```
本地无 sid ────────────────────────────────► Ok(false)   （本地即可判定，不发请求）
有 sid → GET / （带 X-Hydro-Inject: UserContext）
       → probe_user_context()
           ├─ 注入生效 + _id != 0 ──────────► Ok(true)    服务端确认有效
           ├─ 注入生效 + _id == 0 ──────────► Ok(false)   服务端明确判定失效
           ├─ Err(Auth)（错误包络/登录重定向）► Ok(false)   服务端明确判定失效
           └─ 其余 Err ─────────────────────► Err(…)      无法判定：网络/5xx/解析失败/
                                                          部署不支持注入头 → 保留本地会话
```

## 测试
`tests/mod_tests.rs`（+ `tests/fixtures/*.json`，按 `doc/Hydro/HYDRO-API.md` 手工构造，无联调实例）：
- 响应处置：非 2xx + 错误包络 → `Auth`；**网关 HTML 错误页 → `Network` 且带状态码**（不得报成「不是合法 JSON」）；401 → `Auth`；HTTP 200 + 登录重定向 → `Auth`；成功但字段不匹配 → `Serialization`
- 比赛阶段（`contest_status` 全分支含边界与时间缺失）、赛制归一、`oi_rank_score_type` 只对 oi/strictioi 标注
- `into_contest`（时间/赛制/封榜）、`map_contest_problems`（**字母按下标派生而非顺序**、无顺序表时的回退）、`into_problem`（Markdown 题面/limits/语言翻译/无 config 时的零值）
- 评测投影：非终态清零指标、CE 文本拼接、非终态隐藏 score、`display_id` 由 pids 下标派生、ACM 的 score 必须为 `None`
- 测试点：字段缺失落默认值、按 `subtaskId` 分组与排序、平铺题无子任务
- **共用解析入口**（`HydroResponse::parse_value`，GET 通道直接依赖它）：错误包络 → `Auth`、登录重定向 → `Auth`、非 JSON → `Serialization`（消息含「不是合法 JSON」）、成功载荷透传（并已 `strip_nulls`）
- **工厂契约**：`HydroAdapter::ID == "Hydro"` 且 `session_file() == "Hydro.json"`；`FACTORY.id()` 与适配器 ID 一致；四能力齐备（通用断言只查「至少一个能力」，完整能力在此显式锁定，避免漏装能力拖到运行期 `ProviderNotFound`）
- 内部状态：`restore_token`（空串不回注）/`clear_identity`、顺序表缓存（空表不写 + 写入即命中 + 键隔离；过期语义归 infra `cache_tests.rs`）、URL 拼接
