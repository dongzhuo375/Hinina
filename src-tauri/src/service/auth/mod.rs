// 认证服务：登录流程编排、会话持久化、登出清理。
//
// 会话以 JSON 格式存储在 `sessions/{oj_type}.json`，v0.x 明文存储。
// v1.0 后考虑引入加密或系统凭据管理器。
pub mod error;

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::core::entity::user::User;
use crate::core::error::{AppError, AppResult};
use crate::core::event::app_event::{AppEvent, AuthEvent};
use crate::core::event::event_bus::EventBus;
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
    pub oj_type: String,
}

/// 认证服务。
///
/// 编排登录/登出流程，管理本地会话持久化。
/// 通过 ProviderRegistry 获取当前 OJ 的 AuthProvider，
/// 支持运行时 OJ 切换后自动适配。
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
        Self {
            registry,
            storage,
            event_bus,
        }
    }

    /// 登录：调用 AuthProvider → 保存会话 → 发布事件。
    ///
    /// 登录成功后持久化 session 到 `sessions/{oj_type}.json`。
    pub async fn login(&self, username: &str, password: &str) -> AppResult<User> {
        let oj_type = self.registry.current_oj();
        let provider = self.registry.get_auth(&oj_type)?;

        info!(username = username, oj = ?oj_type, "尝试登录");
        let user = provider.login(username, password).await.map_err(|e| {
            warn!(username = username, error = %e, "登录失败");
            AppError::Auth(format!("登录失败: {}", e))
        })?;

        // 持久化会话
        let session = Session {
            user_id: user.id.clone(),
            username: user.username.clone(),
            token: user.token.clone(),
            oj_type: format!("{:?}", oj_type),
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
        let oj_type = self.registry.current_oj();

        // 尝试远端登出（非致命错误）
        if let Ok(provider) = self.registry.get_auth(&oj_type) {
            if let Err(e) = provider.logout().await {
                warn!(error = %e, "远端登出失败，仅清除本地会话");
            }
        }

        let session_path = self.session_path(&oj_type);
        if self.storage.exists(&session_path) {
            self.storage.remove(&session_path)?;
            debug!(path = %session_path, "会话文件已删除");
        }

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
        let oj_type = self.registry.current_oj();
        let path = self.session_path(&oj_type);

        if !self.storage.exists(&path) {
            return None;
        }

        match self.storage.read_to_string(&path) {
            Ok(raw) => match serde_json::from_str::<Session>(&raw) {
                Ok(session) => {
                    // 将 token 回注到 Provider，保证后续认证接口可用
                    if let Ok(provider) = self.registry.get_auth(&oj_type) {
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

    /// 验证当前会话是否有效。
    ///
    /// 先从本地恢复 session，再调用远端 validate_session。
    /// 如果本地无 session 或远端校验失败，返回 `false`。
    pub async fn validate_session(&self) -> bool {
        let oj_type = self.registry.current_oj();
        let _session = match self.get_session() {
            Some(s) => s,
            None => return false,
        };

        let provider = match self.registry.get_auth(&oj_type) {
            Ok(p) => p,
            Err(e) => {
                warn!(error = %e, "获取 AuthProvider 失败");
                return false;
            }
        };

        match provider.validate_session().await {
            Ok(valid) => {
                if !valid {
                    debug!("会话已过期");
                    self.event_bus
                        .publish(&AppEvent::Auth(AuthEvent::SessionExpired));
                }
                valid
            }
            Err(e) => {
                warn!(error = %e, "会话验证请求失败");
                false
            }
        }
    }

    // ── 内部方法 ──

    /// 保存会话到本地文件。
    fn save_session(&self, session: &Session) -> AppResult<()> {
        let path = self.session_path_str(&session.oj_type);
        // 确保 sessions 目录存在
        self.storage.create_dir(SESSIONS_DIR)?;
        let json = serde_json::to_string_pretty(session)?;
        self.storage.write_string(&path, &json)?;
        debug!(path = %path, "会话已保存");
        Ok(())
    }

    /// 构建会话文件的相对路径。
    fn session_path_str(&self, oj_type: &str) -> String {
        format!("{}/{}.json", SESSIONS_DIR, oj_type)
    }

    /// 同 `session_path_str`，返回 `&str` 借用时需要 Path 的场景。
    fn session_path(&self, oj_type: &crate::core::provider::oj_type::OJType) -> String {
        self.session_path_str(&format!("{:?}", oj_type))
    }
}
