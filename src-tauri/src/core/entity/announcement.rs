use serde::{Deserialize, Serialize};

/// 比赛公告。
///
/// 时间为 UTC 秒级时间戳，各 Adapter 负责统一转换。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Announcement {
    pub id: String,
    pub title: String,
    /// 公告正文（HTML/Markdown）
    pub content: String,
    /// 发布者用户名
    pub author: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 公告分页结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnouncementPage {
    pub records: Vec<Announcement>,
    pub total: i64,
    pub size: i64,
    pub current: i64,
    pub pages: i64,
}
