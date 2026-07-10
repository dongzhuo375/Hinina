# fs_config_repo

## 职责
`ConfigRepository` trait 的文件系统实现，负责应用配置的 JSON 序列化持久化（加载与保存）。所有方法已完整实现并通过单元测试。

## 核心类型/函数
- **`FsConfigRepository`** — 文件系统配置仓库 struct，持有 `Arc<Storage>` 和配置文件路径
- **`FsConfigRepository::new(storage: Arc<Storage>, config_path: &str)`** — 构造函数
- **`load_config<T: DeserializeOwned>()`** — 从文件读取 JSON 并反序列化为泛型配置类型，文件不存在返回 `AppError::Config`，解析失败返回 `AppError::Serialization`
- **`save_config<T: Serialize>(config: &T)`** — 将配置序列化为 pretty JSON 写入文件，序列化失败返回 `AppError::Serialization`
- **`config_exists()`** — 检查配置文件是否存在

## 直接依赖
- `std::sync::Arc`
- `serde::{de::DeserializeOwned, Serialize}`
- `core::error::{AppError, AppResult}`
- `core::repository::config_repo::ConfigRepository`
- `infra::storage::Storage`

## 被依赖
暂无（未被 infra 外部模块直接引用，预期由 `service::config` 通过 trait 使用）

## 逻辑流程
配置文件以 JSON 格式存储在 `{storage.base_dir}/{config_path}` 单文件中。

- **load_config**：调用 `Storage::read_to_string()` 读取原始 JSON，再通过 `serde_json::from_str` 反序列化为泛型类型 `T`。
- **save_config**：通过 `serde_json::to_string_pretty` 序列化为格式化 JSON，再调用 `Storage::write_string` 写入文件（自动创建父目录）。
- **config_exists**：委托 `Storage::exists()` 检查文件存在性。

## 测试覆盖（5 项）
- `save_and_load_roundtrip` — JSON 序列化往返一致性
- `config_exists_returns_false_initially` — 初始配置文件不存在
- `load_missing_config_returns_error` — 读取不存在的配置文件返回错误
- `load_invalid_json_returns_error` — 非法 JSON 反序列化返回错误
- `save_then_exists` — 保存后配置存在性变为 true
