use serde::{Deserialize, Serialize};

/// 比赛信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contest {
    pub id: String,
    pub title: String,
    pub start_time: i64,
    pub end_time: i64,
    pub problems: Vec<String>,
}
