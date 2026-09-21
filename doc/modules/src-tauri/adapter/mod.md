# mod

## 职责
adapter 模块根文件：声明 HOJ、Hydro、QDUOJ、HUSTOJ 四个 OJ 适配器子模块（各自组合实现拆分的 Provider trait），并定义适配器层的公共形状 —— 构造依赖 `AdapterDeps`、工厂 trait `AdapterFactory` 与内建工厂清单 `factories()`。「有哪些 OJ」是数据：接入新 OJ = 新建一个子目录 + 在 `factories()` 加一行，不需要修改 core / infra / commands 的任何文件。

## 核心类型/函数
- `pub mod hoj / hydro / qduoj / hustoj` — OJ 适配器子模块声明（`hydro` 为完整实现，`qduoj` / `hustoj` 仍为骨架）
- **`AdapterDeps`** — 适配器构造依赖（`http_client: Arc<HttpClient>`, `event_bus: Arc<CoreEventBus>`, `session_repo: Arc<dyn SessionRepository>`, `storage: Arc<Storage>`）。**只准 infra 依赖**（硬约束：禁止把 Service 塞进 `AdapterDeps`，与「插件只能访问 `plugin/api`，禁止直接调用内部 Service」同理 —— 适配器一旦反向依赖应用层，依赖边界就彻底糊掉了）。各适配器按需消费：HOJ 用 http + event_bus + session_repo（凭证轮换当场落盘、落盘成功后发布 `CoreEvent::TokenRotated`），Hydro 只用 http_client。`session_repo` 是**仓库（infra 侧）**而不是 Service：Provider 侧凭证轮换需要的是「会话持久化能力」，而不是认证业务逻辑
- **`AdapterFactory`** — OJ 适配器工厂 trait（`Send + Sync`）：
  - `id() -> &'static str` — OJ 身份自声明（稳定契约：决定会话文件名 `sessions/{id}.json`，并与 `oj.instances[].id` 匹配）
  - `build(&self, deps: &AdapterDeps, base_url: &str) -> ProviderSet` — 按服务端地址构造该 OJ 的 Provider 能力集合。这个形状（id + 构造 + 配置）即 v1.0 插件 manifest 的雏形 —— 将来把编译期工厂清单换成运行时扫描插件目录，上层（registry / context / Service）不用再改；现在只做编译期清单，不做动态加载。**注意**：`OjInstance.options`（OJ 私有旋钮）目前没有传递通道，适配器拿不到（见 `doc/Hydro/适配新架构的冲突记录.md` §2.4）
- **`factories() -> Vec<&'static dyn AdapterFactory>`** — 内建 OJ 工厂清单（当前 `[&hoj::FACTORY, &hydro::FACTORY]`）。接入新 OJ：加 `pub mod xxx;` + 此处加一行 `&xxx::FACTORY`
- **`test_adapter_deps(storage: Arc<Storage>) -> AdapterDeps`**（`pub(crate)`，仅 `cfg(test)`）— 测试用 `AdapterDeps` 构造器。收敛在一处的原因：`AdapterDeps` 每加一个字段，全部适配器测试都要跟着改 —— 分散写会让「改了一处漏一处」变成编译错误之外的隐性成本（且四处默认值可能漂移）。临时目录守卫由调用方持有
- `#[cfg(test)] #[path = "tests/adapter_tests.rs"] mod tests` — 本层单元测试

## 直接依赖
- `core::event::core_event_bus::CoreEventBus`
- `core::provider::registry::ProviderSet`
- `core::repository::session_repo::SessionRepository`
- `infra::http::HttpClient`
- `infra::storage::Storage`
- `std::sync::Arc`

## 被依赖
- `lib.rs`（`pub mod adapter`）
- `core::context`（`AppContext::init` 组装 `AdapterDeps`，经 `factories()` 按配置实例注册各 OJ）

## 逻辑流程
无（模块声明 + 公共形状定义）。

## 测试
`src-tauri/src/adapter/tests/adapter_tests.rs` 锁定工厂清单的编译期防线（取代闭集枚举的穷尽检查）：id 唯一（重复会让注册表互相覆盖、会话文件名撞车）、全部工厂可构建且**至少提供一个能力**（依赖只来自 infra；刻意不断言四能力齐备 —— `ProviderSet` 的 Option 字段正是「先实现部分接口」的扩展路径，全 None 的 ProviderSet 才是注册 bug）、HOJ 工厂身份契约（`HOJAdapter::ID == "HOJ"` 且 `session_file() == "HOJ.json"`，与历史枚举 Debug 输出一致）、**HOJ 四能力齐备的专属锁定**（通用断言放宽后，HOJ 作为唯一全功能内建 OJ 由专属测试补回覆盖，漏装能力不再等到运行期 ProviderNotFound）。接入新 OJ 后通用断言自动覆盖新工厂。

Hydro 的对应断言放在自己的测试里（`adapter/hydro/tests/mod_tests.rs`）：身份契约（`HydroAdapter::ID == "Hydro"` → `sessions/Hydro.json`）、四能力齐备、`FACTORY.id()` 与适配器 ID 一致。通用断言 `factory_ids_unique_and_buildable` 自 Hydro 加入起自动覆盖它。
