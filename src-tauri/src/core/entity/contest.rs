use serde::{Deserialize, Serialize};

/// 比赛信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contest {
    pub id: String,
    pub title: String,
    /// 比赛开始时间（UTC 秒级时间戳），各 Adapter 负责统一转换
    pub start_time: i64,
    /// 比赛结束时间（UTC 秒级时间戳），各 Adapter 负责统一转换
    pub end_time: i64,
    pub problems: Vec<String>,
}
