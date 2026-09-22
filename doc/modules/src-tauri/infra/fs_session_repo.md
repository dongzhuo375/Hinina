# fs_session_repo

## 职责
`SessionRepository` 的文件系统实现：会话以 JSON 存在 `sessions/{oj_id}.json`（路径相对 `storage.base_dir()`）。所有**变更**（`save` / `remove` / `rotate_token`）共用一把 `mutation_lock` 串行化，写入走 `Storage::write_string_atomic`（tmp + fsync + rename）。

## 核心类型/函数
- **`SESSIONS_DIR: &str = "sessions"`** — 会话持久化目录（相对 `storage.base_dir()`）
- **`FsSessionRepository`** — 字段：`storage: Arc<Storage>`、`mutation_lock: Mutex<()>`
  - `new(storage: Arc<Storage>) -> Self`（`#[must_use]`）— 创建仓库，**不触碰磁盘**（目录在首次写入时创建）
  - `path(&self, oj_id: &OjId) -> String`（私有）— 会话文件相对路径 `sessions/{oj_id.session_file()}`
- **`impl SessionRepository`**
  - `load` — **不取锁**（只读）；文件不存在 → `Ok(None)`；读取失败 `warn` 后原样上抛；JSON 解析失败 `warn` 后返回 `AppError::Serialization("会话文件解析失败: …")`；成功记 `debug` 并返回 `Some(session)`
  - `save` — 取 `mutation_lock` → `save_locked`
  - `remove` — 取 `mutation_lock` → `remove_locked`
  - `rotate_token` — 取 `mutation_lock` 后**读-改-写整体持锁**：`load` → 会话不存在返回 `Ok(false)` → 改 `token` → `save_locked` → `Ok(true)`
- **`save_locked`**（私有，调用方须已持锁）— `create_dir(SESSIONS_DIR)`（目录可能被用户清空过，或首次运行）→ `serde_json::to_string_pretty` → `storage.write_string_atomic`（失败 `warn` 后上抛）
- **`remove_locked`**（私有，调用方须已持锁）— 文件不存在直接 `Ok(())`（幂等）；否则 `storage.remove`（失败 `warn` 后上抛）

## 直接依赖
- `std::sync::{Arc, Mutex}`
- `tracing::{debug, warn}`
- `core::entity::session::Session`
- `core::error::{AppError, AppResult}`（仅本地文件解析错误）
- `core::provider::oj_id::OjId`
- `core::repository::session_repo::SessionRepository`
- `infra::storage::Storage`（**`write_string_atomic`**）

## 被依赖
- `core::context`（`AppContext::init` 构造并注入 `Arc<dyn SessionRepository>`；`AuthService` 与 `AdapterDeps` 拿到的是**同一个实例** —— 锁共享是修复生效的前提）
- `adapter::mod`（`#[cfg(test)] test_adapter_deps` 构造测试用 `AdapterDeps`）

## 逻辑流程
```
load(oj_id)   → exists? ──否──► Ok(None)
                └─是─► read_to_string ─► serde_json::from_str::<Session>
                          ├─ Err ─► warn + AppError::Serialization
                          └─ Ok  ─► Ok(Some(session))

save(session) → lock(mutation_lock) → save_locked
                                      → create_dir("sessions")
                                      → to_string_pretty
                                      → write_string_atomic（tmp + sync_all + rename）

remove(oj_id) → lock(mutation_lock) → remove_locked（不存在即 Ok(())）

rotate_token(oj_id, token)
  → lock(mutation_lock)（中毒则 into_inner 继续）
  → load(oj_id) ── None ──► Ok(false)（不复活已删除的会话）
  → session.token = token → save_locked → Ok(true)
```

## 设计约束
- **为什么三个变更共用一把锁**：只锁 `rotate_token` 时，「轮换的读-改-写」仍可能与「登出的删除」交错 —— 轮换读到旧会话 → 登出删除文件 → 轮换把旧会话写回，**已删除的会话被复活**，下次启动 `get_session` 回注过期登录态（共享机房上的真实风险）；与「登录的 save」交错同理（旧值覆盖新登录）。共用一把锁才能兑现 `SessionRepository::rotate_token` 文档承诺的原子性。
- **`load` 刻意不取锁**：只读不需要串行化；若 `load` 也取锁，`rotate_token` 内部调用它会自死锁（`std::sync::Mutex` 不可重入）—— 内部调用一律走 `*_locked` 变体。
- **锁中毒时不放弃**：保护的只是变更的串行化，panic 不会让数据不一致，而「放弃保存/轮换」会让登录态凭空丢失或磁盘 token 永久过期。
- **原子写而非就地写**：`fs::write` 是「截断 + 就地写」，进程在写入中途崩溃会留下半截文件，下次启动只能当作损坏处理 —— 对「重启后必须可恢复」的凭据不可接受。`write_string_atomic` 的崩溃/掉电语义见 `infra::storage`。
- **解析失败不降级为「无会话」**：由仓库层如实报错、应用层（`AuthService::get_session`）降级为「无会话」并 `warn` 留痕 —— 静默吞掉会让「会话文件损坏」表现为「从未登录」，无从排查。

## 测试
`tests/fs_session_repo_tests.rs`（每例独立临时目录）：保存-读取 roundtrip、读不存在的会话返回 `None`、**会话按 OJ 隔离**（不同 OJ 互不覆盖）、`remove` 幂等、`rotate_token` 只改 token（其余字段不动）、**会话不存在时 `rotate_token` 不复活它**（返回 `false` 且不产生文件）、损坏文件如实报错（不返回 `None`）、**并发轮换后文件仍是合法且 token 一致**、**轮换与删除并发交错后不得复活已删除的会话**（锁定 `mutation_lock` 的变更串行化 —— 去掉共用锁后该用例可复现复活）。
