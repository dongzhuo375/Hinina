// 认证服务：登录流程编排、会话持久化、登出清理。
//
// 会话以 JSON 格式存储在 `sessions/{oj_id}.json`，v0.x 明文存储。
// v1.0 后考虑引入加密或系统凭据管理器。
pub mod error;

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::core::entity::user::User;
use crate::core::error::AppResult;
use crate::core::event::app_event::{AppEvent, AuthEvent};
use crate::core::event::event_bus::EventBus;
use crate::core::event::event_category::EventCategory;
use crate::core::provider::registry::ProviderRegistry;
use crate::infra::storage::Storage;

/// 会话持久化目录。
const SESSIONS_DIR: &str = "sessions";

/// 本地会话记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// 用户 ID（UUID），`#[serde(default)]` 兼容升级前不含此字段的旧版 session 文件。
    #[serde(default)]
    pub user_id: String,
    pub username: String,
    pub token: String,
    /// 会话归属的 OJ id（旧版文件键名为 `oj_type`，经 alias 兼容读取）
    #[serde(alias = "oj_type")]
    pub oj_id: String,
}

/// 会话校验结果（三态），经 IPC 以 snake_case 字符串传递给前端。
///
/// 必须区分"服务端明确判定失效"与"无法判定"：前者要让用户重新登录，
/// 后者（网络抖动、Provider 缺失）若同样按失效处理，会在赛前把选手踢回登录页，
/// 反复重登还可能触发 HOJ 的暴力破解锁定（同 IP + 同用户名 30 分钟 20 次）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionValidity {
    /// 服务端确认会话有效
    Valid,
    /// 本地无会话，或服务端明确判定失效（此时磁盘会话已清除）
    Invalid,
    /// 无法判定（网络异常 / Provider 不可用），本地会话保留
    Unknown,
}

/// 认证服务。
///
/// 编排登录/登出流程，管理本地会话持久化。
/// 通过 ProviderRegistry 获取当前 OJ 的 AuthProvider，
/// 支持运行时 OJ 切换后自动适配。
///
/// 构造时订阅 `EventCategory::Auth`：Provider 侧发布 `TokenRefreshed` 时
/// 将新凭证回写磁盘会话，保证重启后 `get_session` 恢复的是最新 token。
/// 订阅句柄由 EventBus 持有，随 AuthService 生命周期共存（进程级单例，无泄漏风险）。
pub struct AuthService {
    registry: Arc<dyn ProviderRegistry>,
    storage: Arc<Storage>,
    event_bus: Arc<EventBus>,
}

impl AuthService {
    /// 创建 AuthService。
    pub fn new(
        registry: Arc<dyn ProviderRegistry>,
        storage: Arc<Storage>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        let service = Self {
            registry,
            storage,
            event_bus,
        };
        service.subscribe_token_refresh();
        service
    }

    /// 订阅 Provider 凭证轮换事件，将新 token 持久化到当前 OJ 的磁盘会话。
    ///
    /// 事件回调为同步闭包（EventBus 约定），此处只做文件读写，不阻塞异步运行时。
    /// 磁盘会话可能尚不存在（轮换发生在登录持久化之前的极端时序），此时跳过回写。
    fn subscribe_token_refresh(&self) {
        let storage = Arc::clone(&self.storage);
        let registry = Arc::clone(&self.registry);
        self.event_bus.subscribe(
            EventCategory::Auth,
            Arc::new(move |event: &AppEvent| {
                let AppEvent::Auth(AuthEvent::TokenRefreshed { token }) = event else {
                    return;
                };
                let oj_id = registry.current_oj();
                let path = format!("{}/{}", SESSIONS_DIR, oj_id.session_file());
                let Ok(raw) = storage.read_to_string(&path) else {
                    return;
                };
                let Ok(mut session) = serde_json::from_str::<Session>(&raw) else {
                    warn!(path = %path, "凭证轮换回写失败：会话文件解析错误");
                    return;
                };
                session.token = token.clone();
                match serde_json::to_string_pretty(&session) {
                    Ok(json) => {
                        if let Err(e) = storage.write_string(&path, &json) {
                            warn!(error = %e, path = %path, "凭证轮换回写失败：写入会话文件错误");
                        } else {
                            debug!(path = %path, "凭证轮换已回写磁盘会话");
                        }
                    }
                    Err(e) => warn!(error = %e, "凭证轮换回写失败：会话序列化错误"),
                }
            }),
        );
    }

    /// 登录：调用 AuthProvider → 保存会话 → 发布事件。
    ///
    /// 登录成功后持久化 session 到 `sessions/{oj_id}.json`。
    pub async fn login(&self, username: &str, password: &str) -> AppResult<User> {
        let oj_id = self.registry.current_oj();
        let provider = self.registry.get_auth(&oj_id)?;

        info!(username = username, oj = %oj_id, "尝试登录");
        let user = provider.login(username, password).await.map_err(|e| {
            warn!(username = username, error = %e, "登录失败");
            // 保留变体：密码错误（Auth）与网络中断（Network）对用户的处置完全不同，
            // 一律改写成 Auth 会让「服务器连不上」显示成「认证错误」
            e.context("登录失败")
        })?;

        // 持久化会话
        let session = Session {
            user_id: user.id.clone(),
            username: user.username.clone(),
            token: user.token.clone(),
            oj_id: oj_id.to_string(),
        };
        self.save_session(&session)?;

        info!(username = user.username, "登录成功");
        self.event_bus
            .publish(&AppEvent::Auth(AuthEvent::LoginSuccess {
                user: user.clone(),
            }));

        Ok(user)
    }

    /// 登出：删除本地会话 → 发布事件。
    ///
    /// 即便远端 logout 失败，也会清除本地会话并发布事件。
    pub async fn logout(&self) -> AppResult<()> {
        let oj_id = self.registry.current_oj();

        // 尝试远端登出（非致命错误）
        if let Ok(provider) = self.registry.get_auth(&oj_id) {
            if let Err(e) = provider.logout().await {
                warn!(error = %e, "远端登出失败，仅清除本地会话");
            }
        }

        self.clear_session(&oj_id);

        info!("已登出");
        self.event_bus.publish(&AppEvent::Auth(AuthEvent::Logout));
        Ok(())
    }

    /// 从本地文件恢复会话。
    ///
    /// 返回 `None` 表示无已保存的会话（从未登录或已登出）。
    ///
    /// 恢复成功时将 token 回注到 Provider，确保重启后认证请求仍携带 Authorization 头。
    pub fn get_session(&self) -> Option<Session> {
        let oj_id = self.registry.current_oj();
        let path = self.session_path(&oj_id);

        if !self.storage.exists(&path) {
            return None;
        }

        match self.storage.read_to_string(&path) {
            Ok(raw) => match serde_json::from_str::<Session>(&raw) {
                Ok(session) => {
                    // 将 token 回注到 Provider，保证后续认证接口可用
                    if let Ok(provider) = self.registry.get_auth(&oj_id) {
                        provider.restore_token(&session.token);
                    }
                    debug!(username = session.username, "会话已恢复");
                    Some(session)
                }
                Err(e) => {
                    warn!(error = %e, path = %path, "会话文件 JSON 解析失败");
                    None
                }
            },
            Err(e) => {
                warn!(error = %e, path = %path, "会话文件读取失败");
                None
            }
        }
    }

    /// 校验当前会话有效性（三态）。
    ///
    /// 先从本地恢复 session（`get_session` 会把 token 回注 Provider，
    /// 否则重启后的校验请求会因缺少 Authorization 头而必然 401），再请求远端校验：
    /// - 本地无会话 → `Invalid`
    /// - 远端确认有效 → `Valid`
    /// - 远端明确判定失效 → 清除磁盘会话 + 发布 `SessionExpired`，返回 `Invalid`
    /// - 网络异常 / 无 Provider → `Unknown`（保留本地会话，由调用方决定重试）
    pub async fn validate_session(&self) -> SessionValidity {
        let oj_id = self.registry.current_oj();
        if self.get_session().is_none() {
            debug!("本地无会话，判定为未登录");
            return SessionValidity::Invalid;
        }

        let provider = match self.registry.get_auth(&oj_id) {
            Ok(p) => p,
            Err(e) => {
                warn!(error = %e, "获取 AuthProvider 失败，会话有效性无法判定");
                return SessionValidity::Unknown;
            }
        };

        match provider.validate_session().await {
            Ok(true) => {
                debug!("会话校验通过");
                SessionValidity::Valid
            }
            Ok(false) => {
                // 服务端明确判定失效：清除磁盘会话，避免重启后回注过期 token
                info!("会话已失效，清除本地会话");
                self.clear_session(&oj_id);
                self.event_bus
                    .publish(&AppEvent::Auth(AuthEvent::SessionExpired));
                SessionValidity::Invalid
            }
            Err(e) => {
                // 网络错误不代表会话失效，保留本地会话
                warn!(error = %e, "会话校验请求失败，有效性未知");
                SessionValidity::Unknown
            }
        }
    }

    // ── 内部方法 ──

    /// 删除指定 OJ 的本地会话文件（不存在时静默跳过）。
    fn clear_session(&self, oj_id: &crate::core::provider::oj_id::OjId) {
        let path = self.session_path(oj_id);
        if self.storage.exists(&path) {
            if let Err(e) = self.storage.remove(&path) {
                warn!(error = %e, path = %path, "清除失效会话文件失败");
            } else {
                debug!(path = %path, "失效会话文件已清除");
            }
        }
    }

    /// 保存会话到本地文件。
    fn save_session(&self, session: &Session) -> AppResult<()> {
        let path = self.session_path_str(&session.oj_id);
        // 确保 sessions 目录存在
        self.storage.create_dir(SESSIONS_DIR)?;
        let json = serde_json::to_string_pretty(session)?;
        self.storage.write_string(&path, &json)?;
        debug!(path = %path, "会话已保存");
        Ok(())
    }

    /// 构建会话文件的相对路径。
    fn session_path_str(&self, oj_id: &str) -> String {
        format!("{}/{}.json", SESSIONS_DIR, oj_id)
    }

    /// 同 `session_path_str`，返回 `&str` 借用时需要 Path 的场景。
    fn session_path(&self, oj_id: &crate::core::provider::oj_id::OjId) -> String {
        self.session_path_str(oj_id.as_str())
    }
}

#[cfg(test)]
#[path = "tests/auth_tests.rs"]
mod tests;
