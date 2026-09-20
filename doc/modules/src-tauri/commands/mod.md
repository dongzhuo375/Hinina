# mod

## 职责
Commands 模块入口。声明所有 Tauri Command 子模块并挂载本层单元测试。**不做注册**：全部 Command 由 `main.rs` 的 `tauri::generate_handler![...]` 一次性注册（选用 generate_handler 而非 `.setup()` 手动注册，避免兼容性问题，见本文件头部注释）。

## 核心类型/函数
无（仅模块声明）：
- `pub mod auth_cmd` — 登录 / 登出 / 会话查询 / 会话校验
- `pub mod maintenance_cmd` — 重置客户端（`reset_client`：三层缓存 + 公告基线 + 公告已读状态）与清理本地数据（`local_data_usage` 预览 / `purge_local_data` 删日志内容与过期留档）
- `pub mod config_cmd` — 配置读取 / 热重载 / 更新 + `get_storage_info`（存储与版本信息）
- `pub mod contest_cmd` — 比赛列表 / 选中 / 加载配置比赛 / 榜单 / 公告与已读状态
- `pub mod oj_cmd` — 切换当前 OJ（校验已注册 → 切 Registry → 持久化 `oj.active` → 发布 `OJSwitched`）
- `pub mod problem_cmd` — 题目详情 / 列表 / 用户题目状态 / 题目限制
- `pub mod submission_cmd` — 提交 / 评测结果单次查询 / 提交列表 / 详情 / 测试点
- `pub mod theme_cmd` — 主题读取 / 设置
- `pub mod workspace_cmd` — 工作区加载 / 保存 / 切换 / 当前 / 文件更新 / 语言设置
- `#[cfg(test)] #[path = "tests/mod_tests.rs"] mod tests` — 本层单元测试

## 直接依赖
无外部依赖，仅声明子模块。

## 被依赖
- `src-tauri/src/main.rs`（`use hinina_lib::commands` + `generate_handler!` 逐个注册 36 个 Command）

## 逻辑流程
无（仅模块声明）。

## 测试
`src-tauri/src/commands/tests/mod_tests.rs` 锁定 Command 层**可测的纯部分**：`#[tauri::command]` 函数签名依赖 `tauri::State<'_, AppContext>`，而 `State` 没有公开构造器（只能在 Tauri 运行时内获得），命令本体无法直接单测；其实质逻辑分两段 —— ① 参数默认值/钳制与 uid 解析（以同形纯函数等价锁定），② Service 调用与错误变体穿透（已在各 Service 测试中覆盖）。既有用例：`start_auto_save_if_needed` 的 static AtomicBool 一次性标记语义、workspace_cmd 降级路径的 `Option::ok_or_else` 逻辑（原 `auth_cmd::parse_oj_type` 解析测试已随闭集枚举移除：OJ 身份改为数据 `OjId`，注册校验在 `ProviderRegistry` 层由 `list_available` 承担）。P58 新增：`StorageInfo` camelCase 序列化形状（前端按 `baseDir`/`logPath`/`version` 键读取）、分页参数默认值与钳制（公告默认 50、提交列表默认 20、页码至少 1，0/负数钳到 1）、`session_uid` 解析规则（优先 user_id，缺失回退 username）、提交列表 `problemDisplayId` 空白过滤（空串/纯空白视为不筛选）。
