# fs_session_repo

## 职责
`SessionRepository` 的文件系统实现：会话以 JSON 存在 `sessions/{oj_id}.json`（路径相对 `storage.base_dir()`）。

## 核心类型/函数
- **`SESSIONS_DIR: &str = "sessions"`** — 会话持久化目录（相对 `storage.base_dir()`）
- **`FsSessionRepository`** — 字段：`storage: Arc<Storage>`、`rotate_lock: Mutex<()>`
  - `new(storage: Arc<Storage>) -> Self`（`#[must_use]`）— 创建仓库，**不触碰磁盘**（目录在首次写入时创建）
  - `path(&self, oj_id: &OjId) -> String`（私有）— 会话文件相对路径 `sessions/{oj_id.session_file()}`
- **`impl SessionRepository`**
  - `load` — 文件不存在 → `Ok(None)`；读取失败 `warn` 后原样上抛；JSON 解析失败 `warn` 后返回 `AppError::Serialization("会话文件解析失败: …")`；成功记 `debug` 并返回 `Some(session)`
  - `save` — 先 `create_dir(SESSIONS_DIR)`（目录可能被用户清空过，或首次运行）→ `serde_json::to_string_pretty` → `write_string`（失败 `warn` 后上抛）
  - `remove` — 文件不存在直接 `Ok(())`（幂等）；否则 `storage.remove`（失败 `warn` 后上抛）
  - `rotate_token` — 取 `rotate_lock`（**锁中毒时 `unwrap_or_else(into_inner)` 取回内部值继续**）→ `load` → 会话不存在返回 `Ok(false)` → 改 `token` → `save` → `Ok(true)`

## 直接依赖
- `std::sync::{Arc, Mutex}`
- `tracing::{debug, warn}`
- `core::entity::session::Session`
- `core::error::{AppError, AppResult}`（仅本地文件解析错误）
- `core::provider::oj_id::OjId`
- `core::repository::session_repo::SessionRepository`
- `infra::storage::Storage`

## 被依赖
- `core::context`（`AppContext::init` 构造并注入 `Arc<dyn SessionRepository>`）
- `adapter::mod`（`#[cfg(test)] test_adapter_deps` 构造测试用 `AdapterDeps`）

## 逻辑流程
```
load(oj_id)   → exists? ──否──► Ok(None)
                └─是─► read_to_string ─► serde_json::from_str::<Session>
                          ├─ Err ─► warn + AppError::Serialization
                          └─ Ok  ─► Ok(Some(session))

save(session) → create_dir("sessions") → to_string_pretty → write_string

rotate_token(oj_id, token)
  → lock(rotate_lock)（中毒则 into_inner 继续）
  → load(oj_id) ── None ──► Ok(false)（不复活已删除的会话）
  → session.token = token → save → Ok(true)
```

## 设计约束
- **`rotate_lock` 存在的理由**：不加锁时两个并发轮换（或多个接口同时收到轮换响应头）会各自读到旧值、各自写回，后写者胜出 —— 丢掉的正是最新的 token，而调用方都以为成功了。锁中毒时**不放弃轮换**：这里保护的只是「读改写」的原子性，panic 不会让数据不一致，而「放弃轮换」会让磁盘 token 永久过期。
- **解析失败不降级为「无会话」**：由仓库层如实报错、应用层（`AuthService::get_session`）降级为「无会话」并 `warn` 留痕 —— 静默吞掉会让「会话文件损坏」表现为「从未登录」，无从排查。

## 测试
`tests/fs_session_repo_tests.rs`（每例独立临时目录）：保存-读取 roundtrip、读不存在的会话返回 `None`、**会话按 OJ 隔离**（不同 OJ 互不覆盖）、`remove` 幂等、`rotate_token` 只改 token（其余字段不动）、**会话不存在时 `rotate_token` 不复活它**（返回 `false` 且不产生文件）、损坏文件如实报错（不返回 `None`）、**并发轮换后文件仍是合法且 token 一致**（锁定 `rotate_lock` 的读改写原子性）。
