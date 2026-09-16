use serde::{Deserialize, Serialize};

/// 题目详情
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub input_description: String,
    pub output_description: String,
    pub samples: Vec<Sample>,
    pub time_limit: u32,
    pub memory_limit: u32,
    /// 题目允许的提交语言（HOJ 显示名，如 "C++"）；来自 get-contest-problem-details，
    /// 空列表表示服务端未提供，前端回退内置默认。
    #[serde(default)]
    pub languages: Vec<String>,
}

/// 样例数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    pub input: String,
    pub output: String,
}
