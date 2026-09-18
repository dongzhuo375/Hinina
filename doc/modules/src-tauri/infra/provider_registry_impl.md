# provider_registry_impl

## 职责
`ProviderRegistry` trait 的默认实现，以单表 `HashMap<OjId, ProviderSet>`（取代旧的 4 个能力分表）管理所有 OJ 的 Provider 能力集合，并维护当前选中的 OJ。注册哪些 OJ 是运行期数据，接入新 OJ 不需要修改本文件。

## 核心类型/函数
- **`ProviderRegistryImpl`** — Provider 注册中心实现 struct，含 `providers: RwLock<HashMap<OjId, ProviderSet>>` 与 `current: RwLock<OjId>`
- **`ProviderRegistryImpl::new(default_oj: OjId)`** — 构造函数，初始化空 HashMap 并设置默认当前 OJ
- **`capability<T, F>(&self, pick: F) -> AppResult<Arc<T>>`**（私有帮手）— 按能力取当前 OJ 的 Provider，四个 `current_xxx` 共用：取 `current_id()` → 读锁查单表（未注册 → `ProviderNotFound`「OJ {} 未注册」）→ `pick(set)` 克隆（该能力为 `None` → `ProviderNotFound`「OJ {} 未提供该能力」）
- **`register(oj_id, set)`** — 获取写锁，将 `(OjId, ProviderSet)` 插入单表（同 id 二次注册为覆盖）
- **`current_auth / current_contest / current_problem / current_submission`** — 分别转调 `capability(|set| &set.xxx)`
- **`current_id()` / `set_current(oj_id)`** — 读写当前 OJ；`current_id` 锁中毒时 `into_inner` 取回内部数据（注册表进程级单例，中毒即全局异常，不静默回退）
- **`list_available()`** — 以 providers 单表 key 集合为准（注册过 `ProviderSet` 即视为可用 OJ）

## 直接依赖
- `std::collections::HashMap`
- `std::sync::{Arc, RwLock}`
- `core::error::{AppError, AppResult}`
- `core::provider::oj_id::OjId`
- `core::provider::registry::{ProviderRegistry, ProviderSet}`

## 被依赖
- `core::context`（`AppContext::init` 创建并通过 `ProviderRegistry` trait 注入）

## 逻辑流程
1. **初始化**：`new(default_oj)` 创建空 HashMap 并设置 current OjId
2. **注册**：`register` 获取写锁，将 `(OjId, ProviderSet)` 插入单表
3. **按能力查询**：`current_xxx` 经私有 `capability` 取当前 OJ 的对应能力；「未注册」与「已注册但缺该能力」均返回 `AppError::ProviderNotFound`
4. **切换 OJ**：`set_current` 获取写锁更新 current；`current_id` 获取读锁返回当前值（中毒时 `into_inner`）
5. **列出可用 OJ**：`list_available` 以单表 key 集合为准

## 测试
`src-tauri/src/infra/tests/provider_registry_impl_tests.rs`（由 `provider_registry_impl.rs` 底部 `#[cfg(test)] #[path = "tests/provider_registry_impl_tests.rs"] mod tests;` 引用）锁定查询侧契约：未注册时四种能力均 `ProviderNotFound` 且列表为空、部分实现（`ProviderSet::default`）是合法注册但缺能力报 `ProviderNotFound`、切换到未注册 OJ 同样 `ProviderNotFound`、同 id 二次注册覆盖与 `list_available` 反映注册、`current_id` / `set_current` 往返与 `OjId` 身份规整（trim）。
