# storage

## 职责
本地文件存储工具，提供底层的文件读写能力。不包含业务语义，业务层应通过 Repository trait 间接使用，不应直接依赖此模块。所有路径必须是 `base_dir` 下**非空**的相对路径：`resolve()` 只放行 `Normal` 组件，拒绝空串/`.`（会解析成 base 自身，`remove_all` 足以删掉数据根目录或整个 `workspaces/`）、`..`（穿越）与盘符/根目录等绝对形态（`PathBuf::join` 对带 prefix/root 的路径会整体替换 `base_dir`）。`resolve()` 只做**词法**校验、不解析符号链接；需要防链接穿越的读取用 `symlink_metadata()`（lstat 语义）。workspace 文件名的更严白名单在 `fs_workspace_repo::workspace_relative`，本函数是所有 Storage 调用的公共底线。

## 核心类型/函数
- **`Storage`** — 文件存储 struct，持有基础目录 `base_dir: PathBuf`
- **`Storage::new(base_dir: PathBuf)`** — 构造函数，指定存储根目录
- **`Storage::base_dir()`** — 返回基础目录的不可变引用
- **`Storage::remove_file(relative_path) -> AppResult<()>`** — 删除**文件**（文件专用、不跟随链接）：只做 `fs::remove_file`，目录条目必然失败（目录绝不被删）；Unix 上对符号链接删除链接本身（目标不受影响），Windows 上目录链接同样失败——供「只删文件」类清理使用（2026-11-04，回收趟 TOCTOU 处置）
- **`Storage::remove_empty_dir(relative_path) -> AppResult<()>`** — 删除**空目录**（目录专用、非递归、不跟随链接）：只做 `fs::remove_dir`，文件永不被删、非空目录必然失败；Unix 对符号链接返回 ENOTDIR，Windows 只删链接条目本身（外部目标不受影响）——供「只回收空目录」类清理使用（2026-11-04，回收趟 TOCTOU 处置）
- **`FileStat`** — `symlink_metadata` 的返回值：`len/is_empty/is_file/is_dir/is_symlink/modified` 的不可变快照（私有字段 + 只读方法）。刻意不复用 `std::fs::Metadata`，避免抽象签名绑定本地文件系统
- **`Storage::symlink_metadata(relative_path) -> AppResult<FileStat>`** — 读取文件元数据，**不跟随符号链接**（lstat 语义），返回自有 `FileStat`：`len()` / `is_empty()` / `is_file()` / `is_dir()` / `is_symlink()` / `modified() -> Option<SystemTime>`（`None` = 平台未提供或读取失败，调用方按需跳过）。`resolve()` 只做词法路径校验，跟随链接会让调用方穿过 `base_dir` 内的链接看到/删除外部目标，故供留档统计/清理、日志体积预览等需要路径约束的场景使用（2026-11-04，F5 + 符号链接加固；H4 收为自有类型）
- **`Storage::read(relative_path) -> AppResult<Vec<u8>>`** — 读取二进制文件
- **`Storage::read_to_string(relative_path) -> AppResult<String>`** — 读取 UTF-8 文件
- **`Storage::write(relative_path, data) -> AppResult<()>`** — 写入二进制数据，自动创建父目录
- **`Storage::write_string(relative_path, content) -> AppResult<()>`** — 写入字符串（**截断 + 就地写**：进程在写入中途崩溃会留下半截文件，只适合可重建的数据）
- **`Storage::write_string_atomic(relative_path, content) -> AppResult<()>`** — **原子写**：先写 `{path}.{seq}.tmp`、`sync_all` 刷盘、再 `rename` 覆盖目标（同目录同卷；Windows 上 `std::fs::rename` 以 `MOVEFILE_REPLACE_EXISTING` 原子替换）。写入前先清理同目标的崩溃残片。用于凭据等「重启后必须可恢复」的数据（`FsSessionRepository`）
- **`is_atomic_tmp(target, name) -> bool`**（私有自由函数）— 判断文件名是否为 `target` 的原子写临时残片（`{target}.{数字}.tmp`）。只认本模块命名约定：`HOJ.json.tmp`（无数字段）与其他目标的残片都不匹配，不误伤
- **`Storage::remove_stale_temps(target)`**（私有）— 删除 `target` 同前缀的原子写残片；单个删除失败只跳过（清理失败不该让写入本身失败）
- **`Storage::exists(relative_path) -> bool`** — 检查路径是否存在
- **`Storage::create_dir(relative_path) -> AppResult<()>`** — 递归创建目录
- **`Storage::remove(relative_path) -> AppResult<()>`** — 删除文件或空目录
- **`Storage::remove_all(relative_path) -> AppResult<()>`** — 递归删除文件或目录
- **`Storage::list(relative_path) -> AppResult<Vec<PathBuf>>`** — 列出目录直接子项，返回相对路径

## 直接依赖
- `std::path::{Path, PathBuf, Component}`
- `std::fs`
- `std::io::Write`（原子写用 `File::write_all`）
- `std::time::SystemTime`（`FileStat::modified`）
- `std::sync::atomic::{AtomicU64, Ordering}`（原子写临时文件序号）
- `core::error::{AppError, AppResult}`

## 被依赖
- `core::context`（`AppContext` 持有 `Arc<Storage>` 注入各模块）
- `infra::fs_workspace_repo`（`FsWorkspaceRepository` 依赖 `Storage` 存取工作区文件）
- `infra::fs_config_repo`（`FsConfigRepository` 依赖 `Storage` 存取配置文件）
- `infra::fs_session_repo`（会话写入依赖 **`write_string_atomic`**）

注：`infra::fs_plugin_repo` **不依赖** `Storage` —— `FsPluginRepository` 直接持有插件目录 `PathBuf` 并用 `std::fs` 读写（插件目录在 `plugins/` 下有独立的边界校验规则，见其模块文档）。

## 逻辑流程
构造时接收 `base_dir` 路径并 best-effort 兜底 `create_dir_all`（数据目录由 `data_dir::resolve` 保证已创建，失败不阻断——后续读写以明确 Io 错误暴露）。所有文件操作通过 `resolve()` 方法将相对路径拼接到 `base_dir` 下，遍历 `Path::components()` 只放行 `Normal` 且要求至少一个组件，拒绝 `CurDir`（空串 / `.` / `a/./b`）、`ParentDir`（`..`）与 `Prefix`/`RootDir`（`C:\x`、`/x`、`C:x` 等绝对形态，防 `join` 整体替换 base）（纯逻辑检查，不依赖文件系统）。write 系列方法自动创建不存在的父目录。`list()` 返回 `strip_prefix` 后的相对路径。

`write_string_atomic` 的步骤：`resolve` → 建父目录 → `remove_stale_temps`（清同目标残片）→ 取全局递增 `seq` 拼出 `{path}.{seq}.tmp` → `File::create` + `write_all` + **`sync_all`** → `drop(file)`（Windows 上 rename 前须先关句柄）→ `fs::rename`。写入或刷盘失败、rename 失败都 best-effort 删除临时文件后返回 `AppError::Io`。

**崩溃 / 掉电语义（务必如实理解）**：
- **进程崩溃**：rename 是原子替换，目标要么是旧内容、要么是新内容，不会是半截；
- **掉电**：数据已 fsync，但 rename 的目录项未额外 fsync（Windows 上需 `FILE_FLAG_BACKUP_SEMANTICS` 打开目录句柄，此处不做）—— 日志型文件系统上最坏回退为**旧的完整文件**，同样不会出现半截；
- **残片**：崩溃落在「写完临时文件」与「rename」之间会残留 `{path}.{seq}.tmp`，由下次写入前的 `remove_stale_temps` 回收（rename 成功路径不经过清理分支）；
- **并发同一路径**：可能互相清掉对方的在途临时文件（表现为其中一方收到 `Io` 错误，不会损坏数据）。需要并发安全时由调用方自行串行化（如 `FsSessionRepository` 的 `mutation_lock`）。

## 测试
测试代码位于 `tests/storage_tests.rs`，通过 `#[cfg(test)] #[path = "tests/storage_tests.rs"]` 引用。
路径防线：正常路径通过、`..` 拒绝、**空串 / `.` / `workspaces/.` 拒绝（`remove_all` 不得删到数据根目录）**、绝对形态（`/x`；Windows 下 `C:\x`、`C:x`、verbatim）拒绝、深层嵌套合法路径通过；原子写同样拒绝绝对形态。
原子写：首次写入、覆盖已有内容（rename 覆盖语义）、**写前清理同目标残片且不误伤**（`{target}.{数字}.tmp` 被清；`{target}.tmp` 与其他目标的残片保留）。
元数据（`symlink_metadata`）：常规文件返回 `len/is_file/modified`、缺失/穿越路径报 `Io`；Unix 下链接返回 `is_symlink()` 且 `!is_file()`（lstat，不跟随目标）。
