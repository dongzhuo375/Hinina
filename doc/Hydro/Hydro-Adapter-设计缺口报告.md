# Hydro 适配器 — 设计缺口报告

> 2026-09-19 ｜ 分支 `feat/hydro-adapter`
>
> **本文档讲两件事**：Hydro 协议**缺什么**（能力边界与有损映射），以及为接入它**改过哪些既有代码**。
> 设计层面的撞车（架构冲突、双方设计不一致、后续处置）在
> `doc/Hydro/适配新架构的冲突记录.md`；协议事实依据在 `doc/Hydro/HYDRO-API.md`；
> 适配器实现见 `src-tauri/src/adapter/hydro/`，模块文档见 `doc/modules/src-tauri/adapter/hydro/`。

---

## 状态一览

| # | 缺口 | 状态 | 一句话 |
|---|---|---|---|
| **D1** | `ContestProblem.cid: i64` 装不下 hex 比赛 ID | ✅ 已解决 | `cid` 改 `String` |
| **D2** | `OjConfig.contest_id: i64` 同上 | ✅ 已解决（#20） | 换成 `contest_ref: String` |
| **D3** | 前端无 OJ 入口 | ✅ 已解决 | 枚举选择器 + 按需注册 + 保存即建实例 |
| **D4** | 无域名（`/d/:domainId`）配置通道 | ⏸ **待决断** | `OjInstance.options` 传不进适配器 |
| **D5** | `Problem` 无「题面格式」判别位 | 🔶 已知降级 | HTML 题面会按 Markdown 渲染 |
| **D6** | JSON 无样例字段 | 🔶 已知降级 | `samples` 恒空，样例只在题面里 |
| **D7** | 记录无提交时间 | ✅ 已吸收 | ObjectId 前 4 字节反推，回退 `judgeAt` |
| **D8** | `/record` 无总数 / 无 limit / 无 uid | 🔶 已知降级 | `total`/`pages` 按假定页大小推导 |
| **D9** | `JudgementStatus` 缺 Hydro 专属状态 | 🔶 已知降级 | 折入 `Unknown`（文案落差） |
| **D10** | 榜单是预渲染单元格矩阵 | 🔶 已知降级 | 分页/搜索失效，部分字段靠文本反推 |
| **D11** | 无公告接口（对应能力是答疑） | 🔶 已知降级（已确认置空） | 公告页恒空 |
| **D12** | 隐藏本人记录时提交只返回 `tid` | 🔶 已知降级 | 明确报错，无法轮询结果 |
| **D13** | 状态筛选传的是 HOJ 码 | ✅ 已吸收 | 单向翻译；无对应语义时明确报错 |
| **D14** | `HttpClient` 的 Cookie jar 全局共享 | ⏸ 待决断（当前无实害） | — |
| **D15** | pid 可以是字符串（`P1000` / `A1`） | ✅ 已吸收 | 单字母歧义已缓解（提交路径不换算） |
| **D16** | 全局限流 100 请求 / 5 秒 | ✅ 无需动作 | 顺序表缓存把请求放大降到 0 |
| **D17** | `HttpClient` 无法注入自定义请求头 | ✅ 已解决（#20 + 收尾） | 注入 `HeaderMap` + 新增 raw 变体 |
| **D18** | 部署不支持 `X-Hydro-Inject` 时 | ✅ 已吸收（最保守） | 按「无法判定」上抛，绝不判失效 |
| **D19** | 三个 HOJ 专属字段在 Hydro 无来源 | ✅ 无需动作 | 填安全默认值 |
| **D20** | 前端语言域缺 Bash / Haskell 映射 | 🔶 已知降级 | 高亮回退 C++、文件名回退 `main.txt` |

**结论**：设计基本成立 —— 20 条差异里 **19 条已在适配层吸收或已按需修复**，**只剩 D4 需要决断**
（D14 仅备案）。🔶 的 8 条属**协议本身不提供该能力**，建议在用户文档里明示边界，而不是继续改代码。

---

## 1. 结论摘要

**设计基本成立**：Hydro 与 HOJ 是两套协议，但差异几乎全部落在 Adapter 层 —— 4 个 Provider trait
（`AuthProvider` / `ContestProvider` / `ProblemProvider` / `SubmissionProvider`）与领域实体**足以承载**
Hydro 的 5 个核心闭环（登录 / 取比赛 / 看题目 / 提交 / 看评测），**没有一条差异迫使我们在 Service 层
做妥协**；entity 层只有两处字段类型（D1 / D2）需要调整，且都已解决。

**接入成本实测**：新增 OJ = 1 个子目录 + `adapter::factories()` 一行 + 一条配置，且可从 UI 自助启用
（见 §8）。

---

## 2. 为什么不能复用 HOJ 的调用方式（协议对照）

HOJ 是 Hydro 的衍生版并自带一层 REST + JWT，**两者不是同一个 API**（连实现栈都不同：HOJ 是
Java + MyBatis-Plus + Redis，Hydro 是 Node + MongoDB）：

| 维度 | HOJ（`adapter/hoj`） | Hydro（`adapter/hydro`） |
|---|---|---|
| 接口体系 | 统一 REST `/api/*` | 传统 Handler 路由 + `Accept: application/json` 内容协商（JSON-RPC 只注册了 user/users/domain/problem，缺题目列表/记录/状态） |
| 认证载体 | 响应头 `authorization` 里的 JWT | `Set-Cookie: sid=<32 位>`（登录响应体里**没有** token） |
| 认证传递 | `Authorization: <jwt>` | `Authorization: Bearer <sid>`（服务端取空格分隔第 2 段；**该头一旦出现即完全覆盖 Cookie**） |
| 响应包络 | `{status, msg, data}` | **无包络**：成功即原始 body；失败 `{"error":{"name","params","code"}}`（**无 message**） |
| 未登录 | HTTP 401 或 HTTP 200 + 体内 `status:403` | HTTP 200 + `{"url":"/login?redirect=..."}`（重定向被 JSON 化）或 403 `PrivilegeError`；**无效 sid 静默降级为匿名** |
| 当前用户 | `get-user-auth-info` | **无 `/user/me`**：靠 `X-Hydro-Inject: UserContext` 注入到任意路由的响应体 |
| 凭证轮换 | `Refresh-Token` 头 + 新 `Authorization`（需回写磁盘会话） | 无轮换；sid 值不变，服务端滑动续期 |
| 分页 | `{records,total,size,current,pages}` | `page` + `<前缀>pcount`；记录列表**无总数**、无 limit、无 uid 参数 |
| 题目限时 | 独立接口 `get-contest-problem-details` | 详情 JSON 的 `pdoc.config.{timeMin,timeMax,memoryMin,memoryMax}` |
| 样例 | 题面 HTML 里可解析 `<input>/<output>` | 题面是单块 Markdown，**无独立样例字段** |
| 榜单 | 结构化 VO（ACM/OI 两套），支持分页/搜索/打星/赛后提交 | **预渲染单元格矩阵** `rows`，无分页/搜索/打星参数 |
| 状态码 | 0–15 | 0–33（含 CANCELED/HACKED/FETCHED/IGNORED/FORMAT_ERROR/HACK_*） |
| 语言值 | 显示名（`"C++"`） | key（`cc.cc17`），展示名见 `langRange` |

---

## 3. 适配层已吸收的差异（设计有效的证据）

以下差异**完全在 Adapter 内解决**，未触碰任何现有文件：

| # | 差异 | 吸收方式 |
|---|---|---|
| A1 | 无统一包络 | `HydroResponse::into_value` 固定判定顺序：状态码 → 错误包络 → 登录重定向 → 类型化解析 |
| A2 | 错误无 message | `HydroError::ApiError{code,name,params}` 原样携带，文案由调用方组织 |
| A3 | 未登录是 HTTP 200 重定向 | `types::login_redirect_url` 只认指向 `/login` 的 url → `AppError::Auth` |
| A4 | 无效 sid 静默匿名 | 会话失效判据改为 `UserContext._id == 0`（而非 HTTP 状态码） |
| A5 | Cookie 会话 ↔ Hinina 的 token 契约 | `restore_token` 存裸 sid，`headers()` 组装时补 `Bearer ` 前缀（服务端不校验 scheme 名） |
| A6 | 登录响应体无 token | 走 infra 的 raw 变体读响应头 `Set-Cookie`（并因 `Accept: application/json` 避免 302 丢失该头） |
| A7 | 无 `/user/me` | `X-Hydro-Inject: UserContext` 注入头 + `probe_user_context`（严格）/`current_user`（宽松）双入口 |
| A8 | 状态码值域不同（0–33 vs 0–15） | `types::map_status` 全表映射；`FETCHED(22)` 特意折入非终态的 `Pending`（否则轮询提前停住 + 在途结果进终态缓存） |
| A9 | 前端状态筛选传 HOJ 码 | `types::hoj_status_to_hydro` 单向翻译，无对应语义时明确报错 |
| A10 | 语言 key ↔ 显示名 | `types::LANG_TABLE` 双向静态表（29 项，双射由测试锁定），展示名刻意选前端能识别的前缀写法 |
| A11 | 记录无提交时间 | `types::objectid_seconds` 从 `_id`（时间型 ObjectId）前 4 字节反推，回退 `judgeAt` |
| A12 | 榜单是单元格矩阵 | `types::scoreboard_rank_page` **按表头 `type` 定位列**（不依赖列顺序）后逐格归一为 `RankCell` |
| A13 | 展示字母只存在于 `tdoc.pids` 下标 | `resolve_problem_id`（字母 → pid）+ `map_contest_problems`（下标 → 字母），带 60s 顺序表缓存避免请求放大 |
| A14 | 测试点无独立接口 | 复用 `/record/:rid` 的 `rdoc.testCases`，按 `subtaskId` 分组（不猜 `rdoc.subtasks` 结构） |
| A15 | `psdict` 以 docId 为键而入参是 pid | 经 `pdict` 反查 docId，未提交的题不出现在返回 map 中 |
| A16 | 比赛 ID 是 hex ObjectId | `Contest.id` 与 `ContestProblem.cid` 都是 `String` ✓ |
| A17 | 比赛阶段需客户端推算 | `contest_status(start, end, now)` 纯函数（时间缺失按「进行中」，避免误判为已结束） |
| A18 | 灵活时长模式 `endAt` 缺省 | 由 `beginAt + duration`（小时）推算 |

---

## 4. 已解决的缺口

### D1. `ContestProblem.cid: i64` 装不下 Hydro 的比赛 ID → ✅ 已解决

- **当时的问题**：Hydro 的比赛主键是 24 位 hex ObjectId，`i64` 装不下 → 只能填 `0`。
- **解法**：`cid` 改为 `String`，语义与 `OjConfig::contest_ref` 对齐（比赛是对服务端资源的
  **不透明引用**）。HOJ 填数字串（`p.cid.to_string()`）、Hydro 如实携带 ObjectId。
- **影响面**：entity + 两个 adapter + 前端 `types/contest.ts` + 2 处测试夹具（跨端契约，一次改完）。

### D2. `OjConfig.contest_id: i64` 装不下 Hydro 的比赛 ID → ✅ 已解决（#20）

- **当时的问题**：`oj.contestId` 是 `i64`（`0` = 不自动加载），Hydro 的比赛 ID 是 hex 字符串 →
  **「自动加载配置的比赛」这条路径对 Hydro 不可用**（`load_configured_contest` → 登录页简报 → 进场）。
- **解法**：`#20` 把它换成 `contest_ref: String`（旧字段经 `legacy_contest_id` 一次性迁移）。
- **影响面**：`core/entity/config.rs` + `commands/contest_cmd.rs` + `services/contest.service.ts`。

### D3. 前端没有 OJ 切换入口 → ✅ 已解决

- **当时的问题**：`SettingsView` 的 OJ 分组只有 `hojUrl`，下拉候选只有配置里已有的实例，
  `LoginView` 也不传 `ojType` → **Hydro 无法从 UI 激活**（只能手改 `config.json`）。
- **解法**（枚举方案，刻意不做实例 CRUD）：
  1. `src/utils/oj.ts` 作为 OJ 类型域唯一权威（`OJ_TYPES` 枚举 + `ojSelectOptions` 候选组装）；
  2. 选中尚未配置的类型 → 引导填地址 → 「保存」upsert 实例并**自动完成切换**；
  3. 后端 `AppContext::ensure_oj_registered`（幂等）让新实例**免重启**生效，`switch_oj` 在校验前调用；
     判定条件与启动注册同源（只认配置里已启用且 id 匹配的实例，不能凭空激活）。
- **影响面**：前端 3 文件（含 6 例测试）+ `core/context.rs`（含 5 例测试）+ `commands/oj_cmd.rs`。
- **遗留**：`OJ_TYPES` 与后端 `adapter::factories()` 需人工同步（新增 OJ 时两处各加一项）。

### D17. `HttpClient` 无法注入自定义请求头 → ✅ 已解决（#20 + 收尾）

- **当时的问题**：Hydro 的 JSON 输出**完全依赖** `Accept: application/json`，另有
  `X-Hydro-Inject`；而 `HttpClient` 只能注入 `Authorization` → 适配器只能直连 `reqwest::Client`，
  **代价是失去 5xx 退避重试**。
- **解法（两步）**：
  1. `#20`：两个 text 变体改为接收调用方构造的 `HeaderMap`（认证方式是 Adapter 层概念）；
  2. 收尾：新增 **raw 变体** `get_text_raw` / `post_text_raw` —— 任意状态码都返回
     `(status, headers, body)`、不做状态码映射（5xx 仍退避重试、耗尽后返回响应；4xx 不重试）。
- **衍生缺口也已修**：infra 原本在非 2xx 时**丢弃响应体**，而 Hydro 的用户可见错误全在包络里
  （`LoginError` / `OpcountExceededError` / `PermissionError`）。raw 变体解决了它，**Hydro 的
  GET 与 POST 两条通道因此合一**，适配器不再直连 reqwest。
- **代价**：401 不再由 infra 自动映射为 `Auth`，改由适配器的 `http_status_error` 承担
  （判据与 infra 的 `status_error` 逐条对齐，否则前端会话守卫失效）。

---

## 5. 待决断

### D4. 无域名（`/d/:domainId`）配置通道 ⏸

- **现象**：Hydro 多域部署的业务路由需要 `/d/:domainId` 前缀（系统域 `system` 可省略）。
  适配器目前按**系统域**拼路径。
- **影响**：若目标部署把比赛放在自定义域（如 `hydro.ac/d/myschool/...`）→ 请求 **404**。
- **现状**：`OjInstance.options` 已能承载 `{"domain": "myschool"}` 这类私有旋钮（弱类型 Map 是
  `#20` 有意为之），但 `AdapterFactory::build(&self, deps, base_url)` **只把地址交给适配器**，
  options 没有传递通道。
- **可选补丁**（择一）：
  ```rust
  // ① 推荐：build 接收整个实例（形状也更贴近 v1.0 插件 manifest）
  fn build(&self, deps: &AdapterDeps, instance: &OjInstance) -> ProviderSet;
  // ② 或单独传 options
  fn build(&self, deps: &AdapterDeps, base_url: &str, options: &Map<String, Value>) -> ProviderSet;
  ```
  适配器侧改动很小（`url()` 加前缀 + 构造参数）；**风险主要在工厂签名（3 个 adapter + 组合根）**。
- **决策所需信息**：**目标 Hydro 部署是否用自定义域**。不用的话本条可降为备案。

### D14. `HttpClient` 的 Cookie jar 全局共享 ⏸（当前无实害）

- **现状**：`with_timeout` 启用 `cookie_store(true)`，jar 在进程内共享；Hydro 登录后 `sid` 会留在里面。
- **影响**：当前只有一个「当前 OJ」，且不同 OJ 通常不同主机 → **无实害**。若将来支持「多 OJ 同时登录」，
  同一主机的不同部署可能串扰。
- **建议**：暂不处理。若引入多 OJ 并存，改为每 Adapter 一个 client（或关掉自动 cookie 存储 ——
  Hydro 侧我们本就显式发 `Authorization`）。

---

## 6. 已知降级（建议在用户文档明示边界，不改代码）

这 8 条都是**协议本身不提供该能力**，或**信息只存在于服务端渲染结果里**。适配层已按最保守的方式降级：
不报错、不误导，但功能上确实弱于 HOJ。

### D5. `Problem` 没有「题面格式」判别位

- Hydro 的 `pdoc.html: bool` 标明 `content` 是 HTML 还是 Markdown；`Problem.description` 只有字符串，
  前端一律按 Markdown 渲染。
- **影响**：HTML 题面（`html: true`）会被当 Markdown 渲染 → 标签被转义或结构错乱（DOMPurify 消毒后
  显示为纯文本标记）。Hydro 后台默认 Markdown，HTML 是可选。
- **若要修**：`Problem` 增加 `description_format` 字段 + 前端渲染分流（跨端契约）。

### D6. Hydro 的 JSON 没有样例字段 → `Problem.samples` 恒空

- 样例写在题面 Markdown 的代码块里（`pdoc.data` 只是测试数据文件列表，需额外权限）。
- **影响**：题面下方的「样例」区块（含复制按钮）不可用；样例仍能在题面里看到。
- **若要修**：前端从题面围栏代码块提取样例卡片（纯前端启发式，不动 entity）。

### D8. `/record` 不返回总条数、无 limit / uid 参数

- 响应只有 `page` + `rdocs`；页大小由服务端 `pagination.record` 决定（默认 100），客户端不能指定；
  用户筛选用 `uidOrName`（不是 `uid`）。
- **影响**：`SubmissionPage.total/pages` 只能按 `ASSUMED_RECORD_PAGE_SIZE = 100` 推导。若部署改了
  该设置，分页控件会出现「多一页空页」或「少一页」的偏差（`onlyMine` 恒真 + 选手通常 <100 条时不可见）。
- **无法精确**：只能靠 `stat=true`，那需要 `PRIV_VIEW_JUDGE_STATISTICS` 特权（选手没有）。

### D9. `JudgementStatus` 缺 Hydro 专属状态

- Hydro 的 9 CANCELED / 11 HACKED / 30 IGNORED / 32 HACK_SUCCESSFUL / 33 HACK_UNSUCCESSFUL
  在 HOJ 的 18 个变体里没有对应项。
- **影响**：这些状态显示为 **"Unknown"**（Hydro 网页端显示 "Cancelled"/"Hacked"/…），文案落差。
- **现状**：折入 `Unknown` 是**刻意不猜**（错映射会误导选手判断自己的提交结果）；31 FORMAT_ERROR
  取语义最近的 `PresentationError`。校赛 ACM 场景极少出现这些状态。

### D10. 榜单是预渲染单元格矩阵，`RankQuery` 基本失效

- **服务端不支持**分页/关键词搜索/移除打星/赛后提交，每题单元格只给展示文本与 `score`：
  - `current_page` / `limit` → 忽略，单页全量（`pages = 1`）；
  - `keyword` → 忽略（**榜单搜索框对 Hydro 无效**，会静默返回全量）；
  - `remove_star` → 忽略（前端「全量快照模式」本就在客户端过滤，故实际可用）；
  - `contains_end` → 忽略（赛后可见性由赛制 `showRecord` 决定）；
  - ACM 单元格的 `error_num` / `ac_time` / `is_first_ac` / `try_num` **靠文本与 `style` 反推**
    （`score == 100` 判 AC、首行 `+n`/`-n`/✓ 取失败次数、橙色 span 取待判次数、`style` 非空判首 A）；
  - OI 家族每题只有分数 → `time_info` 恒空、`error_num` 恒 0；
  - `gender` / `school` / `nickname` 无来源 → **「女生队高亮」不可用**；
  - ACM 的 `total`（总提交数）由各题尝试次数求和反推。
- **现状**：全部在 `types::scoreboard_rank_page` 内完成，并有 3 行（普通/打星/封榜待判）的逐字段测试锁定。
- **建议**：接受，但应告知用户「Hydro 下榜单搜索无效、女生队高亮不可用」。

### D11. Hydro 没有公告接口（对应能力是「答疑」）

- Hydro 只有 `/contest/:tid/clarification`（提问 + 裁判回复的会话），没有单向广播的公告。
- **影响**：公告页恒为空（不报错）。
- **已确认的决策**：**置空**。不做有损映射（把定向回复伪装成全场公告会让选手误读）；
  后续若要支持，应由前端**按 OJ 能力**把「公告」面板切换为「问答」形态，并在 Provider 层新增
  「答疑」能力（新 trait 方法或新实体）。HOJ 是第一优先级，短期不动。

### D12. 比赛隐藏本人记录时，提交只返回 `tid` 而非 `rid`

- `POST /p/:pid/submit` 在 `tid && !pretest && !contest.canShowSelfRecord(...)` 时返回 `{tid}`。
- **影响**：拿不到记录 ID → **无法轮询评测结果**（提交成功但看不到结果）。ACM 赛制下
  `showSelfRecord` 恒真，常规校赛不受影响；`oi` / `strictioi` 在特定配置下会命中。
- **现状**：明确报错「未返回评测记录 ID：本场比赛隐藏本人评测记录」—— 优于静默返回空串
  （后者会让前端轮询一个空 ID 到超时）。

### D20. 前端语言域缺 Bash / Haskell 的扩展名与高亮映射

- Hydro 提供 `bash` / `hs`；适配器译为 `"Bash"` / `"Haskell"` 后，前端 `utils/language.monacoIdStrict`
  无法识别 → Monaco 高亮回退 `cpp`、工作区源文件名回退 `main.txt`。
- **影响**：仅高亮与文件名（**提交不受影响** —— `lang` 由适配器从显示名反查回 key，往返恒等）。
- **若要修**：`utils/language.ts` 补 `bash→.sh` / `haskell→.hs` 与 `monacoIdStrict` 前缀识别。

---

## 7. 备案（已吸收 / 无需动作）

| # | 缺口 | 处置 |
|---|---|---|
| **D7** | 记录无提交时间字段 | `objectid_seconds` 从 `_id` 前 4 字节反推（Hydro 自身也依赖 ObjectId 的时间有序性）；非 24 位 hex / 时间越界一律拒绝并回退 `judgeAt`（测试已锁定） |
| **D13** | 状态筛选传 HOJ 码 | `hoj_status_to_hydro` 单向翻译；HOJ 的 PE(3)/RJE(11)/SF(12)/PA(13)/FREQ(14) 在 Hydro 无对应语义 → **明确报错**而非静默忽略筛选（静默忽略会让选手以为「筛出来的就是全部」） |
| **D15** | pid 可为字符串 | `Problem.id` / `SubmissionRecord.pid` / `display_pid` 本就是 `String` ✓；唯一歧义是 `resolve_problem_id` 把**单字母**入参当展示字母 —— **提交路径刻意不做该换算**，故不会提交到错误的题；顺序表缺失时回退原值并记 warn |
| **D16** | 全局限流 100 请求 / 5 秒 | ① 榜单「全量快照模式」在 Hydro 下只有 1 个请求（服务端不分页）；② limits 批量拉取的额外开销被 60s 顺序表缓存降到 0；③ `OpcountExceededError` 被判为业务错误（不触发登出），消息带错误名便于识别 |
| **D18** | 部署不支持 `X-Hydro-Inject` | `probe_user_context` 对「字段缺失」返回 `Err(Unknown)`（无法判定 → **保留会话**），只有「注入生效且 `_id == 0`」才判失效 —— 若按「匿名」处理，一次版本差异就会把全部在线选手踢回登录页。代价：不支持该头的部署会话永远校验为 `unknown`，靠 `sessionGuard` 的认证类 IPC 失败兜底 |
| **D19** | 三个 HOJ 专属字段无来源 | `Contest.auth` 固定 `0`、`rank_show_name` 空串（前端回退 username）、`allow_end_submit` false、`SubmissionDetail.oi_rank_score` None —— 语义是 HOJ 专属，填安全默认值是正确做法（前端均未消费） |

---

## 8. 对既有代码的改动（汇总）

**本轮（Hydro 分支）全部改动，共 3 个提交**：

| 提交 | 内容 | 触及既有文件 |
|---|---|---|
| `57c45bb` | Hydro 适配器接入数据化 OJ 身份/配置/注册 | `adapter/mod.rs`（2 行：`pub mod hydro;` + `&hydro::FACTORY`） |
| `7ff6c51` | 冲突记录 + 文档同步 | 仅 `doc/**` |
| `2773a67` | 四项收尾（infra raw 变体、OJ 枚举选择器、终态判据清单、`cid` 字符串化） | `infra/http.rs`、`core/context.rs`、`commands/oj_cmd.rs`、`core/entity/{contest,submission}.rs`、`adapter/hoj/mod.rs`、前端 3 文件 |

**这验证了 `#20` 重构的价值**：接入一个新 OJ 不再触碰 `core/provider`、`core/entity/config.rs`、
`core/context.rs`、`commands/auth_cmd.rs` —— 只需在适配器目录与工厂清单各加一处。随后的四项收尾里
只有 D1 / D3 / D17 需要动既有代码，且都是**跨端契约或基础设施的通用改进**，不是为 Hydro 打的补丁。

> **上一轮（`#20` 之前）的 4 处接线已全部作废**（`OJType::Hydro` + `from_name`、`OjConfig::hydro_url`、
> `context.rs` 的 15 行注册块、`parse_oj_type`），原因与处置见 `适配新架构的冲突记录.md` §1。

**新增 OJ 时需同步的位置**（当前 3 处，都是数据不是逻辑分支）：
1. `src-tauri/src/adapter/<oj>/`（新目录 + `FACTORY`）；
2. `adapter::factories()` 加一行；
3. 前端 `src/utils/oj.ts` 的 `OJ_TYPES` 加一项（+ 可选 `ojBaseUrlHint`）。

---

## 9. 待联调清单（本次无可用 Hydro 实例）

所有单元测试基于**按文档手工构造的夹具**（`adapter/hydro/tests/fixtures/`），以下必须在真实部署上验证：

1. **`X-Hydro-Inject: UserContext` 是否生效**（最关键）：决定会话校验能否区分「匿名」与「无法判定」。
   验证方式：登录后抓 `GET /` 的响应体，确认含 `UserContext` 且 `_id` 为真实 uid。
2. **提交时间**：核对记录页显示的提交时刻与 Hydro 网页端一致（验证 ObjectId 反推）。
3. **展示字母解析**：在真实比赛里逐题打开题面，确认 `resolve_problem_id` 的字母 → pid 换算正确
   （尤其题目数 >26 或 pid 形如 `A1` 的场景）。
4. **榜单单元格形态**：确认 `rows` 的列顺序、AC 单元格 `score == 100`、首 A 的 `style`、封榜的
   橙色 span 与实现假设一致（这是反推最多的一处）。
5. **登录响应头**：确认 `Set-Cookie: sid=...` 出现在带 `Accept: application/json` 的 200 响应上
   （而非 302 重定向）。
6. **错误包络**：确认错误响应的 HTTP 状态码等于 `error.code`，且未登录场景确实表现为
   HTTP 200 + `{"url":"/login?..."}`。
7. **限流**：确认 `OpcountExceededError` 的触发阈值与前端轮询节奏无冲突。
8. **`tdoc.pids` 在 `/contest/:tid/problems` 响应里是否存在**（实现优先用响应自带的，
   缺失时回退带缓存的 `/contest/:tid`）。
9. **UI 自助启用全流程**：设置页选 Hydro → 填地址 → 保存 → 自动切换 → 登录（验证 D3 的闭环，
   含「免重启」这一点）。
