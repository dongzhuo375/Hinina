# oj_id

> 源文件：`src-tauri/src/core/provider/oj_id.rs`

## 职责
定义 OJ 身份标识 newtype `OjId`。OJ 身份是**数据而非编译期枚举**：接入一个新 OJ 不应要求修改 Domain（Clean Architecture：领域类型不随外部集成点的数量变化），适配器经 `AdapterFactory::id()` 自声明身份（如 `adapter::hoj::HOJAdapter::ID`），本类型只做非空规整（`new` 内 trim 首尾空白）。

## 核心类型/函数
- **`OjId(String)`** — OJ 身份 newtype，derive `Debug / Clone / PartialEq / Eq / Hash / Serialize / Deserialize`
- **`OjId::new(raw: &str) -> Self`** — 构造，自动 trim（配置手改常见首尾空白）
- **`OjId::as_str(&self) -> &str`** — 取内部字符串
- **`OjId::session_file(&self) -> String`** — 会话文件名（不含目录）：`{id}.json`，如 `HOJ.json`
- **`impl Display`** — 输出同 `as_str()`，供日志与错误消息直接使用

## 直接依赖
- `serde::{Deserialize, Serialize}`
- `std::fmt`

## 被依赖
- `core::provider::registry`（ProviderRegistry 以 `OjId` 为注册/查询/切换键）
- `infra::provider_registry_impl`
- `core::context`（按 `oj.active` 构造当前 OJ 身份、按实例 id 匹配工厂）
- `commands::oj_cmd`（`switch_oj` 入参规整）
- `service::auth`（会话文件路径经 `session_file()` 拼接）

## 逻辑流程
无（纯类型定义）。

## 设计要点
- **持久化契约**：会话文件名 = `{id}.json`（如 `sessions/HOJ.json`）。内建 OJ 的 id 必须与历史枚举变体的 Debug 输出完全一致（`"HOJ"`），升级后既有会话文件无需迁移。
- **数据化的代价与防线**：失去闭集枚举的编译期穷尽检查，防线移至三处 —— 启动时校验当前 OJ 已注册并告警（`core::context`）、查询未命中返回 `AppError::ProviderNotFound`（`infra::provider_registry_impl`）、`factories()` 的 id 唯一性测试（`adapter/tests/adapter_tests.rs`）。
