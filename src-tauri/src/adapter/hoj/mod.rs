// HOJ Adapter：实现 Auth + Contest + Problem + Submission 四个 trait。
//
// HOJ (Hydro Online Judge) API 基于 JWT 认证，统一响应格式 {status, msg, data}。
// Token 在登录响应的 `authorization` 头中返回，后续请求通过该头传递。
pub mod types;
pub mod error;

use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use tracing::{debug, info, warn};

use crate::core::entity::contest::{Contest, ContestProblem};
use crate::core::entity::problem::{Problem, Sample};
use crate::core::entity::submission::{JudgementResult, JudgementStatus};
use crate::core::entity::user::User;
use crate::core::error::{AppError, AppResult};
use crate::core::provider::auth::AuthProvider;
use crate::core::provider::contest::ContestProvider;
use crate::core::provider::problem::ProblemProvider;
use crate::core::provider::submission::SubmissionProvider;
use crate::infra::http::HttpClient;

use self::types::{
    map_status, ApiResponse, ContestProblemVO, ContestVO, JudgeVO, LoginRequest,
    ProblemInfoVO, SubmitRequest, UserInfoVO,
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
}

impl HOJAdapter {
    /// 创建 HOJAdapter。
    ///
    /// `base_url` 不含尾部 `/api`，如 `https://hoj.dongzhuo.top`。
    pub fn new(http: Arc<HttpClient>, base_url: String) -> Self {
        // 去掉尾部斜杠以统一拼接
        let base_url = base_url.trim_end_matches('/').to_string();
        Self {
            http,
            base_url,
            token: RwLock::new(None),
        }
    }

    /// 构造完整 API URL。
    fn api_url(&self, path: &str) -> String {
        format!("{}/api{}", self.base_url, path)
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

    /// 发送带认证的 GET 请求，自动处理服务端 token 轮换。
    ///
    /// HOJ 服务端会在 token 到期前返回 `Refresh-Token: true` 和新 `Authorization` 头。
    /// 此方法在收到轮换的新 token 时自动更新内部存储，避免后续请求 401。
    async fn get_json_authed<T: serde::de::DeserializeOwned>(&self, url: &str) -> AppResult<T> {
        let token = self.get_token();
        let (data, headers) = self
            .http
            .get_json_with_headers::<T>(url, token.as_deref())
            .await?;
        if let Some(new_token) = extract_refreshed_token(&headers) {
            debug!("HOJ token 已轮换，更新本地缓存");
            self.set_token(new_token);
        }
        Ok(data)
    }

    // ── 工具方法 ──

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
        let api_resp: ApiResponse<UserInfoVO> = serde_json::from_str(&raw_body).map_err(|e| {
            warn!(body = raw_body, error = %e, "HOJ login 响应解析失败");
            AppError::Serialization(format!("HOJ login 响应解析失败: {}", e))
        })?;

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

    async fn validate_session(&self) -> AppResult<bool> {
        // HOJ 无专门 session 校验接口，通过调用需认证的接口间接验证
        // 尝试获取比赛列表（需要认证的端点）来判断 token 是否有效
        let url = self.api_url("/get-user-auth-info");
        let token = match self.get_token() {
            Some(t) => t,
            None => return Ok(false),
        };

        let response = self
            .http
            .client()
            .get(&url)
            .header("Authorization", &token)
            .send()
            .await;

        match response {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(_) => Ok(false),
        }
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
            .map_err(|e| AppError::Network(format!("HOJ contest list 请求失败: {}", e)))?;

        let page = api_resp.into_data().map_err(|msg| {
            AppError::Contest(format!("HOJ contest list 失败: {}", msg))
        })?;

        let contests: Vec<Contest> = page
            .records
            .into_iter()
            .map(|c| Contest {
                id: c.id.to_string(),
                title: c.title,
                start_time: Self::parse_time(&c.start_time),
                end_time: Self::parse_time(&c.end_time),
                description: c.description.unwrap_or_default(),
                contest_type: c.r#type,
                status: c.status,
                auth: c.auth,
            })
            .collect();

        debug!(count = contests.len(), "HOJ 比赛列表已获取");
        Ok(contests)
    }

    async fn get_contest(&self, contest_id: &str) -> AppResult<Contest> {
        let url = self.api_url(&format!("/get-contest-info?cid={}", contest_id));

        let api_resp = self
            .get_json_authed::<ApiResponse<ContestVO>>(&url)
            .await
            .map_err(|e| AppError::Network(format!("HOJ contest info 请求失败: {}", e)))?;

        let c = api_resp.into_data().map_err(|msg| {
            AppError::Contest(format!("HOJ contest info 失败: {}", msg))
        })?;

        Ok(Contest {
            id: c.id.to_string(),
            title: c.title,
            start_time: Self::parse_time(&c.start_time),
            end_time: Self::parse_time(&c.end_time),
            description: c.description.unwrap_or_default(),
            contest_type: c.r#type,
            status: c.status,
            auth: c.auth,
        })
    }

    async fn list_contest_problems(&self, contest_id: &str) -> AppResult<Vec<ContestProblem>> {
        let url = self.api_url(&format!("/get-contest-problem?cid={}", contest_id));

        let api_resp = self
            .get_json_authed::<ApiResponse<Vec<ContestProblemVO>>>(&url)
            .await
            .map_err(|e| AppError::Network(format!("HOJ contest problem list 请求失败: {}", e)))?;

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
            })
            .collect();

        Ok(problems)
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
            .map_err(|e| AppError::Network(format!("HOJ contest problem list 请求失败: {}", e)))?;

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
            .map_err(|e| AppError::Network(format!("HOJ problem detail 请求失败: {}", e)))?;

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
        let token = self
            .get_token()
            .ok_or_else(|| AppError::Auth("请先登录".into()))?;

        let cid: i64 = contest_id.parse().unwrap_or(0);
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

        // 使用返回响应头的 POST，检测 HOJ 私有 token 轮换语义
        let (api_resp, headers) = self
            .http
            .post_json_with_headers::<ApiResponse<JudgeVO>, _>(&url, &body, Some(&token))
            .await
            .map_err(|e| AppError::Submission(format!("HOJ submit 请求失败: {}", e)))?;
        if let Some(new_token) = extract_refreshed_token(&headers) {
            debug!("HOJ token 已轮换（submit），更新本地缓存");
            self.set_token(new_token);
        }

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

        // 使用 get_json 统一走重试逻辑（而非裸 client.get）
        let api_resp = self
            .http
            .get_json::<ApiResponse<types::SubmissionInfoVO>>(&url, None)
            .await
            .map_err(|e| AppError::Network(format!("HOJ judgement 请求失败: {}", e)))?;

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
