# Hydro 适配器 — 设计缺口报告

> 生成日期：2026-09-17 ｜ 分支：`feat/hydro-adapter` ｜ 基线：`main` @ `0e0ccdb`
>
> **本文档的用途**：实现 Hydro 兼容时，凡**适配层无法吸收**的差异一律**不修改现有框架代码**，
> 在此逐条留档并附建议补丁，交由项目负责人决断设计是否修改。
> 适配器实现见 `src-tauri/src/adapter/hydro/`，模块文档见 `doc/modules/src-tauri/adapter/hydro/`。

---

## 1. 结论摘要

**设计基本成立**：Hydro 与 HOJ 是两套协议，但差异几乎全部落在 Adapter 层 —— 4 个 Provider trait
（`AuthProvider` / `ContestProvider` / `ProblemProvider` / `SubmissionProvider`）与领域实体**足以承载**
Hydro 的 5 个核心闭环（登录 / 取比赛 / 看题目 / 提交 / 看评测），**没有一条差异迫使我们在 Service 层
或 entity 层做妥协**。

> **状态更新（2026-09-19）**：main 的 #20（身份/配置/注册数据化）落地后，本报告的**改动面已变**：
> 原先「本次对既有代码的 4 处接线改动」全部由 #20 取代（§5 已改写），并新增一条更窄的 infra 缺口
> （非 2xx 丢弃响应体）。条目状态变化：**D2 已解决**（`contest_ref: String`）、
> **D3 部分解决**（有 OJ 下拉与 `switch_oj`，但不能自助新增实例）、
> **D17 已修复**（`HttpClient` 现接收 `HeaderMap`）但衍生新缺口、
> **D4 有配置位置但传不进适配器**。细节见 `doc/Hydro/适配新架构的冲突记录.md`。

按处置紧迫度分三档（括号内为 2026-09-19 状态）：

| 档位 | 含义 | 条目 |
|---|---|---|
| **A. 必须改才能真正用起来** | 不改则 Hydro 无法被激活/关键流程不可用 | D1（仍未解决）、D2（**已解决**）、D3（**部分解决**）、D4（**待定**）、D17（**已修复**，衍生 infra 丢响应体） |
| **B. 建议改（有损但可用）** | 不改则功能降级或语义失真 | D5、D6、D8、D9、D10、D11、D12、D18 |
| **C. 可接受 / 仅备案** | 记录事实，暂不建议动 | D7、D13、D14、D15、D16、D19、D20 |

---

## 2. 为什么不能复用 HOJ 的调用方式（协议对照）

HOJ 是 Hydro 的衍生版并自带一层 REST + JWT，**两者不是同一个 API**：

| 维度 | HOJ（`adapter/hoj`） | Hydro（`adapter/hydro`） |
|---|---|---|
| 接口体系 | 统一 REST `/api/*` | 传统 Handler 路由 + `Accept: application/json` 内容协商（JSON-RPC 只注册了 user/users/domain/problem，缺题目列表/记录/状态） |
| 认证载体 | 响应头 `authorization` 里的 JWT | `Set-Cookie: sid=<32 位>`（登录响应体里**没有** token） |
| 认证传递 | `Authorization: <jwt>` | `Authorization: Bearer <sid>`（取空格分隔第 2 段；**该头一旦出现即完全覆盖 Cookie**） |
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
| A5 | Cookie 会话 ↔ Hinina 的 token 契约 | `restore_token` 存裸 sid，`send` 补 `Bearer ` 前缀（服务端不校验 scheme 名） |
| A6 | 登录响应体无 token | 直连 `HttpClient::client()` 读 `Set-Cookie`（并因 `Accept: application/json` 避免 302 丢失该头） |
| A7 | 无 `/user/me` | `X-Hydro-Inject: UserContext` 注入头 + `probe_user_context`/`current_user` 双入口 |
| A8 | 状态码值域不同（0–33 vs 0–15） | `types::map_status` 全表映射；`FETCHED(22)` 特意折入非终态的 `Pending` |
| A9 | 前端状态筛选传 HOJ 码 | `types::hoj_status_to_hydro` 单向翻译，无对应语义时明确报错 |
| A10 | 语言 key ↔ 显示名 | `types::LANG_TABLE` 双向静态表（29 项，双射由测试锁定），展示名刻意选前端能识别的前缀写法 |
| A11 | 记录无提交时间 | `types::objectid_seconds` 从 `_id`（时间型 ObjectId）前 4 字节反推，回退 `judgeAt` |
| A12 | 榜单是单元格矩阵 | `types::scoreboard_rank_page` **按表头 `type` 定位列**（不依赖列顺序）后逐格归一为 `RankCell` |
| A13 | 展示字母只存在于 `tdoc.pids` 下标 | `resolve_problem_id`（字母 → pid）+ `map_contest_problems`（下标 → 字母），带 60s 顺序表缓存避免请求放大 |
| A14 | 测试点无独立接口 | 复用 `/record/:rid` 的 `rdoc.testCases`，按 `subtaskId` 分组（不猜 `rdoc.subtasks` 结构） |
| A15 | `psdict` 以 docId 为键而入参是 pid | 经 `pdict` 反查 docId，未提交的题不出现在返回 map 中 |
| A16 | 比赛 ID 是 hex ObjectId | `Contest.id` 本就是 `String` ✓；题目/提交 ID 保持字符串 ✓ |
| A17 | 比赛阶段需客户端推算 | `contest_status(start, end, now)` 纯函数（时间缺失按「进行中」，避免误判为已结束） |
| A18 | 灵活时长模式 `endAt` 缺省 | 由 `beginAt + duration`（小时）推算 |

---

## 4. 设计缺口清单（未修改，待决断）

### A 档 — 必须改才能真正用起来

---

#### D1. `ContestProblem.cid: i64` 装不下 Hydro 的比赛 ID

- **现象**：Hydro 的比赛主键是 24 位 hex ObjectId（`"64f0c0f0f0f0f0f0f0f0f0f0"`），而 `ContestProblem.cid` 是 `i64`。
- **影响**：`list_contest_problems` 返回的每条题目都只能把 `cid` 填 **0**。当前前端不消费该字段（渲染只用 `displayId`/`problemId`），故**暂无功能故障**；但一旦有「按 cid 过滤题目」之类的逻辑，Hydro 侧会全部落到 0。
- **适配层现状**：填 0 并在代码注释中标注。
- **建议补丁**（entity 层，Breaking Change，需评估前端同步）：
  ```rust
  // core/entity/contest.rs
  pub struct ContestProblem {
      // pub cid: i64,
      pub cid: String,   // HOJ 的 cid 与 Hydro 的 ObjectId 都能承载
      ...
  }
  // 前端 src/types/contest.ts: cid: number → cid: string
  // HOJ 侧 adapter: cid: p.cid.to_string()
  ```
- **决策所需信息**：`cid` 目前无消费方 → 改动风险低；但它是跨端契约（`src/types/contest.ts`），需前后端同批改。

---

#### D2. `OjConfig.contest_id: i64` 装不下 Hydro 的比赛 ID

**状态：✅ 已解决（#20）** —— `OjConfig.contest_id: i64` 已被 `contest_ref: String` 取代
（不透明引用：HOJ 是数字串、Hydro 是 ObjectId），旧字段经 `legacy_contest_id` 迁移。
本条目保留为决策记录。

- **现象**：`oj.contestId` 是 `i64`（0 = 不自动加载），而 Hydro 的比赛 ID 是 hex 字符串。
- **影响**：**Hydro 无法使用「自动加载配置的比赛」这条路径**（`load_configured_contest` → 登录页比赛简报 → 进场）。当前 Hydro 只能靠 `list_contests` 取列表后在前端选（但前端也没有选择入口，见 D3）→ **Hydro 实际上无法进入比赛工作台**。
- **适配层现状**：无解 —— `contest_id` 由配置直接传给 `get_contest`/`list_contest_problems`，Adapter 拿到的是已序列化的 `i64`。
- **建议补丁**（配置层，同样跨端）：
  ```rust
  // core/entity/config.rs（OjConfig）
  // pub contest_id: i64,
  pub contest_id: String,   // "0" / "" = 不自动加载；HOJ 传 "123"，Hydro 传 ObjectId
  // 同步：sanitize() 的 .max(0) → trim；validate() 不加限制（两种形态都合法）
  // 前端 src/types/config.ts: contestId: number → string
  //      src/services/contest.service.ts 的 config.oj.contestId 消费点
  ```
- **备选方案**：新增 `hydro_contest_id: String` 字段，仅在当前 OJ 是 Hydro 时使用 —— 改动面更小但配置语义分裂（同一概念两个字段）。
- **决策所需信息**：这是**阻断 Hydro 可用性**的第一条，建议优先处理。

---

#### D3. 前端没有 OJ 切换入口（Hydro 无法从 UI 激活）

**状态：⚠️ 部分解决（#20）** —— `SettingsView` 现有 OJ 下拉（候选 = `oj.instances`，
`disabled` 的置灰）与显式 `switch_oj` 命令，`login` 不再承担切换副作用。
**残差**：下拉候选来自配置，前端**不能新增/删除实例**，`default_oj_instances()` 只给 HOJ
→ 启用 Hydro 仍需手改 `config.json`。详见 `适配新架构的冲突记录.md` §2.2。

- **现象**：① `SettingsView` 的「OJ」分组只有服务器地址（且只有 `hojUrl`）；② `LoginView` 调用 `auth.login(username, password)` **不传** `ojType`；③ 因此 `ProviderRegistry` 的当前 OJ 永远停在启动时的默认值。
- **影响**：即使后端注册了 HydroAdapter，用户也无法在界面上选择 Hydro。
- **适配层现状**：`AppContext::init` 改为从 `user.lastOjType` 解析当前 OJ（本次接线之一），因此**手改 `config.json` 的 `user.lastOjType: "Hydro"` + 填 `oj.hydroUrl` 即可激活**（已记 warn 提示地址为空的情况）。但这是开发者路径，不是选手路径。
- **建议补丁**（前端，3 处）：
  1. `src/types/config.ts`：`oj` 增加 `hydroUrl: string`；
  2. `SettingsView.vue` OJ 分组：增加「OJ 类型」选择器（`HOJ` / `Hydro`），切换时写 `user.lastOjType` 并提示需重启或重新登录；地址输入框按当前类型显示 `hojUrl` / `hydroUrl`；
  3. `LoginView.vue` / `authStore.login`：把当前 OJ 类型经 `ojType` 传给 `login` 命令（后端已支持该参数）。
- **决策所需信息**：本次任务范围明确排除前端改动，故仅留档。若确定要让 Hydro 真正可用，这是必须做的一步。

---

#### D4. 无域名（domain）配置项，只能访问系统域

**状态：⏸ 有配置位置但传不进适配器（#20 后）** —— `OjInstance.options` 已能承载
`{"domain": "myschool"}` 这类私有旋钮，但 `AdapterFactory::build(&self, deps, base_url)`
只把地址交给适配器，**options 没有传递通道**。要用起来需扩展 `build` 签名
（见 `适配新架构的冲突记录.md` §2.4）。若目标部署不用自定义域，本条可降为 C 档。

- **现象**：Hydro 多域部署的业务路由需要 `/d/:domainId` 前缀（系统域 `system` 可省略）。
- **影响**：若目标 Hydro 部署把比赛放在自定义域（如 `hydro.ac/d/myschool/...`），本适配器按系统域拼路径会 **404**。
- **适配层现状**：按系统域实现，类注释与文档均已标注。
- **建议补丁**：
  ```rust
  // core/entity/config.rs（OjConfig）
  pub hydro_domain: String,   // 默认 "" = 系统域；非空时路径前缀 /d/{domain}
  // adapter/hydro: fn url(&self, path) → if domain.is_empty() { base+path } else { format!("{}/d/{}{}", base, domain, path) }
  ```
  Adapter 侧改动很小（1 个 `url()` 方法 + 1 个构造参数），**风险主要在配置与前端**。
- **决策所需信息**：**若你的目标 Hydro 部署不用自定义域，本条可降为 C 档**（请确认）。

---

#### D17. `infra::http::HttpClient` 无法注入自定义请求头

**状态：✅ 已修复（#20）** —— `HttpClient` 的两个 text 变体现接收调用方构造的
`HeaderMap`（认证方式是 Adapter 层概念，infra 不做假设）。Hydro 的
`Accept: application/json` / `Authorization: Bearer <sid>` / `X-Hydro-Inject`
现在都在适配器里组装。

**但衍生出一条更窄的缺口**：infra 在**非 2xx 时丢弃响应体**，而 Hydro 的用户可见错误
全在响应体包络里（`LoginError` / `OpcountExceededError` / `PermissionError`）。
当前分工是 **GET 走 infra**（要回 5xx 重试）、**POST 走原始 reqwest**（要错误包络；
POST 本就不重试，不损失重试）。建议 infra 增加「任意状态码都返回 status + headers + body」
的变体，两条通道即可合一。详见 `适配新架构的冲突记录.md` §2.1。

<details>
<summary>原始记录（问题已修复，保留以说明背景）</summary>

- **现象**：Hydro 的 JSON 输出**完全依赖请求头** `Accept: application/json`（响应分支见文档 §1.5），另有 `X-Hydro-Inject: UserContext` 注入头；而 `HttpClient` 的 `get_text_with_headers` / `post_text_with_headers` **只能注入 `Authorization`**。
- **影响**：① Hydro 适配器只能绕过 `HttpClient` 直连 `reqwest::Client`；② **代价是失去 infra 的 5xx 退避重试**（`MAX_RETRIES = 2`，1s/2s 指数退避）与统一的 `status_error` 映射 —— 后者已在 Adapter 内以 `http_status_error` 镜像（判据一致：401→`Auth`、403→`Network`），前者**没有补偿**（判定为「重试属 infra 职责，不在 Adapter 重复实现」）。
- **影响评估**：客户端是高频轮询型（榜单 10s、题目总览 30s），单次 5xx 失败只影响一个刷新周期；但 `ProblemService::load_problem_limits` 的批量拉取中，一次 5xx 会让该题 limits 被跳过（已有「部分失败跳过」语义兜底）。**不构成功能阻断，但比 HOJ 路径的健壮性低一档**。
- **建议补丁**（infra 层，向后兼容）：
  ```rust
  // infra/http.rs —— 新增一个可传任意请求头的变体，现有方法保持不动
  pub async fn get_text_with_headers_and(
      &self,
      url: &str,
      headers: &[(&str, &str)],
      auth_token: Option<&str>,
  ) -> AppResult<(String, reqwest::header::HeaderMap)> { /* retry_get 增加 headers 形参 */ }
  pub async fn post_text_with_headers_and<B: Serialize>(...) -> AppResult<(...)> { ... }
  ```
  改完后 `adapter/hydro` 的 `send()` 可回归 infra，**顺带恢复 5xx 重试**；`retry_get` 需把 headers 透传进去（改动集中在 infra/http.rs 一个文件）。
- **决策所需信息**：这是**唯一的「设计确实需要改」结论**（其余都是可选增强）。是否现在改由你定：不改也能跑，改了更稳。

</details>

---

### B 档 — 建议改（有损但可用）

---

#### D5. `Problem` 没有「题面格式」判别位

- **现象**：Hydro 的 `pdoc.html: bool` 标明 `content` 是 HTML 还是 Markdown；Hinina 的 `Problem.description` 只有字符串，前端**一律按 Markdown 渲染**（`utils/markdown.ts`）。
- **影响**：若某题面是 HTML（`html: true`），前端会把它当 Markdown 渲染 —— 标签被转义或结构错乱（DOMPurify 消毒后仍会显示为纯文本标记）。
- **适配层现状**：原样透传 `content` 到 `description`（Markdown 场景完全正确；HTML 场景降级显示）。
- **建议补丁**：
  ```rust
  // core/entity/problem.rs
  pub struct Problem {
      ...
      /// 题面内容格式："markdown"（默认）/ "html"
      #[serde(default = "default_markdown")]
      pub description_format: String,
  }
  // 前端 utils/markdown.ts 增加 renderProblemContent(content, format) 分流（HTML 走同一套 DOMPurify 消毒）
  ```
- **决策所需信息**：取决于目标 Hydro 部署是否使用 HTML 题面（Hydro 后台默认 Markdown，HTML 是可选）。

---

#### D6. Hydro 的 JSON 没有样例字段 → `Problem.samples` 恒空

- **现象**：Hydro 把样例写在题面 Markdown 的代码块里，JSON 里没有结构化样例（`pdoc.data` 只是测试数据文件列表，需额外权限）。
- **影响**：题面下方的「样例」区块（含复制按钮）在 Hydro 下不可用；样例仍能在题面 Markdown 中看到（只是没有独立区块与复制功能）。
- **适配层现状**：`samples` 返回空 `Vec`，前端自然不渲染该区块（无报错）。
- **建议补丁（二选一）**：
  - 前端侧：题面渲染时提取围栏代码块生成样例卡片（不改 entity，纯前端启发式）；
  - 或新增 Provider 能力 `list_problem_samples`（需要新的 trait 方法 → 属框架改动）。
- **决策所需信息**：属体验增强，不影响正确性。

---

#### D8. `/record` 不返回总条数、无 limit/uid 参数

- **现象**：Hydro 记录列表的响应只有 `page` + `rdocs`，**没有 total/pages**；页大小由服务端 `pagination.record` 决定（默认 100），客户端不能指定；用户筛选用 `uidOrName`（不是 `uid`）。
- **影响**：`SubmissionPage.total/pages` 只能推导 —— 当前实现按 `ASSUMED_RECORD_PAGE_SIZE = 100` 计算：`total = (page-1)*100 + 本页条数`，`pages = 本页满 100 时 page+1 否则 page`。若部署改了 `pagination.record`，分页控件会出现「多一页空页」或「少一页」的偏差。
- **适配层现状**：如上（`only_mine` 恒真 + 选手通常 <100 条提交时，实际表现为单页，偏差不可见）。
- **建议补丁**：无（协议限制）。若要精确，只能改用 `stat=true`（需 `PRIV_VIEW_JUDGE_STATISTICS` 特权，选手没有）。**建议接受**。

---

#### D9. `JudgementStatus` 缺 Hydro 专属状态

- **现象**：Hydro 的 9 CANCELED / 11 HACKED / 30 IGNORED / 32 HACK_SUCCESSFUL / 33 HACK_UNSUCCESSFUL 在 HOJ 的 18 个变体里没有对应项。
- **影响**：这些状态在前端显示为 **"Unknown"**（Hydro 网页端分别显示 "Cancelled"/"Hacked"/"Ignored"/…），文案落差。
- **适配层现状**：`map_status` 折入 `Unknown`（**刻意不猜**：错映射会误导选手判断自己的提交结果）。另 31 FORMAT_ERROR 取语义最近的 `PresentationError`。
- **建议补丁**（entity + 前端，需同步）：
  ```rust
  // core/entity/submission.rs
  pub enum JudgementStatus {
      ...,
      Cancelled, Ignored, Hacked, FormatError, HackSuccessful, HackUnsuccessful,
  }
  // 前端 src/utils/submission.ts 的 STATUS_META 补 6 项文案/缩写/色调（Record<JudgementStatus, ...> 会强制补齐）
  // 前端 src/types/submission.ts 的联合类型同步
  ```
- **决策所需信息**：这些状态在**校赛 ACM 场景极少出现**（CANCELED 只在管理员取消时、HACK 只在 Hack 赛制）。**可接受**，建议暂不动。

---

#### D10. 榜单是预渲染单元格矩阵，`RankQuery` 基本失效

- **现象**：Hydro 榜单返回 `rows`（表头 + 单元格），**服务端不支持**分页/关键词搜索/移除打星/赛后提交；每题单元格只给展示文本与 `score`。
- **影响**（逐项）：
  - `RankQuery.current_page` / `limit` → 忽略，返回单页全量（`pages = 1`）；
  - `keyword` → 忽略（前端工具条的搜索框对 Hydro 无效，会静默返回全量）；
  - `remove_star` → 忽略（Hydro 的打星只能在客户端过滤，前端「全量快照模式」本就在客户端过滤，故实际可用）；
  - `contains_end` → 忽略（Hydro 的赛后可见性由赛制 `showRecord` 决定，客户端无法在请求里切换）；
  - ACM 单元格的 `error_num` / `ac_time` / `is_first_ac` / `try_num` **靠文本与 `style` 反推**（`score == 100` 判 AC、首行 `+n`/`-n`/✓ 取失败次数、橙色 span 取待判次数、`style` 非空判首 A）；
  - OI 家族每题只有分数 → `time_info`（最优耗时）恒空、`RankCell.error_num` 恒 0；
  - `ContestRankRow.gender` / `school` / `nickname` → Hydro 榜单投影无这些字段 → 前端「女生队高亮」不可用；
  - `total`（ACM 总提交数）由各题尝试次数求和反推。
- **适配层现状**：如上，全部在 `types::scoreboard_rank_page` 内完成，并有 3 行（普通/打星/封榜待判）的逐字段测试锁定。
- **建议补丁**：无（协议限制）。**建议接受**，但应告知用户：Hydro 下榜单搜索框无效、女生队高亮不可用。

---

#### D11. Hydro 没有公告接口（对应能力是「答疑」）

- **现象**：Hydro 只有 `/contest/:tid/clarification`（提问 + 裁判回复的会话），没有单向广播的公告。
- **影响**：公告页在 Hydro 下恒为空（不报错）。
- **适配层现状**：`list_announcements` 返回空 `AnnouncementPage`，方法文档写明了原因与后续方案。
- **本次决策记录（已与项目负责人确认）**：**置空**。后续若要支持，应由**前端按 OJ 能力自动把「公告」面板切换为「问答」形态**；但 HOJ 是第一优先级的 OJ，短期不为 Hydro 改前端，故先留空并在此留档。
- **建议补丁（未来）**：不要在本方法里做有损映射（把定向回复伪装成全场公告会让选手误读），而应在 Provider 层新增「答疑」能力（新 trait 方法或新实体），由前端按能力切换面板。

---

#### D12. 比赛隐藏本人记录时，提交只返回 `tid` 而非 `rid`

- **现象**：Hydro 的 `POST /p/:pid/submit` 在 `tid && !pretest && !contest.canShowSelfRecord(...)` 时返回 `{tid}` 而不是 `{rid}`。
- **影响**：客户端拿不到记录 ID → **无法轮询评测结果**（提交成功但看不到结果）。ACM 赛制下 `showSelfRecord` 恒真，故常规校赛不受影响；`oi`/`strictioi` 等赛制在特定配置下会命中。
- **适配层现状**：明确报错「Hydro 未返回评测记录 ID：本场比赛隐藏本人评测记录，无法查询评测结果」——**优于静默返回空串**（后者会让前端轮询一个空 ID 到超时）。
- **建议补丁**：无（协议限制）。若确需支持，只能在前端给出「已提交，本场比赛隐藏本人记录」的专门提示。

---

#### D18. 部署不支持 `X-Hydro-Inject` 时的降级策略

- **现象**：`X-Hydro-Inject: UserContext` 是获取当前用户的**唯一**途径（无 `/user/me`）。若目标部署版本不支持该头，响应里就没有 `UserContext` 字段。
- **影响**：此时无法区分「匿名（sid 失效）」与「不支持注入头」。若按前者处理 → **一次版本差异会把全部在线选手踢回登录页**。
- **适配层现状**：`probe_user_context` 对「字段缺失」返回 `Err(Unknown)`（= 无法判定 → 保留会话），只有「注入生效且 `_id == 0`」才判定失效。`validate_session` 因此只会因**明确**的匿名而登出。代价：若部署真的不支持注入头，会话将永远校验为 `unknown`（保留登录态但也不会主动登出），需靠 `sessionGuard` 的认证类 IPC 失败兜底。
- **建议补丁**：无（已是最保守的正确处置）。**建议联调时优先验证该头**（见 §6 待办 1）。

---

### C 档 — 可接受 / 仅备案

---

#### D7. 记录无提交时间字段（已用 ObjectId 反推）

- **现象**：`rdoc` 只有 `judgeAt`（评测完成时刻），没有提交时刻。
- **适配层现状**：`objectid_seconds` 从 `_id` 前 4 字节反推（Hydro 自身也依赖 ObjectId 的时间有序性做跨域时间过滤）。**前提**：部署使用标准 MongoDB ObjectId（24 位 hex）。
- **风险**：若部署换了 ID 方案，反推会返回 `None` 并回退 `judgeAt`（≈ 提交时间 + 评测耗时，通常差几秒）。已在测试中锁定「非 24 位 hex / 时间越界一律拒绝」。
- **建议**：无需改动；联调时核对一次时间显示（§6 待办 2）。

---

#### D13. `SubmissionQuery.status` 承载 HOJ 状态码

- **现状**：前端状态下拉的 value 是 HOJ 码，Adapter 经 `hoj_status_to_hydro` 单向翻译；HOJ 的 PE(3)/RJE(11)/SF(12)/PA(13)/FREQ(14) 在 Hydro 无对应语义 → **明确报错**而不是静默忽略筛选。
- **影响**：Hydro 用户选这些筛选项会看到一条明确错误提示。
- **建议补丁（未来）**：前端按当前 OJ 过滤下拉候选（需前端改动）。**可接受**。

---

#### D14. `HttpClient` 的 Cookie jar 是全局共享的

- **现状**：`HttpClient::with_timeout` 启用 `cookie_store(true)`，jar 在整个进程内共享。Hydro 登录后 `sid` 会留在 jar 里。
- **影响**：当前注册中心只有一个「当前 OJ」，且不同 OJ 通常不同主机，**无实害**。但若未来支持「多 OJ 同时登录」，jar 共享可能造成会话串扰（尤其同一主机的不同部署）。
- **建议**：暂不处理；若引入多 OJ 并存，应改为每 Adapter 一个 client（或显式关闭自动 cookie 存储，因为 Hydro 侧我们已显式发送 `Authorization`）。

---

#### D15. Hydro 的 pid 可以是字符串（`P1000` / `A1`）

- **现状**：`Problem.id` / `SubmissionRecord.pid` / `display_pid` 本就是 `String` ✓；`ContestProblem.id: i64` 取数字 `docId` ✓。
- **唯一歧义**：`resolve_problem_id` 把**单字母**入参当展示字母解析（依据 Hydro `record_main` 的规则）。若某题的**真实 pid 恰好是一个字母**（如 `"A"`）且它不是该比赛的第一题，`get_problem` 会取到错误的题面。
  - 缓解：`submit` 路径**不做**该换算（提交永远收真实 pid），故**不会提交到错误的题**；影响仅限题面展示，且顺序表缺失时会回退原值并记 warn。
- **建议**：无需改动；联调时留意（§6 待办 3）。

---

#### D16. Hydro 全局限流 100 请求 / 5 秒

- **现状**：每个 Handler 都跑 `limitRate('global', 5, 100)`（key = ip@user）；登录另有 60s/30 与 60s/5；超限抛 `OpcountExceededError`(403)。
- **影响**：① 榜单「全量快照模式」（打星/女生队过滤）会顺序拉全部分页（前端上限 40 页）—— Hydro 榜单不分页，**实际只有 1 个请求**，风险消失；② 题目 limits 批量拉取（并发 4、每题 1 次 `get_problem`）已通过 60s 顺序表缓存把 `get_problem` 的额外开销降到 0；③ 若未来前端轮询节奏加密，需注意。
- **适配层现状**：`OpcountExceededError` 被判为**业务错误**（不触发登出），消息里带错误名与 params，便于识别。
- **建议**：无需改动。

---

#### D19. 三个 HOJ 专属字段在 Hydro 下无来源

| 字段 | Hydro 情况 | 适配层填值 | 影响 |
|---|---|---|---|
| `Contest.auth`（0 公开 / 1 私有 / 2 保护） | 可见性由 `tdoc.assign`（组限定）表达，且该字段不在投影里 | 固定 `0` | 前端未消费该字段，无影响 |
| `Contest.rank_show_name`（榜单显示名规则） | Hydro 用 `displayName` 且仅管理员可见 | `""`（前端回退 username） | 无影响 |
| `Contest.allow_end_submit` | 赛后可见性由赛制 `showRecord` 决定，请求不可切换 | `false` | `contains_end` 本就被忽略（D10） |
| `SubmissionDetail.oi_rank_score` | 无该字段 | `None` | 前端未消费 |

- **建议**：无需改动（这些字段的语义是 HOJ 专属，Adapter 填「安全默认值」是正确做法）。

---

#### D20. 前端语言域缺 Bash / Haskell 的扩展名与高亮映射

- **现状**：Hydro 提供 `bash` / `hs`（Haskell）等语言；Adapter 译为 `"Bash"` / `"Haskell"` 后，前端 `utils/language.monacoIdStrict` 无法识别 → Monaco 高亮回退 `cpp`、工作区源文件名回退 `main.txt`。
- **影响**：用 Bash/Haskell 提交时，工作区文件名是 `main.txt`、高亮是 C++。**提交本身不受影响**（`lang` 由 Adapter 从显示名反查回 key，往返恒等）。
- **建议补丁（前端）**：`utils/language.ts` 的 `monacoIdStrict` / `HOJ_LANGUAGE_BY_EXT` 补 `bash→.sh` / `haskell→.hs`（`hs` 已在扩展名表里，但 `monacoIdStrict` 不认 "Haskell" 前缀）。
- **决策所需信息**：校赛以 C/C++ 为主，**可接受**。

---

## 5. 对既有代码的改动

### 5.1 本轮（适配新架构后）：只剩 2 行

| 文件 | 改动 |
|---|---|
| `adapter/mod.rs` | `pub mod hydro;` + `factories()` 里加 `&hydro::FACTORY` |
| `adapter/hydro/**` | 新增（工厂 + HTTP 入口按通道分工 + `TtlCache`） |

**这正好验证了 #20 的价值**：接入一个新 OJ 不再触碰 `core/provider`、`core/entity/config.rs`、
`core/context.rs`、`commands/auth_cmd.rs` —— 只在适配器目录与工厂清单里各加一处。

### 5.2 上一轮（#20 之前）的 4 处接线：已全部作废

以下改动曾用于接入 Hydro，现已被 #20 的实现取代（更彻底，故未保留）：

| 文件 | 当时的改动 | 现状 |
|---|---|---|
| `core/provider/oj_type.rs` | 新增 `Hydro` 变体 + `OJType::from_name` | 文件已删除 → `oj_id.rs` |
| `core/entity/config.rs` | `OjConfig` 新增 `hydro_url` + 两处校验分支 | 被 `oj.instances` 取代 |
| `core/context.rs` | 15 行注册块 + 按 `lastOjType` 解析当前 OJ | 被工厂循环取代 |
| `commands/auth_cmd.rs` | `parse_oj_type` 委托 `OJType::from_name`，支持 `HYDRO` | 被 `OjId` + `oj_cmd::switch_oj` 取代 |

---

## 6. 待联调清单（本次无可用 Hydro 实例）

所有单元测试基于**按文档手工构造的夹具**（`adapter/hydro/tests/fixtures/`），以下必须在真实部署上验证：

1. **`X-Hydro-Inject: UserContext` 是否生效**（最关键）：决定会话校验能否区分「匿名」与「无法判定」。验证方式：登录后抓 `GET /` 的响应体，确认含 `UserContext` 且 `_id` 为真实 uid。
2. **提交时间**：核对记录页显示的提交时刻与 Hydro 网页端一致（验证 ObjectId 反推）。
3. **展示字母解析**：在真实比赛里逐题打开题面，确认 `resolve_problem_id` 的字母 → pid 换算正确（尤其题目数 >26 或 pid 形如 `A1` 的场景）。
4. **榜单单元格形态**：确认 `rows` 的列顺序、AC 单元格 `score == 100`、首 A 的 `style`、封榜的橙色 span 与实现假设一致（这是反推最多的一处）。
5. **登录响应头**：确认 `Set-Cookie: sid=...` 出现在带 `Accept: application/json` 的 200 响应上（而非 302 重定向）。
6. **错误包络**：确认错误响应的 HTTP 状态码等于 `error.code`，且未登录场景确实表现为 HTTP 200 + `{"url":"/login?..."}`。
7. **限流**：确认 `OpcountExceededError` 的触发阈值与前端轮询节奏无冲突。
8. **`tdoc.pids` 在 `/contest/:tid/problems` 响应里是否存在**（实现优先用响应自带的，缺失时回退带缓存的 `/contest/:tid`）。

---

## 7. 建议的决策顺序（2026-09-19 更新）

1. **D17 的衍生缺口**：infra 在非 2xx 时丢弃响应体 → 建议新增「任意状态码都返回
   status + headers + body」的变体。收益明确（GET 也能读错误包络，两条通道可合一），
   风险极低（新增方法、不动既有）。见 `适配新架构的冲突记录.md` §2.1；
2. **D3 的残差**：设置页支持新增/删除 OJ 实例 —— 决定 Hydro 能否被选手**自助**启用
   （现在只能手改 `config.json`）。见冲突记录 §2.2；
3. **D4**：若目标部署用自定义域，需让 `AdapterFactory::build` 能读到 `OjInstance.options`
   （签名扩展）；不用自定义域的话本条可降为 C 档。见冲突记录 §2.4；
4. **D1**：`ContestProblem.cid` 仍是 `i64`，装不下 hex 比赛 ID（当前无消费方，影响为零）；
5. D5 / D6 / D8 / D9 / D10 / D11 / D12 / D18 属**已知降级**，建议在 README 或用户文档里
   明示 Hydro 的能力边界，而不是现在改代码；
6. D7 / D13–D16 / D19 / D20 备案即可。

**顺带一条文档同步建议**：`core/entity/submission.rs` 的 `is_terminal()` 注释写着
「三处判据必须保持一致」，实际已有四处（含 `adapter::hydro::types::is_terminal_status`）。见冲突记录 §2.3。
