use serde::{Deserialize, Serialize};
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};

/// 获取当前 UTC 时间戳（毫秒级），用于 Workspace 时间戳字段。
fn utc_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// 生成 4 位随机十六进制字符，用于 Workspace ID 防碰撞。
/// 使用 std 内置 hasher 的随机状态，避免引入额外依赖。
fn random_hex_suffix() -> String {
    let hasher = RandomState::new().build_hasher();
    format!("{:04x}", hasher.finish() as u16)
}

/// Workspace — 核心领域对象。
///
/// 负责代码存储、自动保存、崩溃恢复、比赛隔离、模板管理、缓存。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub contest_id: String,
    pub problem_id: String,
    pub root_path: String,
    /// 各文件名 → 文件内容
    pub files: std::collections::HashMap<String, String>,
    pub language: String,
    pub is_dirty: bool,
    /// 创建时间（UTC 毫秒级时间戳）
    pub created_at: i64,
    /// 最后修改时间（UTC 毫秒级时间戳），崩溃恢复时用于判断最近活跃工作区
    pub updated_at: i64,
}

impl Workspace {
    /// 创建新的 Workspace，自动填充 id（含随机后缀防碰撞）、created_at 和 updated_at。
    #[must_use]
    pub fn new(contest_id: String, problem_id: String, root_path: String) -> Self {
        let now = utc_now_ms();
        let id = format!("ws-{}-{}-{}-{}", contest_id, problem_id, now, random_hex_suffix());
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
        self.updated_at = utc_now_ms();
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
