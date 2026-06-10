use serde::{Deserialize, Serialize};

/// Workspace — 核心领域对象。
///
/// 负责代码存储、自动保存、崩溃恢复、比赛隔离、模板管理、缓存。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub contest_id: String,
    pub problem_id: String,
    pub root_path: String,
    /// 各文件名 → 文件内容
    pub files: std::collections::HashMap<String, String>,
    pub language: String,
    pub is_dirty: bool,
}
