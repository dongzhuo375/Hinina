# api/storage

## 职责
插件私有 JSON 存储：按插件哈希目录隔离的键值存储，带 CAS 与每激活速率预算。

## 核心类型/函数
- `MAX_STORAGE_KEY_BYTES = 256` / `MAX_STORAGE_VALUE_BYTES = 65_536`
- `PluginStorageValue` — `{state:"missing"}` 或 `{state:"value", value}`：JSON null 与不存在明确区分
- `validate_key` / `validate_value` — 键 1..256 UTF-8 字节无控制字符；value JSON 编码 ≤64 KiB
- `PluginStorageApi`（trait）— `get(key)` / `set(key, value)` / `delete(key)` / `compareAndSet(key, expected, value)`（expected 用同一带 state 的类型）

## 直接依赖
- `plugin::api::PluginError / PluginResult`
- `async_trait`、`serde`、`serde_json::Value`

## 被依赖
- `plugin::execution_api`（能力转发）
- `infra::plugin_storage`（trait 实现：fs4 锁 + atomicwrites，见其模块文档）

## 逻辑流程
- **存储位置**：`<base_dir>/plugin-data/<插件ID的SHA-256>/storage.json`，与插件代码分开；键只是数据字符串，不能指定文件路径。每次调用独立检查宿主授权和停止状态
- **CAS**：`compareAndSet` 同时需 storage.read 与 storage.write；独立 `storage.lock` 经 fs4 跨实例锁定，读取→比较→原子持久化均在锁内，只有 atomicwrites 成功后才返回 true。锁等待最多 5 秒、可被停止取消；阻塞文件操作放在 spawn_blocking
- **配额**：完整文件编码（含 key）≤1 MiB；读/写/删除/CAS 共用每激活 60 次/分钟操作预算，工厂另有跨激活/重载 240 次/分钟总预算（令牌桶允许相应初始突发）；超限在 I/O 前立即返回 `limit_exceeded`。最多 16 个在途事务，取消等待不会提前释放仍在执行的阻塞事务名额
- **损坏与生命周期**：损坏、不可读或异常超大文件报错并保留原件，不静默重置；停止、卸载、重载不删数据。宿主停止前已开始的原子写可能完成，不能保证回滚。CAS 只保证本地 JSON 事务，**不代表远端 exactly-once**
