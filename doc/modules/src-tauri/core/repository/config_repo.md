# config_repo

## 职责
定义配置持久化仓库 trait `ConfigRepository`，抽象配置文件读写，解耦 ConfigService 与底层存储实现。支持泛型加载/保存，配置文件格式由实现决定。

## 核心类型/函数
- **`ConfigRepository`** — 配置仓库 trait，方法：
  - `load_config<T: DeserializeOwned>(&self) -> AppResult<T>` — 加载配置
  - `save_config<T: Serialize>(&self, config) -> AppResult<()>` — 保存配置
  - `config_exists(&self) -> bool` — 检查配置文件是否存在

## 直接依赖
- `serde::{de::DeserializeOwned, Serialize}`
- `core::error::AppResult`

## 被依赖
- `infra::fs_config_repo`

## 逻辑流程
无（纯 trait 定义）。
