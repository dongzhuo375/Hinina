use serde::{Deserialize, Serialize};

/// 题目详情
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Problem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub input_description: String,
    pub output_description: String,
    pub samples: Vec<Sample>,
    pub time_limit: u32,
    pub memory_limit: u32,
}

/// 样例数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sample {
    pub input: String,
    pub output: String,
}
