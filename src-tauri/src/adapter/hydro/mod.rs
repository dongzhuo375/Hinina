// Hydro Adapter：实现 Auth + Contest + Problem + Submission 四个 trait。
//
// 目标对象是**上游 Hydro OJ**（`packages/hydrooj`），与 `adapter/hoj` 的 HOJ
// （Hydro 衍生版，自带 REST + JWT 层）是两套不同协议，故不能复用 HOJ 的调用方式：
//
//   1. 走**传统 Handler 路由 + `Accept: application/json`** 取 JSON
//      （Hydro 没有统一 REST API；JSON-RPC `/d/:domainId/api/:op` 只注册了
//       user/users/domain/problem 几个查询，没有题目列表、记录查询与题目状态）。
//   2. **响应无统一包络**：成功即原始 body；失败为
//      `{"error":{"name","params","code"}}`；未登录在 JSON 模式下可能是
//      HTTP 200 + `{"url":"/login?redirect=..."}`（重定向被 JSON 化）。
//   3. 会话是 **Cookie `sid`**，也可经 `Authorization: Bearer <sid>` 传递
//      （服务端取空格分隔第 2 段，且该头一旦出现即完全覆盖 Cookie）
//      —— 与 Hinina 的 token 契约天然 1:1，`restore_token` 可直接复用。
//   4. **没有 `/user/me`**：当前用户靠 `X-Hydro-Inject: UserContext`
//      把上下文注入到任意路由的响应体。
//
// 所有 OJ 私有语义都收敛在本模块与 `types`（响应归一化归 Adapter，infra 只传字节）。

pub mod error;
pub mod types;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION};
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use tracing::{debug, info, warn};

use crate::adapter::{AdapterDeps, AdapterFactory};
use crate::core::entity::announcement::AnnouncementPage;
use crate::core::entity::contest::{Contest, ContestProblem};
use crate::core::entity::problem::Problem;
use crate::core::entity::rank::{ContestRankPage, RankQuery};
use crate::core::entity::submission::{
    JudgeCase, JudgementResult, SubmissionCases, SubmissionDetail, SubmissionPage, SubmissionQuery,
    SubmissionRecord, SubTaskCases,
};
use crate::core::entity::user::User;
use crate::core::error::{AppError, AppResult};
use crate::core::provider::auth::AuthProvider;
use crate::core::provider::contest::ContestProvider;
use crate::core::provider::problem::ProblemProvider;
use crate::core::provider::registry::ProviderSet;
use crate::core::provider::submission::SubmissionProvider;
use crate::infra::cache::TtlCache;
use crate::infra::http::HttpClient;

use self::types::{
    ContestDetailVO, ContestListVO, ContestProblemListVO, PdocVO, ProblemDetailVO, RdocVO,
    RecordDetailVO, RecordListVO, ScoreboardVO, SubmitVO, TdocVO, UserBriefVO,
};

/// 强制 JSON 输出的内容协商头（Hydro 的响应分支见文档 §1.5）
const JSON_ACCEPT: &str = "application/json";
/// 上下文注入头（小写比对 `usercontext`，值不区分大小写）
const INJECT_HEADER: &str = "X-Hydro-Inject";
const INJECT_USER_CONTEXT: &str = "UserContext";
/// 比赛列表翻页上限（防御：`tpcount` 异常时不至于无限翻页）
const MAX_CONTEST_PAGES: i64 = 20;
/// 比赛题目顺序（tid → docId 列表）缓存时长。
///
/// 存在的理由：前端「题目 limits 渐进填充」会对每道题调一次 `get_problem`，
/// 而展示字母 → pid 的换算需要 `tdoc.pids`；不缓存就会为一场 12 题的比赛
/// 重复发 12 次 `/contest/:tid`（Hydro 全局限流仅 100 请求/5 秒）。
const PID_CACHE_TTL: Duration = Duration::from_secs(60);
/// 顺序表缓存容量上限（比赛数；一个会话内远用不到这么多，仅作上界防御）
const PID_CACHE_CAPACITY: usize = 32;
/// 假定评测记录每页条数（Hydro 的页大小由服务端 `pagination.record` 决定，默认 100，
/// 客户端无法查询；仅用于推导分页控件，见缺口 D8）
const ASSUMED_RECORD_PAGE_SIZE: i64 = 100;

/// 当前登录用户的轻量身份（Hydro 无 `/user/me`，靠注入头获得）
#[derive(Debug, Clone, PartialEq, Eq)]
struct HydroUser {
    id: String,
    uname: String,
}

/// Hydro OJ 适配器。
///
/// 实现 `AuthProvider`、`ContestProvider`、`ProblemProvider`、`SubmissionProvider`。
/// 不持有 `EventBus`：Hydro 没有 HOJ 的 `Refresh-Token` 轮换协议，sid 由服务端
/// 滑动续期且值不变，没有「凭证轮换需回写磁盘」的场景。
pub struct HydroAdapter {
    http: Arc<HttpClient>,
    /// 不含尾部斜杠的站点根地址，如 `https://hydro.ac`
    base_url: String,
    /// 会话 ID（Cookie `sid`）；即 `User.token` 与磁盘会话里保存的凭证
    session: RwLock<Option<String>>,
    /// 当前用户身份缓存（登录/上下文探测时写入，用于 `uidOrName` 筛选与榜单我的行）
    user: RwLock<Option<HydroUser>>,
    /// 比赛题目顺序缓存（tid → docId 列表）。
    ///
    /// 用 infra 的 `TtlCache`（而非手写 `RwLock<HashMap<…, Instant>>`）：TTL 判定与
    /// 容量淘汰由原语负责。键是比赛 ID —— 天然带 OJ 作用域，因为适配器实例由工厂
    /// **按实例构造**（不同 OJ 不共用实例，不存在跨 OJ 串号）。
    pid_cache: TtlCache<String, Vec<String>>,
}

/// HydroResponse 只由 infra 的 raw 变体产生（任意状态码都返回原始响应）
struct HydroResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: String,
    url: String,
}

impl HydroResponse {
    /// 把响应体解析成 JSON，并在解析前完成协议层判定。
    ///
    /// 判定顺序刻意是「状态码 → 错误包络 → 登录重定向 → 类型化解析」：
    /// 先看状态码才能把网关的 HTML 错误页报成 `HTTP 502` 而不是
    /// 「响应不是合法 JSON」（把排障引向错误方向）；再认错误包络是因为
    /// Hydro 用 HTTP 状态码承载 `error.code`，但登录重定向却是 HTTP 200。
    fn into_value(self) -> AppResult<Value> {
        if !self.status.is_success() {
            if let Ok(value) = serde_json::from_str::<Value>(&self.body) {
                let mut value = value;
                types::strip_nulls(&mut value);
                if let Some(err) = types::parse_hydro_error(&value) {
                    return Err(err);
                }
            }
            return Err(http_status_error(&self.url, self.status));
        }
        Self::parse_value(&self.body, &self.url)
    }

    /// 协议层判定 + 解析（GET / POST 两条通道共用）。
    ///
    /// ① 非法 JSON（通常是网关 HTML 错误页）→ `Serialization` 并带 URL 与响应体前
    /// 200 字符；② 错误包络 → `AppError`（变体判定在 `error` 模块）；
    /// ③ **JSON 化的登录重定向**（`{"url":"/login?…"}`，HTTP 200）→ `Auth`。
    fn parse_value(body: &str, url: &str) -> AppResult<Value> {
        let mut value: Value = serde_json::from_str(body).map_err(|e| {
            AppError::Serialization(format!(
                "Hydro 响应不是合法 JSON {}: {}（响应前 200 字符: {}）",
                url,
                e,
                types::preview(body)
            ))
        })?;
        types::strip_nulls(&mut value);

        if let Some(err) = types::parse_hydro_error(&value) {
            return Err(err);
        }
        if let Some(redirect) = types::login_redirect_url(&value) {
            // 未登录的 JSON 化重定向：必须报成 Auth，否则前端会把
            // 「会话过期」当成「成功但数据为空」，永远回不到登录页
            warn!(url = %url, redirect = %redirect, "Hydro 返回登录重定向");
            return Err(AppError::Auth(format!(
                "Hydro 登录状态已失效，请重新登录（{}）",
                redirect
            )));
        }
        // 域相关重定向（`{"url":"/d/..."}`）：不识别会漏进 DTO 解析并报成
        // 「响应字段不匹配」，把配置问题伪装成 DTO 问题
        if let Some(err) = types::domain_redirect_error(&value) {
            return Err(err);
        }
        Ok(value)
    }

    /// 反序列化为目标 DTO
    fn into_json<T: DeserializeOwned>(self) -> AppResult<T> {
        let url = self.url.clone();
        let value = self.into_value()?;
        serde_json::from_value::<T>(value).map_err(|e| {
            AppError::Serialization(format!("Hydro 响应字段不匹配 {}: {}", url, e))
        })
    }
}

/// HTTP 状态码 → `AppError`（镜像 `infra::http::status_error` 的判据）。
///
/// 本适配器两条通道都走 infra 的 **raw** 变体（任意状态码都返回原始响应，为了读
/// 错误包络），而 raw 变体**不做状态码映射** —— 故这一步由适配器自己承担，
/// 判据与 infra 保持一致：**401 → `Auth`**（标准语义即「未认证」，前端
/// `sessionGuard` 与三态会话校验都依赖它）、**403 保持 `Network`**
/// （可能是「无权访问某场私有赛」这类业务限制，误判会把已登录选手踢回登录页）。
fn http_status_error(url: &str, status: StatusCode) -> AppError {
    let msg = format!(
        "HTTP {} {}: {}",
        status.as_u16(),
        status.canonical_reason().unwrap_or(""),
        url
    );
    if status == StatusCode::UNAUTHORIZED {
        return AppError::Auth(msg);
    }
    AppError::Network(msg)
}

/// 当前 UTC 秒级时间戳
fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl HydroAdapter {
    /// Hydro 的 OJ 身份标识（会话文件名 = `sessions/Hydro.json`）。
    ///
    /// 与 `AdapterFactory::id()` 必须一致：它同时决定会话文件路径与配置实例的
    /// 匹配键（`oj.instances[].id`）。Hydro 无历史会话文件，不需要迁移。
    pub const ID: &'static str = "Hydro";

    /// 创建 HydroAdapter。
    ///
    /// `base_url` 是站点根地址（**不含**尾部斜杠），如 `https://hydro.ac`。
    /// 多域部署的 `/d/:domainId` 前缀暂不支持（缺口 D4）：本适配器按**系统域**
    /// （`system`）拼路径 —— 且 `AdapterFactory::build` 目前只把 `base_url`
    /// 交给适配器，`OjInstance.options` 里的域名配置还传不进来（见冲突记录）。
    pub fn new(http: Arc<HttpClient>, base_url: String) -> Self {
        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            session: RwLock::new(None),
            user: RwLock::new(None),
            pid_cache: TtlCache::new(PID_CACHE_TTL, PID_CACHE_CAPACITY),
        }
    }

    /// 拼接完整 URL（路径以 `/` 开头）
    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    // ── 身份状态 ──

    fn session(&self) -> Option<String> {
        self.session.read().ok()?.clone()
    }

    fn set_session(&self, sid: Option<String>) {
        if let Ok(mut guard) = self.session.write() {
            *guard = sid;
        }
    }

    fn cached_user(&self) -> Option<HydroUser> {
        self.user.read().ok()?.clone()
    }

    fn set_user(&self, user: Option<HydroUser>) {
        if let Ok(mut guard) = self.user.write() {
            *guard = user;
        }
    }

    /// 清空内存身份（登出 / 服务端判定会话失效）
    fn clear_identity(&self) {
        self.set_session(None);
        self.set_user(None);
    }

    /// 断言已持有会话（提交等必须登录的操作的前置校验）
    fn require_session(&self) -> AppResult<()> {
        if self.session().is_none() {
            return Err(AppError::Auth("请先登录".into()));
        }
        Ok(())
    }

    // ── 请求入口 ──

    /// 构造本次请求的请求头。
    ///
    /// 三样都由 Hydro 协议决定，故必须在 Adapter 层组装（infra 不感知认证方式）：
    /// - `Accept: application/json` —— **不带它 Hydro 会渲染 HTML 模板**（文档 §1.5
    ///   的响应分支），这是取 JSON 的唯一手段；
    /// - `Authorization: Bearer <sid>` —— Hydro 取空格分隔的第 2 段（scheme 名不参与
    ///   校验），且**该头一旦出现即完全覆盖 sid Cookie**，故只在持有会话时附加；
    /// - `X-Hydro-Inject: UserContext` —— 把当前用户注入响应体（Hydro 无 `/user/me`）。
    fn headers(&self, inject_user: bool) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static(JSON_ACCEPT));
        if inject_user {
            headers.insert(INJECT_HEADER, HeaderValue::from_static(INJECT_USER_CONTEXT));
        }
        if let Some(sid) = self.session() {
            if let Ok(value) = HeaderValue::from_str(&format!("Bearer {}", sid)) {
                headers.insert(AUTHORIZATION, value);
            }
        }
        headers
    }

    /// GET 并返回原始响应（**走 infra 的 raw 变体**：任意状态码都拿到 status + headers + body）。
    ///
    /// 用 raw 而不是 `get_text_with_headers` 的原因：infra 的非 raw 变体在非 2xx 时
    /// **丢弃响应体**，而 Hydro 的用户可见错误全在响应体包络里
    /// （`{"error":{"name","params","code"}}`，无 message）—— 丢掉就只能给用户看
    /// 「HTTP 403」。代价是 401 不会自动变成 `Auth` 变体，故本适配器自己映射
    /// （见 `http_status_error`，判据与 infra 的 `status_error` 一致）。
    ///
    /// 重试由 infra 负责：5xx 退避重试（GET 幂等），耗尽后返回响应而非报错。
    async fn get_raw(&self, path: &str, inject_user: bool) -> AppResult<HydroResponse> {
        let url = self.url(path);
        let (status, headers, body) = self
            .http
            .get_text_raw(&url, &self.headers(inject_user))
            .await?;
        Ok(HydroResponse {
            status,
            headers,
            body,
            url,
        })
    }

    /// GET 并解析为 JSON
    async fn get_value(&self, path: &str, inject_user: bool) -> AppResult<Value> {
        self.get_raw(path, inject_user).await?.into_value()
    }

    /// GET 并反序列化为目标 DTO
    async fn get_json<T: DeserializeOwned>(&self, path: &str, inject_user: bool) -> AppResult<T> {
        let url = self.url(path);
        let value = self.get_value(path, inject_user).await?;
        serde_json::from_value::<T>(value)
            .map_err(|e| AppError::Serialization(format!("Hydro 响应字段不匹配 {}: {}", url, e)))
    }

    /// POST 并返回原始响应（含状态码与响应头）。
    ///
    /// 同样走 infra 的 raw 变体，两个理由：① 登录/提交失败的**用户可见文案在
    /// 错误包络里**（`LoginError` / `OpcountExceededError` / `PermissionError`）；
    /// ② 登录要读响应头里的 `Set-Cookie: sid`（会话就在那里下发）。
    /// POST 为非幂等方法，infra 不做重试，与语义相符。
    ///
    /// 无 body 的 POST（登出）发 `{}` 而不是空请求体：Hydro 的参数装饰器读
    /// `this.request.body`，空对象等价于「无参数」，比 `null` 更不容易触发解析歧义。
    async fn post_raw(&self, path: &str, body: Option<&Value>) -> AppResult<HydroResponse> {
        let url = self.url(path);
        let payload = body.cloned().unwrap_or_else(|| json!({}));
        let (status, headers, body) = self
            .http
            .post_text_raw(&url, &payload, &self.headers(false))
            .await?;
        Ok(HydroResponse {
            status,
            headers,
            body,
            url,
        })
    }

    /// POST 并解析为 JSON（协议层判定见 `HydroResponse::into_value`）
    async fn post_json<T: DeserializeOwned>(
        &self,
        path: &str,
        body: Option<&Value>,
    ) -> AppResult<T> {
        self.post_raw(path, body).await?.into_json()
    }
    // ── 用户上下文 ──

    /// 探测服务端眼中的当前用户（严格版）。
    ///
    /// 三种结果严格区分（`AuthProvider::validate_session` 的三态契约依赖它）：
    /// - `Ok(Some)` —— 服务端确认已登录；
    /// - `Ok(None)`  —— 注入生效但 `_id == 0`，**服务端明确判定为匿名**（sid 已失效）；
    /// - `Err(_)`    —— 无法判定：网络异常、错误包络（可能映射为 `Auth`）、
    ///   或**响应里根本没有 `UserContext` 字段**（该部署不支持注入头）。
    ///
    /// 最后一条尤其重要：若把「不支持注入头」当成「匿名」，一次版本差异就会把
    /// 全部在线选手踢回登录页；按「无法判定」处理才会保留会话（缺口 D18）。
    async fn probe_user_context(&self) -> AppResult<Option<HydroUser>> {
        if let Some(user) = self.cached_user() {
            return Ok(Some(user));
        }
        if self.session().is_none() {
            return Ok(None);
        }

        let value = self
            .get_value("/", true)
            .await
            .map_err(|e| e.context("Hydro 用户上下文"))?;

        let Some(context) = value
            .get("UserContext")
            .or_else(|| value.get("userContext"))
        else {
            return Err(AppError::Unknown(
                "Hydro 响应缺少 UserContext：该部署可能不支持 X-Hydro-Inject 注入头".into(),
            ));
        };
        let brief: UserBriefVO = serde_json::from_value(context.clone())
            .map_err(|e| AppError::Serialization(format!("Hydro UserContext 解析失败: {}", e)))?;

        let id = brief.id.as_ref().map(types::coerce_id).unwrap_or_default();
        if id.is_empty() || id == "0" {
            debug!("Hydro 判定为匿名（UserContext._id == 0）");
            return Ok(None);
        }
        let user = HydroUser {
            id,
            uname: brief.uname.clone().unwrap_or_default(),
        };
        self.set_user(Some(user.clone()));
        Ok(Some(user))
    }

    /// 取当前用户（宽松版，供登录与提交列表筛选使用）。
    ///
    /// 与 `probe_user_context` 的差别：把「无法判定」折成 `Ok(None)` 而不是上抛，
    /// 因为调用点只关心「能不能拿到 uid」，拿不到时按降级路径处理即可
    /// （登录仍成功、提交列表退化为不筛选并记 warn），不该因上下文探测失败
    /// 让主流程失败。**注意：三态会话校验必须用严格版**。
    async fn current_user(&self) -> AppResult<Option<HydroUser>> {
        match self.probe_user_context().await {
            Ok(user) => Ok(user),
            Err(e) => {
                warn!(error = %e, "Hydro 用户上下文探测失败，按未知身份继续");
                Ok(None)
            }
        }
    }

    // ── 比赛题目顺序（展示字母 ↔ pid） ──

    fn cached_pids(&self, tid: &str) -> Option<Vec<String>> {
        self.pid_cache.get(&tid.to_string())
    }

    fn store_pids(&self, tid: &str, pids: &[String]) {
        // 空列表不缓存：多为异常响应，缓存会让后续所有解析都拿到空顺序
        if pids.is_empty() {
            return;
        }
        self.pid_cache.insert(tid.to_string(), pids.to_vec());
    }

    /// 取比赛题目顺序（`tdoc.pids`），带短 TTL 缓存。
    async fn contest_pids(&self, tid: &str) -> AppResult<Vec<String>> {
        if let Some(pids) = self.cached_pids(tid) {
            return Ok(pids);
        }
        let detail: ContestDetailVO = self
            .get_json(&format!("/contest/{}", tid), false)
            .await
            .map_err(|e| e.context("Hydro 比赛详情"))?;
        let pids = detail.tdoc.map(|t| t.pid_list()).unwrap_or_default();
        self.store_pids(tid, &pids);
        Ok(pids)
    }

    /// 把「比赛题目展示字母」解析为 Hydro 的真实 pid。
    ///
    /// 前端契约：`get_problem` 收到的是路由参数 **displayId（字母 A/B/C）**，
    /// 而 Hydro 的 `/p/:pid` 只认 docId 或 pid —— **不认字母**（字母解析只存在于
    /// `record_main` 的题目筛选参数里）。故按 Hydro 自身的规则换算：
    /// `tdoc.pids[parseInt(letter, 36) - 10]`，即 A → 下标 0 … Z → 下标 25。
    ///
    /// 入参不是单字母时原样返回；顺序表拿不到该下标时也原样返回并记 warn
    /// （宁可让服务端回 404 给出真因，也不要在这里编一个 pid）。
    async fn resolve_problem_id(&self, contest_id: &str, problem_id: &str) -> AppResult<String> {
        let trimmed = problem_id.trim();
        let bytes = trimmed.as_bytes();
        if bytes.len() != 1 || !bytes[0].is_ascii_alphabetic() {
            return Ok(trimmed.to_string());
        }
        let index = (bytes[0].to_ascii_uppercase() - b'A') as usize;
        let pids = self.contest_pids(contest_id).await?;
        match pids.get(index).filter(|s| !s.is_empty()) {
            Some(pid) => Ok(pid.clone()),
            None => {
                warn!(
                    contest_id = contest_id,
                    display_id = trimmed,
                    total = pids.len(),
                    "Hydro 题目顺序表缺少该展示字母，按原值作为 pid 请求"
                );
                Ok(trimmed.to_string())
            }
        }
    }

    // ── 映射 helper ──

    /// 赛制是否 ACM（决定榜单单元格语义与 `total_time` 单位）
    fn is_acm_rule(rule: Option<&str>) -> bool {
        rule == Some("acm")
    }

    /// 比赛阶段：`-1` 未开始 / `0` 进行中 / `1` 已结束。
    ///
    /// 抽成纯函数（`now` 由调用方注入）以便单测锁定边界 —— 阶段直接决定
    /// 登录页是「等待开赛」还是「进入赛场」，用挂钟时间测会得到随时间漂移的用例。
    /// 时间缺失（为 0）时按「进行中」处理：宁可让选手进得去，也不要因为
    /// 服务端漏字段而把整场比赛判成已结束。
    fn contest_status(start_time: i64, end_time: i64, now: i64) -> i32 {
        if start_time > 0 && now < start_time {
            return -1;
        }
        if end_time > 0 && now > end_time {
            return 1;
        }
        0
    }

    /// 赛制 → `Contest.contest_type`（0=ACM，1=OI）
    ///
    /// Hydro 的赛制有 acm/oi/ioi/strictioi/ledo/homework 六种，Hinina 只有
    /// ACM/OI 两档：除 `acm` 外一律归 OI（按分数排名）。
    fn contest_type_of(rule: Option<&str>) -> i32 {
        i32::from(!Self::is_acm_rule(rule))
    }

    /// 赛制 → `Contest.oi_rank_score_type`（OI 计分规则，只读展示）
    ///
    /// `oi`/`strictioi` 的每题分取「更高分」提交 → "Highest"；
    /// `ioi`（首次得分即定）与 `ledo`（指数衰减）没有对应口径 → `None`。
    fn oi_rank_score_type_of(rule: Option<&str>) -> Option<String> {
        match rule {
            Some("oi") | Some("strictioi") => Some("Highest".to_string()),
            _ => None,
        }
    }

    /// `tdoc` → `Contest`
    fn into_contest(tdoc: TdocVO) -> Contest {
        let rule = tdoc.rule.clone();
        let start_time = tdoc.begin_at.as_deref().map(types::parse_time).unwrap_or(0);
        // 灵活时长模式（`contestDuration`）下 `endAt` 可能缺省，此时按
        // `beginAt + duration`（`duration` 单位是**小时**，见文档 §4 比赛编辑参数）推算
        let end_time = tdoc
            .end_at
            .as_deref()
            .map(types::parse_time)
            .filter(|v| *v > 0)
            .or_else(|| {
                tdoc.duration
                    .filter(|d| d.is_finite() && *d > 0.0)
                    .map(|d| start_time + (d * 3600.0) as i64)
            })
            .unwrap_or(0);

        Contest {
            id: tdoc.contest_id(),
            title: tdoc.title.clone().unwrap_or_default(),
            start_time,
            end_time,
            description: tdoc.content.clone().unwrap_or_default(),
            contest_type: Self::contest_type_of(rule.as_deref()),
            status: Self::contest_status(start_time, end_time, now_secs()),
            // Hydro 的可见性由 `assign`（组限定）表达，且该字段不在榜单/详情投影里；
            // Hinina 的 `auth` 未参与任何客户端判据，固定 0（公开）
            auth: 0,
            // 榜单显示名规则是 HOJ 专属概念；Hydro 用 displayName 且仅管理员可见，
            // 留空让前端回退 username
            rank_show_name: String::new(),
            seal_rank: tdoc.is_locked(),
            seal_rank_time: tdoc.lock_at.as_deref().map(types::parse_time),
            // 赛后提交开关是 HOJ 专属；Hydro 的赛后提交可见性由赛制
            // （`showRecord`）决定，客户端无法在请求里切换
            allow_end_submit: false,
            oi_rank_score_type: Self::oi_rank_score_type_of(rule.as_deref()),
        }
    }

    /// `pdict` + 题目顺序 → `ContestProblem` 列表
    ///
    /// `contest_id` 由调用方传入（Hydro 的比赛 ID 是 24 位 hex ObjectId，
    /// `ContestProblem.cid` 已是字符串，可如实携带）。
    fn map_contest_problems(
        vo: &ContestProblemListVO,
        contest_id: &str,
        pids: &[String],
    ) -> Vec<ContestProblem> {
        let Some(pdict) = vo.pdict.as_ref() else {
            return Vec::new();
        };
        // HashMap 迭代顺序随机，按 docId 升序固定输出顺序
        let mut entries: Vec<&PdocVO> = pdict.values().collect();
        entries.sort_by_key(|pdoc| pdoc.doc_id_num());

        entries
            .into_iter()
            .enumerate()
            .map(|(order, pdoc)| {
                let doc_id = pdoc
                    .doc_id
                    .as_ref()
                    .map(types::coerce_id)
                    .filter(|s| !s.is_empty())
                    .unwrap_or_default();
                let index = pids
                    .iter()
                    .position(|pid| *pid == doc_id)
                    .unwrap_or(order);
                ContestProblem {
                    // Hydro 的题目主键是数字 docId，与 `ContestProblem.id: i64` 同域
                    id: pdoc.doc_id_num(),
                    display_id: types::display_letter(index),
                    cid: contest_id.to_string(),
                    problem_id: pdoc.problem_id(),
                    display_title: pdoc.title.clone().unwrap_or_default(),
                    ac: pdoc.n_accept.unwrap_or(0),
                    total: pdoc.n_submit.unwrap_or(0),
                    // Hydro 没有气球色概念，前端按列序回退内置调色板
                    color: String::new(),
                }
            })
            .collect()
    }

    /// `pdoc` → `Problem`（详情）
    fn into_problem(pdoc: &PdocVO) -> Problem {
        let config = pdoc.problem_config();
        Problem {
            id: pdoc.problem_id(),
            title: pdoc.title.clone().unwrap_or_default(),
            // Hydro 的题面是**单块 Markdown**（`content`），Hinina 的 Problem 分
            // 描述/输入/输出三段 —— 整块塞进 description，另两段留空；
            // 前端本就按 Markdown 渲染 description，与 Hydro 网页端一致
            description: pdoc.content.clone().unwrap_or_default(),
            input_description: String::new(),
            output_description: String::new(),
            // Hydro 的 JSON 里没有独立样例字段（样例写在题面 Markdown 里）→ 恒空（缺口 D6）
            samples: Vec::new(),
            time_limit: config.as_ref().map(|c| c.time_limit_ms()).unwrap_or(0),
            memory_limit: config.as_ref().map(|c| c.memory_limit_mb()).unwrap_or(0),
            // `config.langs` 是 Hydro 的 key（`cc.cc17`），翻译为 HOJ 显示名后
            // 交给前端（语言权威值的跨端契约，见 types::lang_display）
            languages: config
                .as_ref()
                .and_then(|c| c.langs.as_ref())
                .map(|langs| langs.iter().map(|key| types::lang_display(key)).collect())
                .unwrap_or_default(),
        }
    }

    /// `rdoc` → 评测结果（轮询投影）
    fn into_judgement_result(rdoc: &RdocVO) -> JudgementResult {
        let (time_ms, memory_kb, score) = rdoc.terminal_metrics();
        JudgementResult {
            // 非终态原样透传（WAITING/JUDGING/COMPILING/FETCHED），
            // 与 `get_submission_detail` 的 map_status 输出一致
            status: types::map_status(rdoc.status_code()),
            score,
            time_ms,
            memory_kb,
            // 编译错误是选手判断「为什么挂了」的唯一线索，轮询通道必须带上
            error_message: rdoc.compiler_message(),
        }
    }

    /// `rdoc` → 提交详情（完整投影）
    fn into_submission_detail(
        rdoc: &RdocVO,
        udoc: Option<&UserBriefVO>,
        pdoc: Option<&PdocVO>,
    ) -> SubmissionDetail {
        let (time_ms, memory_kb, score) = rdoc.terminal_metrics();
        let terminal = types::is_terminal_status(rdoc.status_code());
        SubmissionDetail {
            submit_id: rdoc.record_id(),
            pid: rdoc.pid.as_ref().map(types::coerce_id).unwrap_or_default(),
            display_pid: pdoc.map(|p| p.problem_id()).unwrap_or_default(),
            username: udoc.map(UserBriefVO::display).unwrap_or_default(),
            submit_time: rdoc.submit_time(),
            status: types::map_status(rdoc.status_code()),
            time_ms,
            memory_kb,
            // 非终态的 score 没有意义（服务端给 0），按 None 透出
            score: terminal.then_some(score),
            length: rdoc.code_length(),
            language: types::lang_display(rdoc.lang.as_deref().unwrap_or_default()),
            code: rdoc.code.clone().unwrap_or_default(),
            error_message: rdoc.compiler_message(),
            judger: rdoc
                .judger
                .as_ref()
                .map(types::coerce_id)
                .filter(|s| !s.is_empty()),
            // Hydro 无 OI 榜单计分字段
            oi_rank_score: None,
        }
    }

    /// `rdoc` → 提交列表条目
    fn into_submission_record(
        rdoc: &RdocVO,
        pdict: &HashMap<String, PdocVO>,
        udict: &HashMap<String, UserBriefVO>,
        pids: &[String],
        is_acm: bool,
    ) -> SubmissionRecord {
        let pid = rdoc.pid.as_ref().map(types::coerce_id).unwrap_or_default();
        let pdoc = pdict.get(&pid);
        let uid = rdoc.uid.as_ref().map(types::coerce_id).unwrap_or_default();
        let (time_ms, memory_kb, _score) = rdoc.terminal_metrics();
        SubmissionRecord {
            submit_id: rdoc.record_id(),
            display_pid: pdoc
                .map(PdocVO::problem_id)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| pid.clone()),
            title: pdoc.and_then(|p| p.title.clone()).unwrap_or_default(),
            // 比赛内展示字母：记录里的 `pid` 是**数字 docId**，按下标换算
            display_id: pids
                .iter()
                .position(|candidate| *candidate == pid)
                .map(types::display_letter)
                .unwrap_or_default(),
            pid,
            username: udict.get(&uid).map(UserBriefVO::display).unwrap_or_default(),
            submit_time: rdoc.submit_time(),
            status: types::map_status(rdoc.status_code()),
            time_ms,
            memory_kb,
            // 契约：ACM 题为 None（Hydro 对 ACM 也返回 0/100 的 score，不能直接透出）
            score: if is_acm { None } else { rdoc.score },
            length: rdoc.code_length(),
            language: types::lang_display(rdoc.lang.as_deref().unwrap_or_default()),
        }
    }

    /// `testCases[i]` → `JudgeCase`
    fn into_judge_case(case: &types::TestCaseVO, index: usize) -> JudgeCase {
        JudgeCase {
            case_id: case.id.unwrap_or(index as i64 + 1),
            seq: index as i64 + 1,
            status: types::map_status(case.status.unwrap_or(0)),
            time_ms: case.time.unwrap_or(0).max(0) as u64,
            memory_kb: case.memory.unwrap_or(0).max(0) as u64,
            score: case.score,
            group_num: case.subtask_id,
        }
    }

    /// 按 `subtaskId` 把测试点分组为子任务。
    ///
    /// 刻意**不解析 `rdoc.subtasks`**：文档未给出其结构（只有一句
    /// 「`rdoc.subtasks`」），而 `testCases[].subtaskId` 是明确存在的字段，
    /// 由它分组既确定又不依赖猜测。没有子任务的题返回空列表
    /// （前端 `hasSubTasks` 为假 → 走平铺展示）。
    fn group_sub_tasks(cases: &[JudgeCase]) -> Vec<SubTaskCases> {
        let mut groups: HashMap<i64, Vec<JudgeCase>> = HashMap::new();
        for case in cases {
            if let Some(group) = case.group_num {
                groups.entry(group).or_default().push(case.clone());
            }
        }
        let mut result: Vec<SubTaskCases> = groups
            .into_iter()
            .map(|(group_num, cases)| SubTaskCases { group_num, cases })
            .collect();
        result.sort_by_key(|group| group.group_num);
        result
    }

    /// 取比赛题目顺序：优先响应自带的 `tdoc.pids`，缺失时回退带缓存的查询
    async fn pids_of(&self, tid: &str, tdoc: Option<&TdocVO>) -> AppResult<Vec<String>> {
        if let Some(pids) = tdoc.map(TdocVO::pid_list).filter(|p| !p.is_empty()) {
            self.store_pids(tid, &pids);
            return Ok(pids);
        }
        self.contest_pids(tid).await
    }

    /// 拉取单条评测记录
    async fn fetch_record(&self, rid: &str) -> AppResult<RecordDetailVO> {
        self.get_json(&format!("/record/{}", rid), false)
            .await
            .map_err(|e| e.context("Hydro 评测记录"))
    }
}

// ── AuthProvider ──

#[async_trait]
impl AuthProvider for HydroAdapter {
    /// 登录：`POST /login`。
    ///
    /// Hydro 的登录响应**体里没有 token**（只有 `{"url":"/"}`），会话靠
    /// `Set-Cookie: sid=<32 位随机串>` 下发；且未登录/登录成功都会触发重定向，
    /// 而带 `Accept: application/json` 时重定向被序列化成 HTTP 200 + `{"url":...}`
    /// （不会 302），Set-Cookie 因此留在同一次响应里 —— 这是本方法必须
    /// 直连 reqwest 而不用 HttpClient POST 变体的原因（后者不设 Accept，
    /// 会跟随 302 丢掉初始响应的 Set-Cookie）。
    ///
    /// 密码**原样发送**（Hydro 服务端自行做哈希比对，客户端不得预处理）。
    async fn login(&self, username: &str, password: &str) -> AppResult<User> {
        let path = "/login";
        let body = json!({
            "uname": username,
            "password": password,
            // 记住我：会话有效期从默认 3 小时延长到 30 天，赛场场景必须开
            "rememberme": true,
        });
        info!(username = username, "Hydro 登录请求");

        let response = self
            .post_raw(path, Some(&body))
            .await
            .map_err(|e| e.context("Hydro 登录"))?;

        let sid = types::extract_sid(&response.headers).ok_or_else(|| {
            AppError::Auth("Hydro 登录响应未携带会话 Cookie（sid）".into())
        })?;
        // 先取会话再消费响应体：错误包络（LoginError 等）在这里被翻译成 AppError
        let _: Value = response.into_value().map_err(|e| {
            warn!(error = %e, "Hydro 登录失败");
            e.context("Hydro 登录")
        })?;

        self.set_session(Some(sid.clone()));
        // 取 uid/uname：失败时降级为「用输入的用户名」，不让一次上下文探测
        // 失败否定已经建立好的会话
        let user = match self.probe_user_context().await {
            Ok(Some(user)) => user,
            Ok(None) => {
                warn!("Hydro 登录后用户上下文为匿名，身份信息降级");
                HydroUser {
                    id: String::new(),
                    uname: username.to_string(),
                }
            }
            Err(e) => {
                warn!(error = %e, "Hydro 登录后用户上下文探测失败，身份信息降级");
                HydroUser {
                    id: String::new(),
                    uname: username.to_string(),
                }
            }
        };
        self.set_user(Some(user.clone()));

        info!(username = user.uname, uid = user.id, "Hydro 登录成功");
        Ok(User {
            id: user.id,
            username: user.uname,
            // 磁盘会话保存裸 sid；`send` 会补上 "Bearer " 前缀
            token: sid,
        })
    }

    /// 登出：`POST /logout`（`GET` 只渲染确认页，不做登出）。
    ///
    /// 远端结果一律忽略（可能失败），以清除本地身份为主 —— 与 HOJ 侧同一约定。
    async fn logout(&self) -> AppResult<()> {
        if self.session().is_none() {
            debug!("Hydro 无会话，跳过远端登出");
            self.clear_identity();
            return Ok(());
        }
        if let Err(e) = self
            .post_json::<Value>("/logout", None)
            .await
        {
            warn!(error = %e, "Hydro 远端登出失败，仅清除本地身份");
        }
        self.clear_identity();
        info!("Hydro 已登出");
        Ok(())
    }

    /// 校验会话有效性（三态契约）。
    ///
    /// - `Ok(true)`  服务端确认有效（注入头回传的 `_id != 0`）
    /// - `Ok(false)` 服务端**明确**判定失效：`_id == 0`（sid 无效会被静默降级为
    ///   匿名）或错误包络被判为 `Auth`（`PrivilegeError`）或 JSON 化登录重定向
    /// - `Err(_)`    无法判定（网络异常/5xx/解析失败/部署不支持注入头）→
    ///   `AuthService` 映射为 `SessionValidity::Unknown` 并**保留**本地会话
    ///
    /// 本地无 sid 时直接 `Ok(false)`：这是本地即可确定的结论，不是网络问题。
    async fn validate_session(&self) -> AppResult<bool> {
        if self.session().is_none() {
            debug!("Hydro 本地无会话，判定为失效");
            return Ok(false);
        }
        match self.probe_user_context().await {
            Ok(Some(_)) => Ok(true),
            Ok(None) => {
                info!("Hydro 服务端判定会话已失效（匿名）");
                self.clear_identity();
                Ok(false)
            }
            Err(e) if matches!(e, AppError::Auth(_)) => {
                info!(reason = %e, "Hydro 服务端明确判定会话失效");
                self.clear_identity();
                Ok(false)
            }
            // 其余一律上抛：网络抖动/5xx/解析失败都属「无法判定」，
            // 折成 Ok(false) 会在赛前一次断网就把选手踢回登录页
            Err(e) => Err(e.context("Hydro 会话校验")),
        }
    }

    /// 回注磁盘会话里的 sid（应用重启后由 `AuthService::get_session` 调用）
    fn restore_token(&self, token: &str) {
        if !token.is_empty() {
            self.set_session(Some(token.to_string()));
        }
    }
}

// ── ContestProvider ──

#[async_trait]
impl ContestProvider for HydroAdapter {
    /// 比赛列表：`GET /contest?page=N` 翻页至 `tpcount` 为止。
    ///
    /// Hydro 的页大小由服务端 `pagination.contest` 决定（客户端无法指定），
    /// 故必须按总页数翻页；上限 `MAX_CONTEST_PAGES` 页做防御。
    async fn list_contests(&self) -> AppResult<Vec<Contest>> {
        let mut contests = Vec::new();
        let mut page = 1i64;
        loop {
            let vo: ContestListVO = self
                .get_json(&format!("/contest?page={}", page), false)
                .await
                .map_err(|e| e.context("Hydro 比赛列表"))?;

            let items = vo.tdocs.unwrap_or_default();
            let received = items.len();
            contests.extend(items.into_iter().map(Self::into_contest));

            let total_pages = vo.tpcount.unwrap_or(1).max(1);
            if received == 0 || page >= total_pages || page >= MAX_CONTEST_PAGES {
                if page >= MAX_CONTEST_PAGES && page < total_pages {
                    warn!(page = page, "Hydro 比赛列表已达翻页上限，结果可能不完整");
                }
                break;
            }
            page += 1;
        }
        debug!(count = contests.len(), "Hydro 比赛列表已获取");
        Ok(contests)
    }

    async fn get_contest(&self, contest_id: &str) -> AppResult<Contest> {
        let vo: ContestDetailVO = self
            .get_json(&format!("/contest/{}", contest_id), false)
            .await
            .map_err(|e| e.context("Hydro 比赛详情"))?;
        let tdoc = vo
            .tdoc
            .ok_or_else(|| AppError::Contest("Hydro 比赛详情缺少 tdoc".into()))?;
        // 顺带刷新题目顺序缓存，省掉后续 get_problem 的一次往返
        self.store_pids(contest_id, &tdoc.pid_list());
        Ok(Self::into_contest(tdoc))
    }

    /// 比赛题目列表：`GET /contest/:tid/problems`。
    ///
    /// 展示字母由 `tdoc.pids` 的**下标**派生（Hydro 自身的规则），
    /// 因为 `pdict` 是按 docId 键的映射，且 JS 对象的数字键会被重排 ——
    /// 不能靠键顺序推字母。
    async fn list_contest_problems(&self, contest_id: &str) -> AppResult<Vec<ContestProblem>> {
        let vo: ContestProblemListVO = self
            .get_json(&format!("/contest/{}/problems", contest_id), false)
            .await
            .map_err(|e| e.context("Hydro 比赛题目列表"))?;
        let pids = self.pids_of(contest_id, vo.tdoc.as_ref()).await?;
        let problems = Self::map_contest_problems(&vo, contest_id, &pids);
        debug!(
            contest_id = contest_id,
            count = problems.len(),
            "Hydro 比赛题目列表已获取"
        );
        Ok(problems)
    }

    /// 榜单：`GET /contest/:tid/scoreboard`。
    ///
    /// **`query` 的字段基本被忽略**：Hydro 榜单是「整榜算完一次性返回」，
    /// 服务端不接受分页/关键词/打星/赛后提交参数（那些是网页端在前端做的），
    /// 故页码/搜索/打星/赛后提交全部无效，返回单页全量（见缺口 D10）。
    /// 调用方的轮询节奏仍然生效。
    async fn get_contest_rank(
        &self,
        contest_id: &str,
        query: &RankQuery,
    ) -> AppResult<ContestRankPage> {
        let vo: ScoreboardVO = self
            .get_json(&format!("/contest/{}/scoreboard", contest_id), false)
            .await
            .map_err(|e| e.context("Hydro 榜单"))?;
        if query.keyword.as_deref().is_some_and(|k| !k.trim().is_empty()) {
            debug!("Hydro 榜单不支持服务端关键词搜索，本次搜索条件被忽略");
        }
        let page = types::scoreboard_rank_page(&vo);
        debug!(
            contest_id = contest_id,
            rows = page.records.len(),
            "Hydro 榜单已获取"
        );
        Ok(page)
    }

    /// 比赛公告：**Hydro 没有公告接口**，恒返回空页。
    ///
    /// 现状（已与项目负责人确认并留档，见缺口 D11）：Hydro 的对应能力是
    /// **答疑（clarification）**，与 Hinina 的公告在语义与数据结构上都不同
    /// （答疑是「提问 + 裁判回复」的会话，公告是单向广播）。HOJ 是本项目
    /// 第一优先级的 OJ，且短期不会为 Hydro 改前端，故此处不把答疑伪装成公告
    /// —— 那会让选手把裁判的定向回复误读成全场公告。
    ///
    /// 后续若要做：在 Provider 层新增「答疑」能力（新的 trait 方法或实体），
    /// 由前端按 OJ 能力自动切换面板形态，而不是在本方法里做有损映射。
    async fn list_announcements(
        &self,
        contest_id: &str,
        _current_page: i64,
        _limit: i64,
    ) -> AppResult<AnnouncementPage> {
        debug!(contest_id = contest_id, "Hydro 无公告接口，返回空页");
        Ok(AnnouncementPage {
            records: Vec::new(),
            total: 0,
            size: 0,
            current: 1,
            pages: 0,
        })
    }
}

// ── ProblemProvider ──

#[async_trait]
impl ProblemProvider for HydroAdapter {
    /// 比赛题目列表（摘要形态，与 `ContestProvider::list_contest_problems` 同源）
    async fn list_problems(&self, contest_id: &str) -> AppResult<Vec<Problem>> {
        let vo: ContestProblemListVO = self
            .get_json(&format!("/contest/{}/problems", contest_id), false)
            .await
            .map_err(|e| e.context("Hydro 题目列表"))?;
        let problems: Vec<Problem> = vo
            .pdict
            .unwrap_or_default()
            .values()
            .map(|pdoc| Problem {
                id: pdoc.problem_id(),
                title: pdoc.title.clone().unwrap_or_default(),
                description: String::new(),
                input_description: String::new(),
                output_description: String::new(),
                samples: Vec::new(),
                time_limit: 0,
                memory_limit: 0,
                // 列表投影不含 config → 语言白名单只能从详情接口取
                languages: Vec::new(),
            })
            .collect();
        debug!(contest_id = contest_id, count = problems.len(), "Hydro 题目列表已获取");
        Ok(problems)
    }

    /// 题目详情：字母 displayId → `tdoc.pids` → `GET /p/:pid?tid=`。
    ///
    /// `problem_id` 可能是展示字母（解题页路由参数）或真实 pid（limits 批量拉取），
    /// 由 `resolve_problem_id` 判定。
    async fn get_problem(&self, contest_id: &str, problem_id: &str) -> AppResult<Problem> {
        let pid = self.resolve_problem_id(contest_id, problem_id).await?;
        let path = if contest_id.trim().is_empty() {
            format!("/p/{}", pid)
        } else {
            // `tid` 让服务端进入比赛模式（校验已报名、清空 tag 与统计）
            format!("/p/{}?tid={}", pid, contest_id)
        };
        let vo: ProblemDetailVO = self
            .get_json(&path, false)
            .await
            .map_err(|e| e.context("Hydro 题目详情"))?;
        let pdoc = vo
            .pdoc
            .ok_or_else(|| AppError::Problem("Hydro 题目详情缺少 pdoc".into()))?;

        let problem = Self::into_problem(&pdoc);
        debug!(
            contest_id = contest_id,
            problem_id = problem_id,
            pid = problem.id,
            title = problem.title,
            "Hydro 题目详情已获取"
        );
        Ok(problem)
    }

    /// 批量获取「我的题目状态」：`GET /contest/:tid/problems` 的 `psdict`。
    ///
    /// 入参是**真实 pid**（前端 `ContestProblem.problem_id`），而 `psdict` 以
    /// **docId** 为键，故先经 `pdict` 反查。未提交的题不出现在返回 map 中
    /// （契约：未出现即未提交）。
    async fn get_user_problem_status(
        &self,
        contest_id: &str,
        problem_ids: &[String],
    ) -> AppResult<HashMap<String, i32>> {
        if problem_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let vo: ContestProblemListVO = self
            .get_json(&format!("/contest/{}/problems", contest_id), false)
            .await
            .map_err(|e| e.context("Hydro 题目状态"))?;
        let pdict = vo.pdict.clone().unwrap_or_default();
        let psdict = vo.psdict.clone().unwrap_or_default();

        let mut statuses = HashMap::new();
        for pid in problem_ids {
            let doc_id = pdict
                .iter()
                .find(|(_, pdoc)| pdoc.problem_id() == *pid)
                .map(|(key, pdoc)| {
                    pdoc.doc_id
                        .as_ref()
                        .map(types::coerce_id)
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| key.clone())
                });
            // psdict 的键可能是 docId（文档如此）或 pid（部分版本按 pid 键）
            let entry = doc_id
                .as_ref()
                .and_then(|key| psdict.get(key))
                .or_else(|| psdict.get(pid));
            let code = entry.map(types::problem_status_code).unwrap_or(0);
            if code != 0 {
                statuses.insert(pid.clone(), code);
            }
        }
        debug!(contest_id = contest_id, count = statuses.len(), "Hydro 题目状态已获取");
        Ok(statuses)
    }
}

// ── SubmissionProvider ──

#[async_trait]
impl SubmissionProvider for HydroAdapter {
    /// 提交代码：`POST /p/:pid/submit`（`lang` + `code` + `tid`）。
    ///
    /// 入参 `problem_id` 是**真实 pid**（前端 `Problem.id` / `ContestProblem.problem_id`），
    /// 故这里**刻意不做字母换算** —— 万一某题的 pid 恰好是一个字母（如 "A"），
    /// 换算会把提交打到另一道题上，这个失败模式不可接受。
    /// `display_id` 仅 HOJ 需要（其 `pid` 收比赛内题号），Hydro 忽略之。
    async fn submit(
        &self,
        contest_id: &str,
        problem_id: &str,
        _display_id: &str,
        language: &str,
        source_code: &str,
    ) -> AppResult<String> {
        self.require_session()?;
        // 语言权威值是 HOJ 显示名，提交契约要 Hydro 的 key；未命中映射表时
        // 原样透传（部署可能自定义了 langs，原值至少是服务端认得的写法）
        let lang = types::lang_key(language).unwrap_or_else(|| language.trim().to_string());
        let mut body = json!({
            "lang": lang,
            "code": source_code,
        });
        if !contest_id.trim().is_empty() {
            body["tid"] = json!(contest_id);
        }

        info!(
            contest_id = contest_id,
            problem_id = problem_id,
            language = language,
            "Hydro 提交代码"
        );
        let vo: SubmitVO = self
            .post_json(&format!("/p/{}/submit", problem_id.trim()), Some(&body))
            .await
            .map_err(|e| e.context("Hydro 提交"))?;

        let rid = vo
            .rid
            .as_ref()
            .map(types::coerce_id)
            .filter(|s| !s.is_empty());
        match rid {
            Some(rid) => {
                debug!(submission_id = rid, "Hydro 代码已提交");
                Ok(rid)
            }
            // 比赛隐藏本人记录时服务端用 tid 代替 rid（`canShowSelfRecord` 为假）：
            // 此时拿不到记录 ID，客户端无法轮询评测结果（缺口 D12）
            None => Err(AppError::Submission(
                "Hydro 未返回评测记录 ID：本场比赛隐藏本人评测记录，无法查询评测结果".into(),
            )),
        }
    }

    /// 评测结果（轮询投影）：`GET /record/:rid`
    async fn get_judgement(&self, submission_id: &str) -> AppResult<JudgementResult> {
        let vo = self.fetch_record(submission_id).await?;
        let rdoc = vo
            .rdoc
            .ok_or_else(|| AppError::Submission("Hydro 评测记录缺少 rdoc".into()))?;
        let result = Self::into_judgement_result(&rdoc);
        debug!(
            submission_id = submission_id,
            status = ?result.status,
            time_ms = result.time_ms,
            "Hydro 评测结果"
        );
        Ok(result)
    }

    /// 提交列表：`GET /record?page=&tid=&uidOrName=&pid=&status=`。
    ///
    /// 与 HOJ 的差异（均在缺口报告留档）：
    /// - Hydro **没有 `limit`/`uid` 参数**，页大小由服务端决定，用户筛选用 `uidOrName`
    ///   （接受 uid/用户名/邮箱，此处用探测到的 uid 以保证精确）；
    /// - Hydro **不返回总条数**，故 `total`/`pages` 只能按假定页大小推导（D8）；
    /// - `status` 收 Hydro 码，前端传的是 HOJ 码 → 经 `types::hoj_status_to_hydro` 翻译，
    ///   无对应语义时明确报错而不是静默忽略筛选（D13）。
    async fn list_contest_submissions(
        &self,
        query: &SubmissionQuery,
    ) -> AppResult<SubmissionPage> {
        let page = query.current_page.max(1);
        let mut path = format!("/record?page={}", page);
        if !query.contest_id.trim().is_empty() {
            path.push_str(&format!("&tid={}", query.contest_id));
        }
        if query.only_mine {
            // 产品决策：后端恒置 onlyMine，前端不可绕过
            let uid = self
                .current_user()
                .await?
                .map(|user| user.id)
                .filter(|id| !id.is_empty())
                .ok_or_else(|| AppError::Auth("Hydro 需登录后才能只看本人提交".into()))?;
            path.push_str(&format!("&uidOrName={}", uid));
        }
        if let Some(display_id) = query
            .problem_display_id
            .as_deref()
            .filter(|s| !s.is_empty())
        {
            // 单字母 pid 由服务端按 tdoc.pids 解析（与 `record_main` 的规则一致）
            path.push_str(&format!("&pid={}", display_id));
        }
        if let Some(status) = query.status {
            let hydro_status = types::hoj_status_to_hydro(status).ok_or_else(|| {
                AppError::Submission(format!(
                    "Hydro 无对应评测状态，无法按该状态筛选（HOJ status={}）",
                    status
                ))
            })?;
            path.push_str(&format!("&status={}", hydro_status));
        }

        let vo: RecordListVO = self
            .get_json(&path, false)
            .await
            .map_err(|e| e.context("Hydro 提交列表"))?;

        let pdict = vo.pdict.clone().unwrap_or_default();
        let udict = vo.udict.clone().unwrap_or_default();
        let is_acm = Self::is_acm_rule(vo.tdoc.as_ref().and_then(|t| t.rule.as_deref()));
        let pids = self
            .pids_of(&query.contest_id, vo.tdoc.as_ref())
            .await
            .unwrap_or_default();

        let records: Vec<SubmissionRecord> = vo
            .rdocs
            .unwrap_or_default()
            .iter()
            .map(|rdoc| Self::into_submission_record(rdoc, &pdict, &udict, &pids, is_acm))
            .collect();

        // Hydro 不返回总数：以「本页条数」为页大小，按假定页大小推导是否还有下一页
        let size = records.len() as i64;
        let has_more = size >= ASSUMED_RECORD_PAGE_SIZE;
        debug!(
            contest_id = query.contest_id,
            page = page,
            count = size,
            "Hydro 提交列表已获取"
        );
        Ok(SubmissionPage {
            records,
            total: (page - 1) * ASSUMED_RECORD_PAGE_SIZE + size,
            size,
            current: page,
            pages: if has_more { page + 1 } else { page },
        })
    }

    /// 提交详情：`GET /record/:rid`（完整投影，含 code / compilerTexts / judger）
    async fn get_submission_detail(&self, submit_id: &str) -> AppResult<SubmissionDetail> {
        let vo = self.fetch_record(submit_id).await?;
        let rdoc = vo
            .rdoc
            .ok_or_else(|| AppError::Submission("Hydro 提交详情缺少 rdoc".into()))?;
        debug!(submit_id = submit_id, "Hydro 提交详情已获取");
        Ok(Self::into_submission_detail(
            &rdoc,
            vo.udoc.as_ref(),
            vo.pdoc.as_ref(),
        ))
    }

    /// 测试点结果：与提交详情**同一端点**（`rdoc.testCases` / `rdoc.subtasks`），
    /// Hydro 没有独立的测试点接口。
    async fn get_submission_cases(&self, submit_id: &str) -> AppResult<SubmissionCases> {
        let vo = self.fetch_record(submit_id).await?;
        let rdoc = vo
            .rdoc
            .ok_or_else(|| AppError::Submission("Hydro 测试点结果缺少 rdoc".into()))?;

        let cases: Vec<JudgeCase> = rdoc
            .test_cases
            .as_deref()
            .unwrap_or_default()
            .iter()
            .enumerate()
            .map(|(index, case)| Self::into_judge_case(case, index))
            .collect();
        let sub_tasks = Self::group_sub_tasks(&cases);
        // 判题模式取题目 `config.type`（default/subtask/objective/submit_answer/
        // remote_judge）：前端只把它当徽章展示，透传比编一个更诚实
        let mode = vo
            .pdoc
            .as_ref()
            .and_then(PdocVO::problem_config)
            .and_then(|config| config.kind)
            .unwrap_or_else(|| "default".to_string());

        debug!(submit_id = submit_id, cases = cases.len(), "Hydro 测试点结果已获取");
        Ok(SubmissionCases {
            cases,
            sub_tasks,
            mode,
        })
    }
}


// ── 适配器工厂 ──

/// Hydro 工厂（`adapter::factories()` 清单成员）。
///
/// 注册侧聚合：四个 trait 实现包进一个 [`ProviderSet`]，组合根对每个 OJ
/// 只见一行 `factory.build(&deps, base_url)`。Hydro 不用 `EventBus`
/// （无 HOJ 的凭证轮换协议，sid 值不变、由服务端滑动续期），故 `AdapterDeps`
/// 里只消费 `http_client`。
pub struct HydroFactory;

impl AdapterFactory for HydroFactory {
    fn id(&self) -> &'static str {
        HydroAdapter::ID
    }

    fn build(&self, deps: &AdapterDeps, base_url: &str) -> ProviderSet {
        let adapter = Arc::new(HydroAdapter::new(
            Arc::clone(&deps.http_client),
            base_url.to_string(),
        ));
        ProviderSet::full(
            Arc::clone(&adapter) as Arc<dyn AuthProvider>,
            Arc::clone(&adapter) as Arc<dyn ContestProvider>,
            Arc::clone(&adapter) as Arc<dyn ProblemProvider>,
            Arc::clone(&adapter) as Arc<dyn SubmissionProvider>,
        )
    }
}

/// Hydro 的工厂单例（零状态，静态常量即可）。
pub static FACTORY: HydroFactory = HydroFactory;

#[cfg(test)]
#[path = "tests/mod_tests.rs"]
mod tests;
