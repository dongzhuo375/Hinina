# mod

## 职责
Commands 模块入口。声明所有 Tauri Command 子模块并挂载本层单元测试。**不做注册**：全部 Command 由 `main.rs` 的 `tauri::generate_handler![...]` 一次性注册（选用 generate_handler 而非 `.setup()` 手动注册，避免兼容性问题，见本文件头部注释）。

## 核心类型/函数
无（仅模块声明）：
- `pub mod auth_cmd` — 登录 / 登出 / 会话查询 / 会话校验
- `pub mod data_dir_cmd` — 数据目录：`get_data_dir` / `set_data_dir` / `reset_data_dir` / `pick_data_dir`（改动重启后生效）
- `pub mod maintenance_cmd` — 重置客户端（`reset_client`：三层缓存 + 公告基线 + 公告已读状态）与清理本地数据（`local_data_usage` 预览 / `purge_local_data` 删日志内容与过期留档）
- `pub mod config_cmd` — 配置读取 / 热重载 / 更新 + `get_storage_info`（存储与版本信息）
- `pub mod contest_cmd` — 比赛列表 / 选中 / 加载配置比赛 / 榜单 / 公告与已读状态
- `pub mod oj_cmd` — 切换当前 OJ（补注册 → 校验已注册 → 切 Registry → 显式清三个 Service 的 OJ 域缓存 → 持久化 `oj.active` → 发布 `CoreEvent::OjSwitched`）
- `pub mod problem_cmd` — 题目详情 / 用户题目状态 / 题目限制
- `pub mod submission_cmd` — 提交 / 评测结果单次查询 / 提交列表 / 详情 / 测试点
- `pub mod theme_cmd` — 主题读取 / 设置
- `pub mod workspace_cmd` — 工作区加载 / 保存 / 切换 / 当前 / 文件更新 / 语言设置 / 文件删除
- `pub mod plugin_cmd` — 插件控制：`plugin_list` / `plugin_start` / `plugin_stop` / `plugin_reload` / `plugin_resync`（`PluginManager` 薄转发）
- `#[cfg(test)] #[path = "tests/mod_tests.rs"] mod tests` — 本层单元测试

## 直接依赖
无外部依赖，仅声明子模块。

## 被依赖
- `src-tauri/src/main.rs`（`use hinina_lib::commands` + `generate_handler!` 逐个注册 45 个 Command：plugin 5 / auth 4 / oj 1 / contest 7 / problem 3 / submission 5 / workspace 7 / config 4 / maintenance 3 / data_dir 4 / theme 2）

## 逻辑流程
无（仅模块声明）。

## 测试
**M2 起命令本体直接可测**：有编排逻辑的命令把实现提取为 `pub(crate) async fn *_impl(ctx: &AppContext, ...)`，`#[tauri::command]` 只做薄转发（零行为变化）。测试经 `test_support::app_ctx::context_with_hoj` 构造**全真上下文**（`AppContext::init` 跑在 TempDir 上、HOJ 实例指向 loopback 脚本 server），直接调用 `*_impl` 验证生产行为 —— 分页钳制与 `only_mine` 强制以脚本 server **实际收到**的查询串断言，`switch_oj` 的注册校验/配置落盘/`OjSwitched` 事件、logout 的会话清理、reset/purge 的精确删除范围均在真实文件系统与事件总线上验证。

`tests/` 下按命令分文件（`auth/oj/maintenance/submission/contest_cmd_tests.rs`）；`mod_tests.rs` 保留独立于命令编排的纯函数与 VO 锁定用例（`auto_save_action` 判据表、`run_purge` 容错、`validate_reset`、`StorageInfo`/`DataDirInfo` 序列化、榜单默认页大小三处同值锁定）。M2 已删除四个「测试文件内复制等价逻辑」的用例（分页钳制、`session_uid`、空白过滤、`Option::ok_or_else`），由真实调用取代。
