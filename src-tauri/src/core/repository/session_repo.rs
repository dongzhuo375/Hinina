use crate::core::entity::session::Session;
use crate::core::error::AppResult;
use crate::core::provider::oj_id::OjId;

/// 会话持久化仓库。
///
/// 抽象「本地会话记录的读写」，让**应用层（`AuthService`）与适配器层（Provider
/// 凭证轮换）共用同一份持久化契约**，而不是让适配器去发布带真实 token 的事件、
/// 由应用层的订阅者代劳落盘。
///
/// # 为什么必须显式落盘
///
/// 会话落盘是**持久性保证**：它决定重启后 `get_session` 回注的是新 token 还是
/// 过期凭证。因此它属于「必须等待结果的核心动作」，不能依赖异步事件消费者
/// （消费者可能落后、可能不存在、失败也不该影响已完成的上游请求）。
pub trait SessionRepository: Send + Sync {
    /// 读取指定 OJ 的会话；不存在返回 `Ok(None)`。
    ///
    /// 文件损坏（JSON 解析失败）**如实报错**，不静默当成「无会话」——
    /// 后者会让选手以为从未登录，而真正的问题（文件被手改 / 写坏）被掩盖。
    fn load(&self, oj_id: &OjId) -> AppResult<Option<Session>>;

    /// 写入会话（目录不存在时自动创建）。
    fn save(&self, session: &Session) -> AppResult<()>;

    /// 删除会话（不存在时静默成功）。
    fn remove(&self, oj_id: &OjId) -> AppResult<()>;

    /// 原子地把指定 OJ 会话的 token 换成新值。
    ///
    /// 返回是否确实更新（会话不存在时 `Ok(false)`，例如轮换发生在登录持久化
    /// 之前的极端时序）。
    ///
    /// 独立成方法而非「`load` + 改 + `save`」三连：读-改-写必须是一个原子单元，
    /// 否则并发轮换或与登出删除交错时会丢更新（写回旧值 / 复活已删除的会话）。
    fn rotate_token(&self, oj_id: &OjId, token: &str) -> AppResult<bool>;
}
