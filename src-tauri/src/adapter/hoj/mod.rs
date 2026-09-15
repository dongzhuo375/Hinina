// HOJ Adapter：实现 Auth + Contest + Problem + Submission 四个 trait。
//
// HOJ (Hydro Online Judge) API 基于 JWT 认证，统一响应格式 {status, msg, data}。
// Token 在登录响应的 `authorization` 头中返回，后续请求通过该头传递。
pub mod types;
pub mod error;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use tracing::{debug, info, warn};

use crate::core::entity::contest::{Contest, ContestProblem};
use crate::core::entity::problem::{Problem, Sample};
use crate::core::entity::rank::{ContestRankPage, RankQuery};
use crate::core::entity::submission::{JudgementResult, JudgementStatus};
use crate::core::entity::user::User;
use crate::core::error::{AppError, AppResult};
use crate::core::event::app_event::{AppEvent, AuthEvent};
use crate::core::event::event_bus::EventBus;
use crate::core::provider::auth::AuthProvider;
use crate::core::provider::contest::ContestProvider;
use crate::core::provider::problem::ProblemProvider;
use crate::core::provider::submission::SubmissionProvider;
use crate::infra::http::HttpClient;

use self::types::{
    map_status, ApiResponse, ContestProblemVO, ContestRankDTO, ContestRankVO, ContestVO, JudgeVO,
    LoginRequest, ProblemInfoVO, SubmitRequest, UserInfoVO, UserProblemStatusDTO,
};

/// HOJ OJ 适配器。
///
/// 实现 `AuthProvider`、`ContestProvider`、`ProblemProvider`、`SubmissionProvider`
/// 四个 trait，通过 HttpClient 与 HOJ 后端通信。
pub struct HOJAdapter {
    http: Arc<HttpClient>,
    base_url: String,
    /// 当前 JWT token（登录后设置）
    token: RwLock<Option<String>>,
    /// 事件总线：token 轮换时发布 `AuthEvent::TokenRefreshed`，供 AuthService 回写磁盘会话
    event_bus: Arc<EventBus>,
}

impl HOJAdapter {
    /// 创建 HOJAdapter。
    ///
    /// `base_url` 不含尾部 `/api`，如 `https://hoj.dongzhuo.top`。
    pub fn new(http: Arc<HttpClient>, base_url: String, event_bus: Arc<EventBus>) -> Self {
        // 去掉尾部斜杠以统一拼接
        let base_url = base_url.trim_end_matches('/').to_string();
        Self {
            http,
            base_url,
            token: RwLock::new(None),
            event_bus,
        }
    }

    /// 构造完整 API URL。
    fn api_url(&self, path: &str) -> String {
        format!("{}/api{}", self.base_url, path)
    }

    /// 解析比赛 ID（HOJ 的 cid 是数字，Hinina 内部统一用字符串传递）。
    ///
    /// 非法 ID 必须报错而不是回退 0：HOJ 以 `cid = 0` 表示「非比赛场景」，
    /// 静默回退会让比赛中的提交落到练习题库 —— 不计入榜单，选手在赛场上无从察觉。
    fn parse_cid(contest_id: &str) -> AppResult<i64> {
        contest_id
            .parse::<i64>()
            .map_err(|_| AppError::Contest(format!("比赛 ID 非法: {}", contest_id)))
    }

    /// 获取当前存储的 token。
    fn get_token(&self) -> Option<String> {
        self.token.read().ok()?.clone()
    }

    /// 存储 token。
    fn set_token(&self, token: String) {
        if let Ok(mut t) = self.token.write() {
            *t = Some(token);
        }
    }

    /// 清除 token。
    fn clear_token(&self) {
        if let Ok(mut t) = self.token.write() {
            *t = None;
        }
    }

    /// 发送 GET 请求并解析为 HOJ 响应，自动处理服务端 token 轮换。
    ///
    /// HOJ 服务端会在 token 到期前返回 `Refresh-Token: true` 和新 `Authorization` 头。
    /// 此方法在收到轮换的新 token 时自动更新内部存储，避免后续请求 401。
    ///
    /// token 缺失时**不报错**，按匿名请求发出：`get-contest-list` 等 `@AnonApi`
    /// 接口在登录页（尚无会话）就要能用；HOJ 对匿名接口带无效 token 也照常返回 200。
    async fn get_json_authed<T: serde::de::DeserializeOwned>(&self, url: &str) -> AppResult<T> {
        let token = self.get_token();
        let (body, headers) = self
            .http
            .get_text_with_headers(url, token.as_deref())
            .await?;
        self.handle_token_rotation(&headers);
        Self::parse_hoj_json::<T>(&body, url)
    }

    /// 发送 POST 请求（JSON body）并解析为 HOJ 响应，同样处理 token 轮换。
    ///
    /// 榜单轮询、题目状态等高频 POST 场景必须走此方法：
    /// 若漏掉轮换处理，token 到期后会出现周期性 401。
    async fn post_json_authed<T: serde::de::DeserializeOwned, B: serde::Serialize>(
        &self,
        url: &str,
        body: &B,
    ) -> AppResult<T> {
        let token = self.get_token();
        let (text, headers) = self
            .http
            .post_text_with_headers(url, body, token.as_deref())
            .await?;
        self.handle_token_rotation(&headers);
        Self::parse_hoj_json::<T>(&text, url)
    }

    /// 把 `get-user-auth-info` 的调用结果映射为三态契约的 `Ok(true)` / `Ok(false)` / `Err`。
    ///
    /// 抽成纯函数是为了让「哪些情况算服务端**明确**判定失效」可被单元测试锁定 ——
    /// 这条判据直接决定选手会不会在赛前被一次网络抖动踢回登录页。
    fn session_validity_from_response(
        result: AppResult<ApiResponse<serde_json::Value>>,
    ) -> AppResult<bool> {
        match result {
            Ok(resp) if resp.is_success() => Ok(true),
            // 非 200 且不属于鉴权失败（如 400 参数错误、500 服务端异常）：
            // 无法据此断定会话状态，按「无法判定」上抛，避免误清会话
            Ok(resp) => Err(AppError::Unknown(format!(
                "HOJ 会话校验返回非成功状态 status={}",
                resp.status
            ))),
            // 服务端明确判定失效，两条路径都要认：
            // ① HTTP 401 —— infra 的 status_error 映射为 Auth；
            // ② HTTP 200 + 体内 status=401/403+登录提示 —— parse_hoj_json 的
            //    auth_failure_from_body 映射为 Auth（HOJ 部分端点用这种方式报鉴权失败）
            Err(AppError::Auth(msg)) => {
                info!(reason = %msg, "HOJ 判定会话已失效");
                Ok(false)
            }
            // 其余一律上抛：网络异常、超时、5xx、响应解析失败都属于「无法判定」，
            // 由 AuthService 映射为 SessionValidity::Unknown 并保留本地会话
            Err(e) => Err(e.context("HOJ 会话校验")),
        }
    }

    /// 断言已持有 token（提交等必须登录的操作的前置校验）。
    ///
    /// `get/post_json_authed` 允许匿名（登录页要在无会话时拉比赛列表），
    /// 因此需要登录的接口自行前置断言，给出「请先登录」而不是等服务端返回 401。
    fn require_token(&self) -> AppResult<()> {
        if self.get_token().is_none() {
            return Err(AppError::Auth("请先登录".into()));
        }
        Ok(())
    }

    /// 解析响应头中的 HOJ 私有轮换协议：更新内存 token 并发布事件供 AuthService 回写磁盘会话。
    fn handle_token_rotation(&self, headers: &reqwest::header::HeaderMap) {
        if let Some(new_token) = extract_refreshed_token(headers) {
            debug!("HOJ token 已轮换，更新本地缓存并发布事件");
            self.set_token(new_token.clone());
            // 通知 AuthService 将新 token 回写磁盘会话，避免重启后回注过期凭证
            self.event_bus.publish(&AppEvent::Auth(AuthEvent::TokenRefreshed {
                token: new_token,
            }));
        }
    }

    /// 解析 HOJ 响应体：剔除 `null` 成员 → 识别响应体内的认证失败 → 类型化解析。
    ///
    /// 去 null 的必要性见 `types::strip_nulls` 注释（HOJ 对未设置字段返回 `null`，
    /// 而 `#[serde(default)]` 只管字段缺失，显式 null 会让整个响应解析失败）。
    ///
    /// 两类解析失败都归为 `AppError::Serialization` 并带上 URL 与响应体前缀：
    /// 「不是合法 JSON」通常是网关返回了 HTML 错误页，「字段不匹配」才是 DTO 问题，
    /// 分开描述才能一眼定位，而不是笼统报一个「网络错误」。
    fn parse_hoj_json<T: serde::de::DeserializeOwned>(body: &str, url: &str) -> AppResult<T> {
        let mut value: serde_json::Value = serde_json::from_str(body).map_err(|e| {
            AppError::Serialization(format!(
                "HOJ 响应不是合法 JSON {}: {}（响应前 200 字符: {}）",
                url,
                e,
                preview(body)
            ))
        })?;
        types::strip_nulls(&mut value);

        if let Some(err) = Self::auth_failure_from_body(&value) {
            return Err(err);
        }

        serde_json::from_value::<T>(value).map_err(|e| {
            AppError::Serialization(format!(
                "HOJ 响应字段不匹配 {}: {}（响应前 200 字符: {}）",
                url,
                e,
                preview(body)
            ))
        })
    }

    /// 识别 HOJ 放在**响应体**里的认证失败，返回 `AppError::Auth`。
    ///
    /// HOJ 的鉴权失败不走 HTTP 状态码：实测匿名访问 `get-contest-problem` 返回
    /// HTTP 200 + `{"status":403,"msg":"请您先登录！"}`。若不在这里识别，
    /// 各调用点会把它包成 Contest / Problem / Submission 变体，而前端 sessionGuard
    /// 是依据 `variant === 'Auth'` 判定会话失效的 —— token 过期时选手只会看到一堆
    /// 「比赛数据错误」，永远不会被带回登录页。
    ///
    /// 判定刻意保守，避免把「无权访问某场私有赛」误判成会话失效而踢人：
    /// - `status == 401`：语义明确是未认证，一律视为会话问题；
    /// - `status == 403`：仅当消息指向登录/凭证时才视为会话问题，
    ///   否则保留为业务错误（如私有赛未注册、需要密码）。
    fn auth_failure_from_body(value: &serde_json::Value) -> Option<AppError> {
        let status = value.get("status")?.as_i64()?;
        if status != 401 && status != 403 {
            return None;
        }
        let msg = value
            .get("msg")
            .and_then(|v| v.as_str())
            .unwrap_or("登录状态已失效");
        let looks_like_auth = status == 401
            || ["登录", "登陆", "token", "Token", "认证", "未授权"]
                .iter()
                .any(|kw| msg.contains(kw));
        if !looks_like_auth {
            return None;
        }
        warn!(status = status, msg = msg, "HOJ 响应体报告认证失败");
        Some(AppError::Auth(format!("{}（HOJ status={}）", msg, status)))
    }

    // ── 工具方法 ──

    /// ContestVO → 领域实体（比赛列表与比赛详情共用，避免两处映射漂移）。
    fn into_contest(c: ContestVO) -> Contest {
        Contest {
            id: c.id.to_string(),
            title: c.title,
            start_time: Self::parse_time(&c.start_time),
            end_time: Self::parse_time(&c.end_time),
            description: c.description.unwrap_or_default(),
            contest_type: c.r#type,
            status: c.status,
            auth: c.auth,
            rank_show_name: c.rank_show_name.unwrap_or_default(),
            seal_rank: c.seal_rank,
            // 封榜时间为空串或无法解析时视为未设置
            seal_rank_time: c
                .seal_rank_time
                .filter(|s| !s.is_empty())
                .map(|s| Self::parse_time(&s))
                .filter(|t| *t > 0),
            allow_end_submit: c.allow_end_submit,
        }
    }

    /// 解析 ISO 时间字符串为秒级 UTC 时间戳。
    /// 格式："2024-01-01T08:00:00" 或 "2024-01-01 08:00:00"
    fn parse_time(s: &str) -> i64 {
        if s.len() < 19 {
            return 0;
        }
        let bytes = s.as_bytes();
        let d = |i: usize| -> i64 {
            ((bytes[i] as i64) - 48) * 10 + (bytes[i + 1] as i64) - 48
        };
        // 使用 chrono 时间计算算法：以 2000-01-01 为纪元基准日
        let year = d(0) * 100 + d(2);
        let month = d(5);
        let day = d(8);
        let hour = d(11);
        let minute = d(14);
        let second = d(17);

        // 计算从 Unix epoch (1970-01-01) 到给定日期的天数
        let y = year as i64;
        let m = month as i64;
        let mut days = (y - 1970) * 365;
        // 闰年修正
        days += (y - 1969) / 4 - (y - 1901) / 100 + (y - 1601) / 400;
        // 月份天数累积（非闰年）
        static MONTH_DAYS: [i64; 13] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334, 365];
        days += MONTH_DAYS[m as usize - 1] + day as i64 - 1;
        // 润年且月份 > 2 时加一天
        if m > 2 && ((y % 4 == 0 && y % 100 != 0) || y % 400 == 0) {
            days += 1;
        }

        days * 86400 + hour as i64 * 3600 + minute as i64 * 60 + second as i64
    }

    /// 从 HTML 样例中提取纯文本 input/output 对。
    ///
    /// HOJ 的 `examples` 字段格式为成对的 `<input>...</input><output>...</output>`，
    /// 例如 `<input>1\n8\n00100100</input><output>Yes</output>`。
    /// 每个 `<input>` 与紧随其后的 `<output>` 构成一组样例。
    fn parse_samples(html: &str) -> Vec<Sample> {
        if html.is_empty() {
            return Vec::new();
        }

        let inputs = extract_tag_contents(html, "input");
        let outputs = extract_tag_contents(html, "output");

        let mut samples = Vec::new();
        for (i, input) in inputs.iter().enumerate() {
            let output = outputs.get(i).cloned().unwrap_or_default();
            samples.push(Sample {
                input: input.clone(),
                output,
            });
        }
        samples
    }
}

/// 从响应头检测 HOJ 服务端的 token 轮换，返回新 token（未轮换返回 `None`）。
///
/// 这是 HOJ 私有的协议语义：服务端在 token 到期前于响应头附加
/// `Refresh-Token: true` 和新的 `Authorization` 头。该约定只属于 HOJ Adapter，
/// 不应上移到 OJ 无关的 infra 层。
fn extract_refreshed_token(headers: &reqwest::header::HeaderMap) -> Option<String> {
    if headers.contains_key("refresh-token") {
        headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
    } else {
        None
    }
}

/// 截取响应体前 200 字符用于错误诊断。
///
/// 解析失败时最有价值的信息是「服务端究竟返回了什么」—— 网关 502 的 HTML 页、
/// 未登录的重定向页、以及真正的 JSON 结构变更，三者处置方式完全不同。
/// 只取前缀，避免把上百 KB 的响应体整段写进错误消息与日志。
fn preview(body: &str) -> String {
    const MAX_CHARS: usize = 200;
    let trimmed = body.trim();
    if trimmed.chars().count() <= MAX_CHARS {
        return trimmed.to_string();
    }
    // 按字符而非字节截断：响应含中文比赛标题，按字节切会落在 UTF-8 序列中间
    let cut: String = trimmed.chars().take(MAX_CHARS).collect();
    format!("{}…", cut)
}

/// 提取指定 HTML 标签的内容（支持 `<tag>` 与 `<tag attr="...">` 形式）。
fn extract_tag_contents(html: &str, tag: &str) -> Vec<String> {
    let mut result = Vec::new();
    let open_marker = format!("<{}", tag);
    let close_marker = format!("</{}>", tag);
    let mut remaining = html;

    while let Some(start) = remaining.find(&open_marker) {
        let after_tag = &remaining[start + open_marker.len()..];
        // 跳过标签属性直到 '>'
        let Some(close_bracket) = after_tag.find('>') else { break };
        let after_open = &after_tag[close_bracket + 1..];
        let Some(end) = after_open.find(&close_marker) else { break };
        let content = &after_open[..end];
        result.push(unescape_html(content.trim()));
        remaining = &after_open[end + close_marker.len()..];
    }

    result
}

/// 反转义常见的 HTML 实体与 `<br>` 换行标签。
fn unescape_html(s: &str) -> String {
    s.replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

#[cfg(test)]
#[path = "tests/mod_tests.rs"]
mod tests;

// ── AuthProvider ──

#[async_trait]
impl AuthProvider for HOJAdapter {
    async fn login(&self, username: &str, password: &str) -> AppResult<User> {
        let url = self.api_url("/login");
        // 注意：HOJ 服务端会对收到的密码自行 MD5 后比对，
        // 客户端必须发送明文密码（详见 PassportManager.login 的 SecureUtil.md5 逻辑）。
        let body = LoginRequest {
            username: username.to_string(),
            password: password.to_string(),
        };

        info!(username = username, "HOJ 登录请求");

        // 直接调用底层 reqwest client 以获取响应头中的 token
        let response = self
            .http
            .client()
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Network(format!("HOJ login 请求失败: {}", e)))?;

        // 从响应头提取 token
        let token = response
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .ok_or_else(|| AppError::Auth("HOJ 登录响应未包含 token".into()))?;

        // 解析响应体：先取原始文本便于诊断，再反序列化
        let raw_body = response
            .text()
            .await
            .map_err(|e| AppError::Serialization(format!("HOJ login 读取响应体失败: {}", e)))?;
        // 解析响应体：走统一的「去 null」解析入口。
        // HOJ 对未设置字段返回 null（如 roleList），直接 from_str 会报 invalid type；
        // 错误消息里已含响应体前缀，无需再把整个 body 写进日志。
        let api_resp: ApiResponse<UserInfoVO> = match Self::parse_hoj_json(&raw_body, &url) {
            Ok(v) => v,
            Err(e) => {
                warn!(error = %e, "HOJ login 响应解析失败");
                return Err(e);
            }
        };

        let user_info = api_resp.into_data().map_err(|msg| {
            warn!(error = msg, "HOJ 登录失败");
            AppError::Auth(format!("HOJ 登录失败: {}", msg))
        })?;

        info!(username = user_info.username, "HOJ 登录成功");
        let token_for_user = token.clone();
        self.set_token(token);
        Ok(User {
            id: user_info.uid,
            username: user_info.username,
            token: token_for_user,
        })
    }

    async fn logout(&self) -> AppResult<()> {
        let url = self.api_url("/logout");
        let token = self.get_token();

        if token.is_none() {
            debug!("HOJ 无 token，跳过远端登出");
            self.clear_token();
            return Ok(());
        }

        // 忽略远端响应（可能失败），以清除 token 为主
        let _ = self
            .http
            .client()
            .get(&url)
            .header("Authorization", &token.unwrap())
            .send()
            .await;

        self.clear_token();
        info!("HOJ 已登出");
        Ok(())
    }

    /// 校验会话有效性。
    ///
    /// HOJ 无专门的 session 校验接口，改用需认证的 `get-user-auth-info` 间接验证。
    ///
    /// **三态契约**（`AuthService::validate_session` 依赖它区分「服务端判定失效」与「无法判定」）：
    /// - `Ok(true)`  服务端确认有效
    /// - `Ok(false)` 服务端**明确**判定失效 → 清磁盘会话 + 发布 SessionExpired + 前端登出
    /// - `Err(_)`    无法判定（网络异常/超时/5xx/解析失败）→ 映射为 `Unknown`，**保留**本地会话
    ///
    /// 绝不能把网络错误折成 `Ok(false)`：那会让赛前一次网络抖动就把选手踢回登录页，
    /// 而反复重登可能触发 HOJ 的暴力破解锁定（同 IP + 同用户名 30 分钟 20 次）。
    async fn validate_session(&self) -> AppResult<bool> {
        // 本地无 token：无需发请求即可判定失效（不是网络问题）
        if self.get_token().is_none() {
            debug!("本地无 token，会话判定为失效");
            return Ok(false);
        }

        let url = self.api_url("/get-user-auth-info");
        Self::session_validity_from_response(self.get_json_authed(&url).await)
    }

    fn restore_token(&self, token: &str) {
        if !token.is_empty() {
            self.set_token(token.to_string());
        }
    }
}

// ── ContestProvider ──

#[async_trait]
impl ContestProvider for HOJAdapter {
    async fn list_contests(&self) -> AppResult<Vec<Contest>> {
        let url = self.api_url("/get-contest-list?limit=1000");
        let api_resp = self
            .get_json_authed::<ApiResponse<types::PageResult<ContestVO>>>(&url)
            .await
            .map_err(|e| e.context("HOJ contest list"))?;

        let page = api_resp.into_data().map_err(|msg| {
            AppError::Contest(format!("HOJ contest list 失败: {}", msg))
        })?;

        let contests: Vec<Contest> = page
            .records
            .into_iter()
            .map(Self::into_contest)
            .collect();

        debug!(count = contests.len(), "HOJ 比赛列表已获取");
        Ok(contests)
    }

    async fn get_contest(&self, contest_id: &str) -> AppResult<Contest> {
        let url = self.api_url(&format!("/get-contest-info?cid={}", contest_id));

        let api_resp = self
            .get_json_authed::<ApiResponse<ContestVO>>(&url)
            .await
            .map_err(|e| e.context("HOJ contest info"))?;

        let c = api_resp.into_data().map_err(|msg| {
            AppError::Contest(format!("HOJ contest info 失败: {}", msg))
        })?;

        Ok(Self::into_contest(c))
    }

    async fn list_contest_problems(&self, contest_id: &str) -> AppResult<Vec<ContestProblem>> {
        let url = self.api_url(&format!("/get-contest-problem?cid={}", contest_id));

        let api_resp = self
            .get_json_authed::<ApiResponse<Vec<ContestProblemVO>>>(&url)
            .await
            .map_err(|e| e.context("HOJ contest problem list"))?;

        let problem_list = api_resp.into_data().map_err(|msg| {
            AppError::Contest(format!("HOJ contest problem list 失败: {}", msg))
        })?;

        let problems: Vec<ContestProblem> = problem_list
            .into_iter()
            .map(|p| ContestProblem {
                id: p.id,
                display_id: p.display_id,
                cid: p.cid,
                problem_id: p.pid.to_string(),
                display_title: p.display_title,
                ac: p.ac,
                total: p.total,
                color: p.color,
            })
            .collect();

        Ok(problems)
    }

    async fn get_contest_rank(
        &self,
        contest_id: &str,
        query: &RankQuery,
    ) -> AppResult<ContestRankPage> {
        let cid: i64 = Self::parse_cid(contest_id)?;

        let url = self.api_url("/get-contest-rank");
        let body = ContestRankDTO {
            cid,
            current_page: query.current_page.max(1),
            // 榜单为全量计算后分页，limit 越大单次越慢；上限做防御性收敛
            limit: query.limit.clamp(1, 200),
            // 非比赛创建者/超管传 true 会被服务端忽略，故恒为 false
            force_refresh: false,
            remove_star: query.remove_star,
            keyword: query.keyword.clone().filter(|k| !k.trim().is_empty()),
            contains_end: query.contains_end,
            concerned_list: Vec::new(),
            external_cid_list: None,
        };

        let api_resp = self
            .post_json_authed::<ApiResponse<types::PageResult<ContestRankVO>>, _>(&url, &body)
            .await
            .map_err(|e| e.context("HOJ contest rank"))?;

        let page = api_resp
            .into_data()
            .map_err(|msg| AppError::Contest(format!("HOJ contest rank 失败: {}", msg)))?;

        // 注意：records 可能含服务端前置的「当前用户/关注用户」副本，
        // 去重与真实参赛人数推导由前端按 uid 处理（见 ContestRankPage 文档注释）
        let records = page.records.into_iter().map(ContestRankVO::into_rank_row).collect();

        debug!(contest_id = contest_id, "HOJ 比赛榜单已获取");
        Ok(ContestRankPage {
            records,
            total: page.total,
            size: page.size,
            current: page.current,
            pages: page.pages,
        })
    }
}

// ── ProblemProvider ──

#[async_trait]
impl ProblemProvider for HOJAdapter {
    async fn list_problems(&self, contest_id: &str) -> AppResult<Vec<Problem>> {
        let url = self.api_url(&format!("/get-contest-problem?cid={}", contest_id));

        let api_resp = self
            .get_json_authed::<ApiResponse<Vec<ContestProblemVO>>>(&url)
            .await
            .map_err(|e| e.context("HOJ contest problem list"))?;

        let problem_list = api_resp.into_data().map_err(|msg| {
            AppError::Problem(format!("HOJ contest problem list 失败: {}", msg))
        })?;

        let problems: Vec<Problem> = problem_list
            .into_iter()
            .map(|p| Problem {
                id: p.pid.to_string(),
                title: if p.display_title.is_empty() {
                    format!("Problem {}", p.display_id)
                } else {
                    p.display_title
                },
                description: String::new(),
                input_description: String::new(),
                output_description: String::new(),
                samples: Vec::new(),
                time_limit: 0,
                memory_limit: 0,
            })
            .collect();

        debug!(contest_id = contest_id, count = problems.len(), "HOJ 比赛题目列表已获取");
        Ok(problems)
    }

    async fn get_problem(
        &self,
        contest_id: &str,
        problem_id: &str,
    ) -> AppResult<Problem> {
        // problem_id 在比赛中对应 displayId（如 "A", "B"）
        let url = self.api_url(&format!(
            "/get-contest-problem-details?cid={}&displayId={}",
            contest_id, problem_id
        ));

        let api_resp = self
            .get_json_authed::<ApiResponse<ProblemInfoVO>>(&url)
            .await
            .map_err(|e| e.context("HOJ problem detail"))?;

        let info = api_resp.into_data().map_err(|msg| {
            AppError::Problem(format!("HOJ problem detail 失败: {}", msg))
        })?;

        let samples = Self::parse_samples(info.problem.examples.as_deref().unwrap_or(""));

        let problem = Problem {
            id: info.problem.id.to_string(),
            title: info.problem.title,
            description: info.problem.description.unwrap_or_default(),
            input_description: info.problem.input.unwrap_or_default(),
            output_description: info.problem.output.unwrap_or_default(),
            samples,
            time_limit: info.problem.time_limit as u32,
            memory_limit: info.problem.memory_limit as u32,
        };

        debug!(contest_id = contest_id, problem_id = problem_id, title = problem.title, "HOJ 题目详情已获取");
        Ok(problem)
    }

    async fn get_user_problem_status(
        &self,
        contest_id: &str,
        problem_ids: &[String],
    ) -> AppResult<HashMap<String, i32>> {
        // 空列表直接返回，避免向服务端发无意义请求
        if problem_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let url = self.api_url("/get-user-problem-status");
        let body = UserProblemStatusDTO {
            pid_list: problem_ids.to_vec(),
            is_contest_problem_list: false,
            cid: Self::parse_cid(contest_id)?,
            gid: None,
            contains_end: false,
        };

        let api_resp = self
            .post_json_authed::<ApiResponse<HashMap<String, serde_json::Value>>, _>(&url, &body)
            .await
            .map_err(|e| e.context("HOJ user problem status"))?;

        let raw = api_resp.into_data().map_err(|msg| {
            AppError::Problem(format!("HOJ user problem status 失败: {}", msg))
        })?;

        let statuses = raw
            .into_iter()
            .map(|(pid, value)| (pid, types::coerce_problem_status(&value)))
            .collect();

        debug!(contest_id = contest_id, "HOJ 用户题目状态已获取");
        Ok(statuses)
    }
}

// ── SubmissionProvider ──

#[async_trait]
impl SubmissionProvider for HOJAdapter {
    async fn submit(
        &self,
        contest_id: &str,
        problem_id: &str,
        language: &str,
        source_code: &str,
    ) -> AppResult<String> {
        let url = self.api_url("/submit-problem-judge");
        // 提交必须已登录：前置断言给出明确错误，而不是让服务端回一个 401
        self.require_token()?;

        let cid: i64 = Self::parse_cid(contest_id)?;
        let body = SubmitRequest {
            pid: problem_id.to_string(),
            language: language.to_string(),
            code: source_code.to_string(),
            cid,
            tid: None,
            gid: None,
            is_remote: false,
        };

        info!(contest_id = contest_id, problem_id = problem_id, language = language, "HOJ 提交代码");

        // 走统一的 POST 封装：原始响应体 → 去 null 解析 → token 轮换。
        // 此前这里自带一份轮换逻辑，与 get/post_json_authed 的实现容易漂移
        let api_resp = self
            .post_json_authed::<ApiResponse<JudgeVO>, _>(&url, &body)
            .await
            .map_err(|e| e.context("HOJ submit"))?;

        let judge = api_resp.into_data().map_err(|msg| {
            warn!(error = msg, "HOJ 提交失败");
            AppError::Submission(format!("HOJ 提交失败: {}", msg))
        })?;

        let submission_id = judge.submit_id.to_string();
        debug!(submission_id = submission_id, "HOJ 代码已提交");
        Ok(submission_id)
    }

    async fn get_judgement(&self, submission_id: &str) -> AppResult<JudgementResult> {
        let url = self.api_url(&format!("/get-submission-detail?submitId={}", submission_id));

        // 走统一 GET 封装：带上 token（若已登录）、处理轮换、去 null 解析。
        // 此前固定传 None：评测详情在需要认证的部署上会直接 401，
        // 且轮询期间感知不到 token 轮换，长时间比赛会出现周期性查询失败。
        // 另外 SubmissionDetail 的 time/memory 在评测未完成时为 null，
        // 不经去 null 处理会让整个轮询链路解析失败。
        let api_resp = self
            .get_json_authed::<ApiResponse<types::SubmissionInfoVO>>(&url)
            .await
            .map_err(|e| e.context("HOJ judgement"))?;

        let info = api_resp.into_data().map_err(|msg| {
            AppError::Submission(format!("HOJ 评测查询失败: {}", msg))
        })?;

        let detail = &info.submission;
        let status = map_status(detail.status);

        // 非终态返回 Running，供上层按 JudgementStatus::Running 继续轮询
        if !types::is_terminal_status(detail.status) {
            return Ok(JudgementResult {
                status: JudgementStatus::Running,
                score: 0.0,
                time_ms: 0,
                memory_kb: 0,
            });
        }

        let result = JudgementResult {
            status,
            score: detail.score.unwrap_or(0.0),
            time_ms: detail.time as u64,
            memory_kb: detail.memory as u64,
        };

        debug!(
            submission_id = submission_id,
            status = ?result.status,
            time_ms = result.time_ms,
            "HOJ 评测结果"
        );

        Ok(result)
    }
}
