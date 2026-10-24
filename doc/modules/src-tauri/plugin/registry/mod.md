# registry/mod

## 职责
插件配置/清单/源码的加载端口：宿主配置、批准与已加载插件的数据结构，I/O 实现在 infra/fs_plugin_repo（不再保留 Unsupported 占位实现）。

## 核心类型/函数
- **`PluginApproval`** — 单插件批准（enabled、permissions、network、resyncIntervalSecs、options 等）；`validate()` 校验结构与间隔
- **`PluginConfiguration`** — `plugins.json` 整体（plugins 映射 + codeRoot）；整体 JSON ≤64 KiB
- **`LoadedPlugin`** — `grant: PluginGrant`（授权快照；清单经 `grant.manifest()` 访问，不在顶层）、`source: String`（内存源码）、`approval: PluginApproval`（宿主批准）
- **`PluginRepository`**（trait）— `configuration()` / `load(id)`；Manager 经 `Arc<dyn PluginRepository>` 使用，具体 `FsPluginRepository` 由 AppContext 选择

## 直接依赖
- `plugin::api`（错误）、`plugin::host::manifest`（清单类型）
- `serde`、`std::collections::BTreeMap`、`std::path::PathBuf`

## 被依赖
- `plugin::manager`（启停读取）、`infra::fs_plugin_repo`（实现；**infra → plugin 单向**）

## 逻辑流程
- `configuration()` 读取 `<base_dir>/plugins.json`，缺失时无插件；`load(id)` 只加载已启用批准的 ID
- 扫描边界：默认代码根 `<base_dir>/plugins`（管理员可在 plugins.json 的 `codeRoot` 指定绝对开发路径）；最多扫描 128 个目录/1024 条目；重复目标 ID 拒绝、未知清单/最低应用版本拒绝、入口 canonical 路径检查、源码 ≤1 MiB
- **故障隔离**：配置整体 JSON/根字段/目录约束失败才使 configuration 整体失败；每个 approval 独立解析校验，错误进入宿主私有 `invalid_plugins`，Manager 列表逐项报告。load(id) 只拒绝目标的错误——非法权限、网络或 resync 间隔的无关插件不会阻塞好插件
- **信任边界**：清单和源码可读不等于授权；读取批准与声明的交集，执行前由 Manager 创建独立授权代次。入口加载后以内存源码执行；本地能修改宿主配置/安装目录的管理员在信任边界内，本实现不抵抗管理员同时替换磁盘对象的攻击
