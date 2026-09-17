# Hinina 开发路线图

> 当前状态：阶段 7（Vue 前端）已完成，进入阶段 8（后续增强）。
> 开发顺序遵循自底向上依赖链：Infrastructure → Domain → Service → Adapter → Command → Frontend。

---

## 阶段 1：基础设施（Infrastructure）

> 所有上层模块的依赖根基，必须先完成。

- [x] **Logger 初始化** — `infra/logger.rs`：配置 tracing-subscriber，设定日志级别和输出目标，敏感信息过滤
- [x] **Storage 实现** — `infra/storage.rs`：补充文件读写方法（`read` / `write` / `exists` / `create_dir`）
- [x] **HttpClient 封装** — `infra/http.rs`：封装 reqwest::Client，统一超时/重试/UA/Cookie Store
- [x] **AppContext::init() 实现** — `core/context.rs`：按注释中 9 步顺序装配所有基础设施

---

## 阶段 2：存储抽象层（Repository）

> Infrastructure 就绪后，实现文件系统 Repository。

- [x] **FsWorkspaceRepository** — `infra/fs_workspace_repo.rs`：基于 Storage 实现 WorkspaceRepository trait（保存/读取/递归列表/删除/存在性检查 + 路径穿越校验，7 项单元测试）
- [x] **FsConfigRepository** — `infra/fs_config_repo.rs`：基于 Storage 实现 ConfigRepository trait（JSON 序列化/反序列化/存在性检查，5 项单元测试）

---

## 阶段 3：领域核心（Domain）

> Repository 就绪后，填充核心实体和事件总线逻辑。

- [x] **EventBus 实现** — `core/event/event_bus.rs`：`publish()` / `subscribe()` / `unsubscribe()` 完整逻辑，含 AppEvent::category() 映射、All 通配订阅、锁外回调防死锁。8 项单元测试
- [x] **AppError From 转换** — `core/error.rs`：`io::Error` / `reqwest::Error` / `serde_json::Error` — 阶段 1 已提前完成
- [x] **Workspace 实体完善** — `core/entity/workspace.rs`：`new()` / `touch()` / `mark_dirty()` / `mark_clean()` 方法。5 项单元测试

---

## 阶段 4：服务层（Service）

> 按依赖顺序实现，每个 Service 完成后对应 Command 也可同步填充。

### 4.1 ConfigService
- [x] **ConfigService** — `service/config/mod.rs`：加载/保存/监听配置变更，发布 SystemEvent::ConfigReloaded

### 4.2 AuthService
- [x] **AuthService** — `service/auth/mod.rs`：登录流程编排、会话持久化、登出清理

### 4.3 ContestService
- [x] **ContestService** — `service/contest/mod.rs`：比赛获取、列表缓存、当前比赛切换

### 4.4 ProblemService
- [x] **ProblemService** — `service/problem/mod.rs`：题目获取、本地缓存、题目切换时保留代码

### 4.5 WorkspaceManager
- [x] **WorkspaceManager 完整实现** — `service/workspace/manager.rs`：create / load / save / auto-save / switch / destroy / recover 全部方法

### 4.6 SubmissionService
- [x] **SubmissionService** — `service/submission/mod.rs`：提交代码、评测结果轮询、超时处理

### 4.7 ThemeService
- [x] **ThemeService** — `service/theme/mod.rs`：主题切换、配色方案管理，发布 SystemEvent::ThemeChanged

---

## 阶段 5：OJ 适配器（Adapter）

> Service 层就绪后，首先实现 HOJ Adapter 验证 Provider trait 设计的通用性。

- [x] **HOJ Adapter** — `adapter/hoj/`：实现 AuthProvider + ContestProvider + ProblemProvider + SubmissionProvider 四个 trait
  - [x] HOJ DTO 类型定义 — `adapter/hoj/types.rs`：ApiResponse<T>、LoginRequest、UserInfoVO、ContestVO、ContestProblemVO、ProblemInfoVO、JudgeVO、状态码映射
  - [x] HOJ API 对接 — 登录、比赛列表、题目详情、提交代码、评测结果

---

## 阶段 6：Tauri Command（IPC）

> 每个 Service 完成后，对应的 Command 薄封装可同步填充。

- [x] **auth_cmd** — `commands/auth_cmd.rs`：login / logout / get_session
- [x] **contest_cmd** — `commands/contest_cmd.rs`：list_contests / select_contest
- [x] **problem_cmd** — `commands/problem_cmd.rs`：get_problem / list_problems
- [x] **submission_cmd** — `commands/submission_cmd.rs`：submit_code / get_judgement
- [x] **workspace_cmd** — `commands/workspace_cmd.rs`：load / save / switch / current
- [x] **config_cmd** — `commands/config_cmd.rs`：get_config / reload_config / update_config
- [x] **theme_cmd** — `commands/theme_cmd.rs`：get_theme / set_theme
- [x] **register_commands** — `commands/mod.rs` + `main.rs`：通过 `tauri::generate_handler!` + `invoke_handler()` 注册所有 18 个 Command

---

## 阶段 7：Vue 前端

> 后端 5 个核心功能闭环打通后，开始前端开发。

- [x] **项目初始化** — Vite + Vue3 + TypeScript + TailwindCSS + Naive UI + Monaco Editor
- [x] **Bridge 层** — `src/bridge/`：封装 Tauri IPC invoke 调用（auth, contest, problem, submission, workspace, config）
- [x] **Service 层** — `src/services/`：auth / contest / problem / submission / workspace 业务逻辑
- [x] **Store 层** — `src/stores/`：Pinia 状态管理（auth, contest, problem, submission, workspace）
- [x] **登录页** — `views/LoginView.vue`
- [x] **比赛页** — `views/ContestView.vue`（单比赛模式，三栏布局：题目列表 + 题面 + 编辑器/提交面板）
- [x] **题目阅读器** — `components/problem/ProblemStatement.vue`（分栏布局：题面 + 样例）
- [x] **代码编辑器** — `components/editor/CodeEditor.vue`（Monaco Editor + 语言切换 + 提交按钮）
- [x] **编辑器设置弹层** — `components/editor/EditorSettingsPopover.vue`（字号 / Tab 宽度 / 编辑器主题 `vs`·`vs-dark`，即时生效 + debounce 落盘；值域唯一来源 `utils/editor.ts`，Rust `EditorConfig::sanitize` 兜底收敛）
- [x] **提交结果面板** — `components/submission/SubmissionPanel.vue`
- [x] **Rust 后端补齐** — OjConfig 增加 contest_id、Contest 实体扩展、load_configured_contest 命令、WorkspaceManager find_or_create（P36）、auto-save Tauri runtime（P39）
- [x] **评测页** — `views/SubmissionsView.vue`（筛选工具条 + 提交表格 + 分页；onlyMine 后端强制；`?problem=` 自动预筛）+ `views/SubmissionDetailView.vue`（判定横幅 + 测试点明细/子任务 + 只读代码）
- [x] **公告页** — `views/AnnouncementsView.vue`（卡片 feed + 长文折叠）+ 客户端已读状态（Rust 文件持久化，ActivityBar 未读红点）
- [x] **设置页** — `views/SettingsView.vue`（OJ / 编辑器 / 布局 / 主题置灰 / 关于 五分组，P55 配置值域统一与消费落地）

---

## 阶段 8：后续增强

> MVP 之后按优先级推进。

- [ ] **QDUOJ Adapter** — `adapter/qduoj/`
- [ ] **HUSTOJ Adapter** — `adapter/hustoj/`
- [ ] **ProviderRegistry 完善** — 支持多 OJ 动态注册与切换
- [ ] **Plugin 运行时** — v1.0 JS 插件运行时，v2.0 WASM 运行时
- [ ] **CI/CD 流水线** — GitHub Actions：构建 → 测试 → 打包 → Release
