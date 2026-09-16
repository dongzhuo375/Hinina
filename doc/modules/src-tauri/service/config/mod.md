# mod

## 职责
配置服务模块入口。负责用户配置、OJ 配置、编辑器偏好、主题、布局的统一管理。`ConfigService` 以 JSON 格式持久化配置，支持运行时读取、更新和热重载。泛型 `R` 为底层持久化实现（当前为 `FsConfigRepository`），可按需替换存储后端。

## 核心类型/函数
- `pub mod error` — 配置错误类型模块声明
- **`ConfigService<R: ConfigRepository>`** — 统一配置管理服务
  - `fn new(repo, event_bus) -> Self` — 创建实例并立即加载配置；加载成功后先经 `normalize_legacy_values` 归一旧默认值（P55，见下）；首次启动时配置文件不存在则自动使用默认值并持久化
  - `fn get(&self) -> AppConfig` — 获取当前配置的不可变副本
  - `fn update<F>(&self, updater: F) -> AppResult<AppConfig>` — 通过闭包修改配置后自动保存到磁盘（失败时保留内存修改，返回错误供调用方回滚）
  - `fn reload(&self) -> AppResult<AppConfig>` — 强制从磁盘重新加载，覆盖内存副本；**与 `new()` 同一归一入口**：手改磁盘文件后 reload 同样要修正旧值；加载成功后发布 `SystemEvent::ConfigReloaded`
  - `fn repo(&self) -> &Arc<R>` — 返回底层 ConfigRepository 引用
- **字段**：`repo: Arc<R>`, `event_bus: Arc<EventBus>`, `config: RwLock<AppConfig>`

## 直接依赖
- `core::entity::config::{normalize_legacy_values, AppConfig}`
- `core::error::{AppError, AppResult}`
- `core::event::app_event::{AppEvent, SystemEvent}`
- `core::event::event_bus::EventBus`
- `core::repository::config_repo::ConfigRepository`

## 被依赖
- `core::context`（`AppContext` 持有 `Arc<ConfigService<FsConfigRepository>>`）
- `service::theme::ThemeService`（通过 `ConfigService` 读写主题配置）

## 逻辑流程
- **初始化**：`new()` 从 Repo 加载配置，成功则调用 `normalize_legacy_values(&mut cfg)` 一次性归一历史旧默认值（`"C++"`→`"cpp"`、`dark`/`vs-dark`→`light`/`vs`、`splitRatio 0.45`→`0.48`，只修正恰好等于旧默认值的项，详见 `core/entity/config.md`）；失败时回退到 `AppConfig::default()` 并尝试写回磁盘
- **读取**：`get()` 从 `RwLock<AppConfig>` 中 clone 返回，无副作用
- **更新**：`update(f)` 先通过闭包修改内存 → 持久化磁盘 → 返回新配置；磁盘写入失败时保留内存修改、上报错误
- **热重载**：`reload()` 从磁盘读最新配置 → 同样经 `normalize_legacy_values` 归一（用户手改磁盘文件塞回旧值也会被修正）→ 覆盖内存 → 发布 `SystemEvent::ConfigReloaded` 通知其他 Service 响应变更
- **归一只在加载路径**：`update()` 不做归一 —— 前端写回的配置本就来自已归一的内存副本，重复归一反而可能吞掉用户显式设置的合法值
