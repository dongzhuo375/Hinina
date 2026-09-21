use std::sync::{Arc, Mutex};

use tracing::{debug, warn};

use crate::core::entity::session::Session;
use crate::core::error::{AppError, AppResult};
use crate::core::provider::oj_id::OjId;
use crate::core::repository::session_repo::SessionRepository;
use crate::infra::storage::Storage;

/// 会话持久化目录（相对 `storage.base_dir()`）。
pub const SESSIONS_DIR: &str = "sessions";

/// [`SessionRepository`] 的文件系统实现：`sessions/{oj_id}.json`。
pub struct FsSessionRepository {
    storage: Arc<Storage>,
    /// 串行化 [`SessionRepository::rotate_token`] 的「读 → 改 → 写」。
    ///
    /// 不加锁时两个并发轮换（或多个接口同时收到轮换响应头）会各自读到旧值、
    /// 各自写回，后写者胜出 —— 丢掉的正是最新的 token，而调用方都以为成功了。
    rotate_lock: Mutex<()>,
}

impl FsSessionRepository {
    /// 创建仓库（不触碰磁盘，目录在首次写入时创建）。
    #[must_use]
    pub fn new(storage: Arc<Storage>) -> Self {
        Self {
            storage,
            rotate_lock: Mutex::new(()),
        }
    }

    /// 会话文件相对路径。
    fn path(&self, oj_id: &OjId) -> String {
        format!("{}/{}", SESSIONS_DIR, oj_id.session_file())
    }
}

impl SessionRepository for FsSessionRepository {
    fn load(&self, oj_id: &OjId) -> AppResult<Option<Session>> {
        let path = self.path(oj_id);
        if !self.storage.exists(&path) {
            return Ok(None);
        }
        let raw = self.storage.read_to_string(&path).map_err(|e| {
            warn!(path = %path, error = %e, "会话文件读取失败");
            e
        })?;
        let session = serde_json::from_str::<Session>(&raw).map_err(|e| {
            warn!(path = %path, error = %e, "会话文件 JSON 解析失败");
            AppError::Serialization(format!("会话文件解析失败: {}", e))
        })?;
        debug!(path = %path, "会话已加载");
        Ok(Some(session))
    }

    fn save(&self, session: &Session) -> AppResult<()> {
        // 目录可能被用户清空过（或首次运行），写入前确保存在
        self.storage.create_dir(SESSIONS_DIR)?;
        let path = self.path(&OjId::new(&session.oj_id));
        let json = serde_json::to_string_pretty(session)?;
        self.storage.write_string(&path, &json).map_err(|e| {
            warn!(path = %path, error = %e, "会话文件写入失败");
            e
        })?;
        debug!(path = %path, "会话已保存");
        Ok(())
    }

    fn remove(&self, oj_id: &OjId) -> AppResult<()> {
        let path = self.path(oj_id);
        if !self.storage.exists(&path) {
            return Ok(());
        }
        self.storage.remove(&path).map_err(|e| {
            warn!(path = %path, error = %e, "会话文件删除失败");
            e
        })?;
        debug!(path = %path, "会话文件已删除");
        Ok(())
    }

    fn rotate_token(&self, oj_id: &OjId, token: &str) -> AppResult<bool> {
        // 锁中毒（持有者 panic）时取回内部值继续：这里保护的只是「读改写」的
        // 原子性，panic 不会让数据不一致，而「放弃轮换」会让磁盘 token 永久过期。
        let _guard = self.rotate_lock.lock().unwrap_or_else(|e| e.into_inner());

        let Some(mut session) = self.load(oj_id)? else {
            return Ok(false);
        };
        session.token = token.to_string();
        self.save(&session)?;
        debug!(oj_id = %oj_id, "会话 token 已轮换并落盘");
        Ok(true)
    }
}

#[cfg(test)]
#[path = "tests/fs_session_repo_tests.rs"]
mod tests;
