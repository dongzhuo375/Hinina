# provider_registry_impl

## 职责
`ProviderRegistry` trait 的默认实现，通过 `HashMap<RwLock>` 按 `OJType` 管理四类 Provider（Auth、Contest、Problem、Submission）的注册与查找，并维护当前选中的 OJ。

## 核心类型/函数
- **`ProviderRegistryImpl`** — Provider 注册中心实现 struct，含 4 个 `RwLock<HashMap<OJType, Arc<dyn Provider>>>` 和当前 OJType
- **`ProviderRegistryImpl::new(default_oj: OJType)`** — 构造函数，初始化空 HashMap 并设置默认 OJ
- **`register_auth / register_contest / register_problem / register_submission`** — 注册各类 Provider
- **`get_auth / get_contest / get_problem / get_submission`** — 按 OJType 查找 Provider，不存在返回 `AppError::ProviderNotFound`
- **`current_oj()` / `set_current_oj(oj_type)`** — 读写当前激活的 OJType
- **`list_available()`** — 列出所有已注册 AuthProvider 的 OJType

## 直接依赖
- `std::collections::HashMap`
- `std::sync::{Arc, RwLock}`
- `core::error::{AppError, AppResult}`
- `core::provider::auth::AuthProvider`
- `core::provider::contest::ContestProvider`
- `core::provider::oj_type::OJType`
- `core::provider::problem::ProblemProvider`
- `core::provider::registry::ProviderRegistry`
- `core::provider::submission::SubmissionProvider`

## 被依赖
暂无（未被 infra 外部模块直接引用，预期由 `core::context` 在初始化时创建并通过 `ProviderRegistry` trait 注入）

## 逻辑流程
1. **初始化**：`new(default_oj)` 创建 4 个空 HashMap 并设置 current OJType
2. **注册**：`register_*` 方法获取写锁，将 `(OJType, Arc<dyn Provider>)` 插入对应 HashMap
3. **查找**：`get_*` 方法获取读锁，按 OJType 查找 HashMap，命中返回克隆的 Arc，未命中返回 `AppError::ProviderNotFound`
4. **切换 OJ**：`set_current_oj` 获取写锁更新 current OJType，`current_oj` 获取读锁返回当前值
5. **列出可用 OJ**：`list_available` 以 AuthProvider 的 key 集合为准（注册过 AuthProvider 即视为可用 OJ）
