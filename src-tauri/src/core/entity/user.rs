use serde::{Deserialize, Serialize};

/// 用户身份与会话
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub token: String,
}
