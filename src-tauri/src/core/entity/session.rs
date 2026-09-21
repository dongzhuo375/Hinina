use serde::{Deserialize, Serialize};

/// 本地会话记录 —— 持久化在 `sessions/{oj_id}.json`。
///
/// # 为什么是独立实体
///
/// 会话的**写入方**不止 `AuthService`：Provider 侧凭证轮换（如 HOJ 的
/// Refresh-Token 协议）也必须在拿到新 token 的当场显式落盘。把 `Session`
/// 与它的持久化契约（路径、字段名、兼容别名）收进领域 + 仓库层，
/// 才能让「应用层」与「适配器层」共用同一份 schema —— 否则适配器只能靠
/// 发布带真实 token 的事件，把落盘职责推给应用层的订阅者（旧实现的缺陷）。
///
/// **schema 与旧版完全一致**（v0.x 明文存储，v1.0 后再考虑加密 / 系统凭据管理器），
/// 因此既有会话文件无需迁移。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Session {
    /// 用户 ID（UUID），`#[serde(default)]` 兼容升级前不含此字段的旧版 session 文件。
    #[serde(default)]
    pub user_id: String,
    pub username: String,
    /// 访问凭证。**绝不允许进入事件流**（见 `core::event::core_event`）。
    pub token: String,
    /// 会话归属的 OJ id（旧版文件键名为 `oj_type`，经 alias 兼容读取）
    #[serde(alias = "oj_type")]
    pub oj_id: String,
}

impl Session {
    /// 组装一条会话记录。
    #[must_use]
    pub fn new(oj_id: &str, user_id: &str, username: &str, token: &str) -> Self {
        Self {
            user_id: user_id.to_string(),
            username: username.to_string(),
            token: token.to_string(),
            oj_id: oj_id.to_string(),
        }
    }
}

#[cfg(test)]
#[path = "tests/session_tests.rs"]
mod tests;
