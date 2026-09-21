// 认证服务：登录流程编排、会话持久化、登出清理。
//
// 会话以 JSON 格式存储在 `sessions/{oj_id}.json`，v0.x 明文存储。
// v1.0 后考虑引入加密或系统凭据管理器。
//
// **持久化职责的归属**（本次重构的关键变更）：
// 会话落盘是「必须等待结果的核心动作」，因此**只在显式路径上完成** ——
// 登录时 `AuthService::login` 落盘、Provider 轮换时由适配器经 `SessionRepository`
// 落盘。旧实现让 Provider 发布带真实 token 的 `TokenRefreshed` 事件、由本服务的
// 同步订阅者代劳写盘，既把凭证带进事件流，又把持久性保证挂在异步投递上。
pub mod error;

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

pub use crate::core::entity::session::Session;
use crate::core::entity::user::User;
use crate::core::error::AppResult;
use crate::core::event::core_event::CoreEvent;
use crate::core::event::core_event_bus::CoreEventBus;
use crate::core::provider::oj_id::OjId;
use crate::core::provider::registry::ProviderRegistry;
use crate::core::repository::session_repo::SessionRepository;

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
/// **不订阅任何事件**：本服务承担的每一项工作（落盘、清理）都必须显式完成，
/// 不能交给异步消费者。`LoggedIn` / `LoggedOut` / `SessionExpired` 只是
/// 「这些动作已经做完」的事实通知。
pub struct AuthService {
    registry: Arc<dyn ProviderRegistry>,
    session_repo: Arc<dyn SessionRepository>,
    event_bus: Arc<CoreEventBus>,
}

impl AuthService {
    /// 创建 AuthService。
    pub fn new(
        registry: Arc<dyn ProviderRegistry>,
        session_repo: Arc<dyn SessionRepository>,
        event_bus: Arc<CoreEventBus>,
    ) -> Self {
        Self {
            registry,
            session_repo,
            event_bus,
        }
    }

    /// 登录：调用 AuthProvider → 保存会话 → 发布事实通知。
    ///
    /// 登录成功后**显式**持久化 session 到 `sessions/{oj_id}.json`；
    /// 落盘失败则整体失败（不发布 `LoggedIn`）—— 会话没落盘就宣告登录成功，
    /// 会让重启后的客户端拿着不存在的会话工作。
    pub async fn login(&self, username: &str, password: &str) -> AppResult<User> {
        let oj_id = self.registry.current_id();
        let provider = self.registry.current_auth()?;

        info!(username = username, oj = %oj_id, "尝试登录");
        let user = provider.login(username, password).await.map_err(|e| {
            warn!(username = username, error = %e, "登录失败");
            // 保留变体：密码错误（Auth）与网络中断（Network）对用户的处置完全不同，
            // 一律改写成 Auth 会让「服务器连不上」显示成「认证错误」
            e.context("登录失败")
        })?;

        // 持久化会话（显式，落盘成功才算登录完成）
        let session = Session::new(oj_id.as_str(), &user.id, &user.username, &user.token);
        self.session_repo.save(&session)?;

        info!(username = user.username, "登录成功");
        // 只带 OJ 与用户标识：token 与完整 User 实体绝不进入事件流
        self.event_bus.publish(CoreEvent::LoggedIn {
            oj_id: oj_id.to_string(),
            user_id: user.id.clone(),
        });

        Ok(user)
    }

    /// 登出：远端登出（非致命）→ 显式删除本地会话 → 发布事实通知。
    ///
    /// 即便远端 logout 失败，也会清除本地会话并发布事件。
    /// 用户域缓存（含源代码的提交详情/测试点）的清理由命令层显式编排
    /// （见 `commands::auth_cmd::logout`）—— 同属「必须完成的核心清理」，
    /// 不交给事件消费者。
    pub async fn logout(&self) -> AppResult<()> {
        let oj_id = self.registry.current_id();

        // 尝试远端登出（非致命错误）
        if let Ok(provider) = self.registry.current_auth() {
            if let Err(e) = provider.logout().await {
                warn!(error = %e, "远端登出失败，仅清除本地会话");
            }
        }

        self.clear_session(&oj_id)?;

        info!("已登出");
        self.event_bus.publish(CoreEvent::LoggedOut {
            oj_id: oj_id.to_string(),
        });
        Ok(())
    }

    /// 从本地文件恢复会话。
    ///
    /// 返回 `None` 表示无已保存的会话（从未登录、已登出，或文件不可读）。
    ///
    /// 恢复成功时将 token 回注到 Provider，确保重启后认证请求仍携带 Authorization 头。
    pub fn get_session(&self) -> Option<Session> {
        let oj_id = self.registry.current_id();

        let session = match self.session_repo.load(&oj_id) {
            Ok(session) => session,
            Err(e) => {
                // 仓库层如实报错，应用层降级为「无会话」并留下痕迹 ——
                // 静默吞掉会让「会话文件损坏」表现为「从未登录」，无从排查
                warn!(oj_id = %oj_id, error = %e, "会话文件读取失败，按未登录处理");
                return None;
            }
        };

        let session = session?;
        // 将 token 回注到 Provider，保证后续认证接口可用
        if let Ok(provider) = self.registry.current_auth() {
            provider.restore_token(&session.token);
        }
        debug!(username = session.username, "会话已恢复");
        Some(session)
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
        let oj_id = self.registry.current_id();
        if self.get_session().is_none() {
            debug!("本地无会话，判定为未登录");
            return SessionValidity::Invalid;
        }

        let provider = match self.registry.current_auth() {
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
                if let Err(e) = self.clear_session(&oj_id) {
                    // 清理失败不改变判定结果：本地会话已不可用，前端必须回登录页
                    warn!(oj_id = %oj_id, error = %e, "清除失效会话失败");
                }
                self.event_bus.publish(CoreEvent::SessionExpired {
                    oj_id: oj_id.to_string(),
                });
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

    /// 删除指定 OJ 的本地会话（不存在时静默成功）。
    fn clear_session(&self, oj_id: &OjId) -> AppResult<()> {
        self.session_repo.remove(oj_id)
    }
}

#[cfg(test)]
#[path = "tests/auth_tests.rs"]
mod tests;
