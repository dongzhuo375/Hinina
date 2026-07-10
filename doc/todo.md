# Hinina 开发路线图

> 当前状态：阶段 1（基础设施）已完成，进入阶段 2（存储抽象层）。
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

- [ ] **FsWorkspaceRepository** — `infra/fs_workspace_repo.rs`：基于 Storage 实现 WorkspaceRepository trait
- [ ] **FsConfigRepository** — `infra/fs_config_repo.rs`：基于 Storage 实现 ConfigRepository trait（JSON 序列化）

---

## 阶段 3：领域核心（Domain）

> Repository 就绪后，填充核心实体和事件总线逻辑。

- [ ] **EventBus 实现** — `core/event/event_bus.rs`：`publish()` / `subscribe()` / `unsubscribe()` 完整逻辑
- [ ] **AppError From 转换** — `core/error.rs`：为 `io::Error`、`reqwest::Error`、`serde_json::Error` 等实现 `From`
- [ ] **Workspace 实体完善** — `core/entity/workspace.rs`：补充 `created_at` / `updated_at` 时间戳

---

## 阶段 4：服务层（Service）

> 按依赖顺序实现，每个 Service 完成后对应 Command 也可同步填充。

### 4.1 ConfigService
- [ ] **ConfigService** — `service/config/mod.rs`：加载/保存/监听配置变更，发布 SystemEvent::ConfigReloaded

### 4.2 AuthService
- [ ] **AuthService** — `service/auth/mod.rs`：登录流程编排、会话持久化、登出清理

### 4.3 ContestService
- [ ] **ContestService** — `service/contest/mod.rs`：比赛获取、列表缓存、当前比赛切换

### 4.4 ProblemService
- [ ] **ProblemService** — `service/problem/mod.rs`：题目获取、本地缓存、题目切换时保留代码

### 4.5 WorkspaceManager
- [ ] **WorkspaceManager 完整实现** — `service/workspace/manager.rs`：create / load / save / auto-save / switch / destroy / recover 全部方法
  > ⚠️ 上下文：`AppContext.workspace_manager` 当前为 `Option<Arc<WorkspaceManager>> = None`（阶段 1 PR5 Review P5-1），应用可正常启动但无工作区管理能力。实现后需改回 `Some(...)`。

### 4.6 SubmissionService
- [ ] **SubmissionService** — `service/submission/mod.rs`：提交代码、评测结果轮询、超时处理

### 4.7 ThemeService
- [ ] **ThemeService** — `service/theme/mod.rs`：主题切换、配色方案管理，发布 SystemEvent::ThemeChanged

---

## 阶段 5：OJ 适配器（Adapter）

> Service 层就绪后，首先实现 HOJ Adapter 验证 Provider trait 设计的通用性。

- [ ] **HOJ Adapter** — `adapter/hoj/`：实现 AuthProvider + ContestProvider + ProblemProvider + SubmissionProvider 四个 trait
  - [ ] HOJ DTO 类型定义 — `adapter/hoj/types.rs`
  - [ ] HOJ API 对接 — 登录、比赛列表、题目详情、提交代码、评测结果

---

## 阶段 6：Tauri Command（IPC）

> 每个 Service 完成后，对应的 Command 薄封装可同步填充。

- [ ] **auth_cmd** — `commands/auth_cmd.rs`：login / logout / get_session
- [ ] **contest_cmd** — `commands/contest_cmd.rs`：list_contests / select_contest
- [ ] **problem_cmd** — `commands/problem_cmd.rs`：get_problem / list_problems
- [ ] **submission_cmd** — `commands/submission_cmd.rs`：submit_code / get_judgement
- [ ] **workspace_cmd** — `commands/workspace_cmd.rs`：load / save / switch / current
- [ ] **config_cmd** — `commands/config_cmd.rs`：get_config / update_config
- [ ] **theme_cmd** — `commands/theme_cmd.rs`：get_theme / set_theme
- [ ] **register_commands** — `commands/mod.rs`：将所有 Command 注册到 Tauri App

---

## 阶段 7：Vue 前端

> 后端 5 个核心功能闭环打通后，开始前端开发。

- [ ] **项目初始化** — Vite + Vue3 + TypeScript + TailwindCSS + Naive UI + Monaco Editor
- [ ] **Bridge 层** — `src/bridge/`：封装 Tauri IPC invoke 调用
- [ ] **Service 层** — `src/services/`：auth / contest / problem / submission / workspace 业务逻辑
- [ ] **Store 层** — `src/stores/`：Pinia 状态管理
- [ ] **登录页** — `views/LoginView.vue`
- [ ] **比赛列表页** — `views/ContestListView.vue`
- [ ] **题目阅读器** — `views/ProblemView.vue`（分栏布局：题面 + 代码编辑器）
- [ ] **提交结果面板** — `views/SubmissionView.vue`

---

## 阶段 8：后续增强

> MVP 之后按优先级推进。

- [ ] **QDUOJ Adapter** — `adapter/qduoj/`
- [ ] **HUSTOJ Adapter** — `adapter/hustoj/`
- [ ] **ProviderRegistry 完善** — 支持多 OJ 动态注册与切换
- [ ] **Plugin 运行时** — v1.0 JS 插件运行时，v2.0 WASM 运行时
- [ ] **CI/CD 流水线** — GitHub Actions：构建 → 测试 → 打包 → Release
