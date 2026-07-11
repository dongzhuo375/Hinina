use serde::{Deserialize, Serialize};

/// 获取当前 UTC 时间戳（秒级），用于 Workspace 时间戳字段。
fn utc_now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

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
    /// 创建时间（UTC 秒级时间戳）
    pub created_at: i64,
    /// 最后修改时间（UTC 秒级时间戳），崩溃恢复时用于判断最近活跃工作区
    pub updated_at: i64,
}

impl Workspace {
    /// 创建新的 Workspace，自动填充 id、created_at 和 updated_at。
    #[must_use]
    pub fn new(contest_id: String, problem_id: String, root_path: String) -> Self {
        let now = utc_now_secs();
        let id = format!("ws-{}-{}-{}", contest_id, problem_id, now);
        Self {
            id,
            contest_id,
            problem_id,
            root_path,
            files: std::collections::HashMap::new(),
            language: String::new(),
            is_dirty: false,
            created_at: now,
            updated_at: now,
        }
    }

    /// 更新 `updated_at` 为当前时间，标记工作区为活跃状态。
    pub fn touch(&mut self) {
        self.updated_at = utc_now_secs();
    }

    /// 标记工作区有未保存的修改。
    pub fn mark_dirty(&mut self) {
        self.is_dirty = true;
        self.touch();
    }

    /// 标记工作区已保存。
    pub fn mark_clean(&mut self) {
        self.is_dirty = false;
    }
}

#[cfg(test)]
#[path = "tests/workspace_tests.rs"]
mod tests;
