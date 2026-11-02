# mod

## 职责
配置服务模块入口。负责用户配置、OJ 配置、编辑器偏好、主题、布局的统一管理。`ConfigService` 以 JSON 格式持久化配置，支持运行时读取、更新和热重载。泛型 `R` 为底层持久化实现（当前为 `FsConfigRepository`），可按需替换存储后端。

## 核心类型/函数
- `pub mod error` — 配置错误类型模块声明
- **`ConfigService<R: ConfigRepository>`** — 统一配置管理服务
  - `fn new(repo, event_bus, access) -> Self` — 创建实例并立即加载配置；加载成功后先经 `normalize_legacy_values` 归一旧默认值（P55，见下）；首次启动时配置文件不存在则自动使用默认值并持久化
  - `fn get(&self) -> AppConfig` — 获取当前配置的不可变副本
  - `fn update<F>(&self, updater: F) -> AppResult<AppConfig>` — 通过闭包修改配置后自动保存到磁盘（持久化失败时保留内存修改，返回错误供调用方回滚，且**不发布事件**），成功后发布 `CoreEvent::ConfigChanged`。**为什么在这里发布**：`update` 才是生产链路的配置变更入口（`update_config`、主题切换、`switch_oj` 的持久化都走它）；只有 `reload` 发布的话，观察者永远等不到通知 —— `reload_config` 命令在前端没有调用方（2026-09-21 复核）
  - `fn update_checked<F>(&self, updater) -> AppResult<AppConfig>`（`pub(crate)`）— 可失败的写事务：先取写事务锁，在**候选副本**上执行 updater；返回 `Err` 时不改内存、不落盘、不发事件（评审三十三 #2）。`update_config` 用它把「读锁内最新 active → 合并/校验 → 整体替换」做成一个原子事务
  - `fn update_locked<F>(&self, updater)`（`pub(crate)`）— 写事务本体，**调用方必须已持有** [`lock_write_txn`]；`switch_oj` 在同一把锁内完成 Registry 切换 + `oj.active` 落盘
  - `fn lock_write_txn(&self) -> MutexGuard<'_, ()>`（`pub(crate)`）— 写事务锁的唯一获取点；锁序固定「写事务锁 → config RwLock」，持有期间不得再调 `update`/`update_checked`（Mutex 不可重入）
  - `fn reload(&self) -> AppResult<AppConfig>` — 强制从磁盘重新加载，覆盖内存副本；**与 `new()` 同一归一入口**：手改磁盘文件后 reload 同样要修正旧值；加载成功后发布 `CoreEvent::ConfigChanged`
  - `fn reload_locked(&self)`（`pub(crate)`）— 重载本体，调用方必须已持有写事务锁；`reload_config` 在同一事务内继续把 Registry 同步到磁盘 `oj.active`（评审三十四）
  - `fn write_txn_is_held(&self) -> bool`（`#[cfg(test)]`）— `try_lock` 探测写事务是否被另一个线程持有，供并发回归在取锁前做确定性握手，不依赖 sleep
  - `fn repo(&self) -> &Arc<R>` — 返回底层 ConfigRepository 引用
- **字段**：`repo: Arc<R>`, `event_bus: Arc<CoreEventBus>`, `config: RwLock<AppConfig>`, `write_txn: Mutex<()>`

## 直接依赖
- `core::entity::config::{normalize_legacy_values, AppConfig}`
- `core::error::{AppError, AppResult}`
- `core::event::core_event::CoreEvent`
- `core::event::core_event_bus::CoreEventBus`
- `core::repository::config_repo::ConfigRepository`

## 被依赖
- `core::context`（`AppContext` 持有 `Arc<ConfigService<FsConfigRepository>>`）
- `service::theme::ThemeService`（通过 `ConfigService` 读写主题配置）

## 逻辑流程
- **初始化**：`new()` 从 Repo 加载配置，成功则调用 `normalize_legacy_values(&mut cfg)` 一次性归一历史旧默认值（`"C++"`→`"cpp"`、整机 `dark`→`light`（含随之写入的 `vs-dark`→`vs`）、`splitRatio 0.45`→`0.48`，只修正恰好等于旧默认值的项，详见 `core/entity/config.md`）；失败时回退到 `AppConfig::default()` 并尝试写回磁盘
- **读取**：`get()` 从 `RwLock<AppConfig>` 中 clone 返回，无副作用
- **更新**：`update(f)` 先获取写事务锁 → 在内存副本上执行闭包 → 提交内存 → 持久化磁盘 → 发布事件 → 返回新配置；磁盘写入失败时保留内存修改、上报错误。**整个更新是一个写事务**（独立 `write_txn` 互斥锁覆盖「updater → save → 发布事件」，不扩大 `RwLock` 的持锁区间）：克隆出来的旧配置不可能后于新配置落盘，内存/磁盘/Registry 的 `oj.active` 保持一致（评审三十一 P2；`reload` 与 `switch_oj` 共用该锁，评审三十三 #1）。`update_checked` 的 updater 返回 `Err` 时内存保持原样（候选在副本上修改），校验失败的保存不会留下半改状态（评审三十三 #2）
- **热重载**：`reload()` 从磁盘读最新配置 → 同样经 `normalize_legacy_values` 归一（用户手改磁盘文件塞回旧值也会被修正）→ 覆盖内存 → 发布 `CoreEvent::ConfigChanged`
- **`update` 与 `reload` 都要发布 `ConfigChanged`**：生产链路的配置变更入口是 `update`（`update_config` 命令、主题切换、`switch_oj` 的持久化），而 `reload_config` 命令在前端**没有调用方** —— 只在 `reload` 里发布会让观察者（审计、插件）永远等不到通知。两条路径的语义统一为「配置已变更即通知」；**持久化失败时不发布**（磁盘与内存已不一致，让观察者按「新配置已生效」行动会掩盖问题）。两处均有用例锁定
- **`ConfigChanged` 只是完成事实通知**：需要「配置变更后必须完成的动作」（如按新间隔同步 auto-save）由命令层在 `update` 返回后**显式调用** `commands::workspace_cmd::sync_auto_save_from_context` —— 不得依赖事件消费者（消费者可能落后、可能不存在）。旧实现把这件事挂在 `ConfigReloaded` 的订阅者上，订阅者内部还要 `tauri::async_runtime::spawn` 才拿得到 tokio 上下文
- **归一只在加载路径**：`update()` 不做归一 —— 前端写回的配置本就来自已归一的内存副本，重复归一反而可能吞掉用户显式设置的合法值

## 有效连接变更

构造函数必传共享AccessRevision，update持有配置锁时比较OJ配置，发生变化立即撤销旧scope；即使后续持久化失败也生效。普通editor修改不撤销；reload保守撤销。配置事件仍只在持久化成功后发布。

访问栅栏为构造必填参数，无私有默认栅栏或with_access后置绑定。组合根创建一次共享Arc<AccessRevision>，测试夹具也显式指定。

update在写锁内执行闭包再比较OJ字段并撤销；新配置在解锁前不可见，旧缓存读在撤销前只能看到原scope。无需在无关编辑器配置更新时提前撤销；即使保存失败，配置解锁前也已建立新的访问代次。
