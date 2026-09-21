# session_repo

## 职责
会话持久化仓库 trait `SessionRepository` —— 抽象「本地会话记录的读写」，让**应用层（`AuthService`）与适配器层（Provider 凭证轮换）共用同一份持久化契约**，而不是让适配器去发布带真实 token 的事件、由应用层的订阅者代劳落盘。

## 核心类型/函数
- **`trait SessionRepository: Send + Sync`**
  - `load(&self, oj_id: &OjId) -> AppResult<Option<Session>>` — 读取指定 OJ 的会话；不存在返回 `Ok(None)`。**文件损坏（JSON 解析失败）如实报错**，不静默当成「无会话」—— 后者会让选手以为从未登录，而真正的问题（文件被手改 / 写坏）被掩盖
  - `save(&self, session: &Session) -> AppResult<()>` — 写入会话（目录不存在时自动创建）
  - `remove(&self, oj_id: &OjId) -> AppResult<()>` — 删除会话（不存在时静默成功，幂等）
  - `rotate_token(&self, oj_id: &OjId, token: &str) -> AppResult<bool>` — 原子地把指定 OJ 会话的 token 换成新值，返回是否确实更新（会话不存在时 `Ok(false)`，例如轮换发生在登录持久化之前的极端时序）

## 直接依赖
- `core::entity::session::Session`
- `core::error::AppResult`
- `core::provider::oj_id::OjId`（会话按 OJ 隔离，`OjId::session_file()` 决定文件名）

## 被依赖
- `infra::fs_session_repo`（唯一生产实现）
- `core::context`（`AppContext::session_repo: Arc<dyn SessionRepository>`，初始化序列第 4 步）
- `service::auth`（登录落盘 / 登出删除 / 会话失效清理）
- `adapter::mod`（`AdapterDeps.session_repo`）与 `adapter::hoj`（凭证轮换落盘）

## 逻辑流程
无（trait 定义）。调用方各自按语义调用；实现见 `infra/fs_session_repo.md`。

## 设计约束
- **落盘是持久性保证，必须显式完成**：它决定重启后 `get_session` 回注的是新 token 还是过期凭证，属于「必须等待结果的核心动作」，不能依赖异步事件消费者（消费者可能落后、可能不存在、失败也不该影响已完成的上游请求）。
- **`rotate_token` 独立成方法而非「`load` + 改 + `save`」三连**：读-改-写必须是一个原子单元，否则并发轮换或与登出删除交错时会丢更新（写回旧值 / 复活已删除的会话）。
- **为什么它进 `AdapterDeps` 而不是 Service**：Provider 需要的是「会话持久化能力」这一 infra 侧能力，不是认证业务逻辑 —— 与「插件只能访问 `plugin/api`，禁止直接调用内部 Service」同理，适配器一旦反向依赖应用层，依赖边界就糊掉了。
