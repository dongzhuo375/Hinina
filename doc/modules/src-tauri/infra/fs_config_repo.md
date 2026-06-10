# fs_config_repo

## 职责
`ConfigRepository` trait 的文件系统实现，负责应用配置的序列化持久化（JSON 格式的加载与保存）。所有方法当前为 TODO 占位。

## 核心类型/函数
- **`FsConfigRepository`** — 文件系统配置仓库 struct，持有 `Arc<Storage>` 和配置文件路径
- **`FsConfigRepository::new(storage: Arc<Storage>, config_path: &str)`** — 构造函数
- **`load_config<T: DeserializeOwned>()`** — 从文件加载并反序列化配置（TODO）
- **`save_config<T: Serialize>(config: &T)`** — 序列化配置并写入文件（TODO）
- **`config_exists()`** — 检查配置文件是否存在（TODO）

## 直接依赖
- `std::sync::Arc`
- `serde::{de::DeserializeOwned, Serialize}`
- `core::error::AppResult`
- `core::repository::config_repo::ConfigRepository`
- `infra::storage::Storage`

## 被依赖
暂无（未被 infra 外部模块直接引用，预期由 `service::config` 通过 trait 使用）

## 逻辑流程
构造函数接收 `Arc<Storage>` 和配置文件路径。`load_config` 从 `Storage` 读取文件并通过 serde 反序列化为泛型配置类型；`save_config` 将配置序列化为 JSON 后写入文件。`config_exists` 检查文件是否存在于 `Storage` 中。
