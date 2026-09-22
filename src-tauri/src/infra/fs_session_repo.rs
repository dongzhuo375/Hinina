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
    /// 串行化**所有**会话变更（`save` / `remove` / `rotate_token`）。
    ///
    /// 只锁 `rotate_token` 时，「轮换的读-改-写」仍可能与「登出的删除」交错：
    /// 轮换读到旧会话 → 登出删除文件 → 轮换把旧会话写回 —— 已删除的会话被复活，
    /// 下次启动 `get_session` 会回注过期登录态（共享机房上的真实风险）。
    /// 与「登录的 save」交错同理（旧值覆盖新登录）。所有变更共用一把锁，
    /// 才能兑现 `SessionRepository::rotate_token` 文档承诺的原子性。
    mutation_lock: Mutex<()>,
}

impl FsSessionRepository {
    /// 创建仓库（不触碰磁盘，目录在首次写入时创建）。
    #[must_use]
    pub fn new(storage: Arc<Storage>) -> Self {
        Self {
            storage,
            mutation_lock: Mutex::new(()),
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
        // 锁中毒（持有者 panic）时取回内部值继续：这里保护的只是变更的
        // 串行化，panic 不会让数据不一致，而「放弃保存」会让登录态凭空丢失。
        let _guard = self.mutation_lock.lock().unwrap_or_else(|e| e.into_inner());
        self.save_locked(session)
    }

    fn remove(&self, oj_id: &OjId) -> AppResult<()> {
        let _guard = self.mutation_lock.lock().unwrap_or_else(|e| e.into_inner());
        self.remove_locked(oj_id)
    }

    fn rotate_token(&self, oj_id: &OjId, token: &str) -> AppResult<bool> {
        // 读-改-写整体持锁：与 save / remove 互斥，兑现 trait 文档的原子性承诺
        let _guard = self.mutation_lock.lock().unwrap_or_else(|e| e.into_inner());

        let Some(mut session) = self.load(oj_id)? else {
            return Ok(false);
        };
        session.token = token.to_string();
        self.save_locked(&session)?;
        debug!(oj_id = %oj_id, "会话 token 已轮换并落盘");
        Ok(true)
    }
}

impl FsSessionRepository {
    /// 写入会话（调用方须已持有 `mutation_lock`）。
    ///
    /// 原子写（tmp + rename）：`fs::write` 是「截断 + 就地写」，进程在写入中途
    /// 崩溃会留下半截文件，下次启动只能当作损坏处理 —— 对「重启后必须可恢复」
    /// 的凭据不可接受。
    fn save_locked(&self, session: &Session) -> AppResult<()> {
        // 目录可能被用户清空过（或首次运行），写入前确保存在
        self.storage.create_dir(SESSIONS_DIR)?;
        let path = self.path(&OjId::new(&session.oj_id));
        let json = serde_json::to_string_pretty(session)?;
        self.storage.write_string_atomic(&path, &json).map_err(|e| {
            warn!(path = %path, error = %e, "会话文件写入失败");
            e
        })?;
        debug!(path = %path, "会话已保存");
        Ok(())
    }

    /// 删除会话（调用方须已持有 `mutation_lock`）。
    fn remove_locked(&self, oj_id: &OjId) -> AppResult<()> {
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
}

#[cfg(test)]
#[path = "tests/fs_session_repo_tests.rs"]
mod tests;
