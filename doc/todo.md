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

- [x] **EventBus 实现（阶段 3 初版，已于阶段 8 重构）** — `core/event/event_bus.rs`：`publish()` / `subscribe()` / `unsubscribe()`、AppEvent::category() 映射、All 通配订阅、锁外回调防死锁。该实现**已被阶段 8 的 CoreEventBus 取代并删除**
- [x] **AppError From 转换** — `core/error.rs`：`io::Error` / `reqwest::Error` / `serde_json::Error` — 阶段 1 已提前完成
- [x] **Workspace 实体完善** — `core/entity/workspace.rs`：`new()` / `touch()` / `mark_dirty()` / `mark_clean()` 方法。5 项单元测试

---

## 阶段 4：服务层（Service）

> 按依赖顺序实现，每个 Service 完成后对应 Command 也可同步填充。

### 4.1 ConfigService
- [x] **ConfigService** — `service/config/mod.rs`：加载/保存/监听配置变更，发布配置变更事件（阶段 8 事件总线重构后为 `CoreEvent::ConfigChanged`；当时为 `SystemEvent::ConfigReloaded`）

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
- [x] **ThemeService** — `service/theme/mod.rs`：主题切换、配色方案管理，发布主题变更事件（阶段 8 事件总线重构后为 `CoreEvent::ThemeChanged`；当时为 `SystemEvent::ThemeChanged`）

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
- [x] **Bridge 层** — `src/bridge/`：封装 Tauri IPC invoke 调用（auth / contest / problem / rank / submission / announcement / workspace / config / system）
- [x] **Service 层** — `src/services/`：auth / config / contest / problem / rank / submission / announcement / system / workspace 业务逻辑
- [x] **Store 层** — `src/stores/`：Pinia 状态管理（auth / contest / problem / rank / submission / announcement / workspace + 会话级清理 `session.ts`）
- [x] **登录页** — `views/LoginView.vue`（左栏 `components/login/LoginForm.vue` ｜ 右栏 `EventTitle.vue` + `Countdown.vue`；会话恢复、按比赛阶段决策进场、匿名简报有界重试、赛前会话错峰预检。PR #35 按设计稿重构）
- [x] **比赛工作台外壳** — `views/ContestLayout.vue`（TopBar + `<router-view>` + StatusBar；各功能页是**平级子路由**而非单页三栏，自 #14 起取代早期的 `views/ContestView.vue`）+ `components/layout/{TopBar,ActivityBar,StatusBar}.vue`（PR #36：TopBar 改两行式并把导航条内嵌其中，外壳承担赛程闸门与跨场缓存刷新）
- [x] **题目总览** — `views/ProblemSetView.vue`（统计卡 + 题目列表，limits 渐进填充，30s±5s 轮询；PR #36 由卡片网格改为紧凑目录行）+ `components/problem/ProblemCard.vue`
- [x] **题目阅读器** — `components/problem/ProblemStatement.vue`（分栏布局：题面 + 样例；容器查询驱动排版）
- [x] **代码编辑器** — `components/editor/CodeEditor.vue`（Monaco Editor + 语言切换 + 提交按钮；PR #36 语言菜单改 Teleport 浮层）
- [x] **编辑器设置弹层** — `components/editor/EditorSettingsPopover.vue`（字号 / Tab 宽度 / 编辑器主题 `vs`·`vs-dark`，即时生效 + debounce 落盘；值域唯一来源 `utils/editor.ts`，Rust `EditorConfig::sanitize` 兜底收敛）
- [x] **提交反馈与快捷提交** — `components/editor/EditorConsoleBar.vue`（最新记录 pill + 失败原因首行）+ `components/problem/QuickSubmitDialog.vue`（题目行内快捷提交）。两者自 #14 工作台重构起取代了早期的 `components/submission/SubmissionPanel.vue`（该文件与 `views/ContestView.vue` 均已删除）；PR #36 补上提交即时反馈与重复发送三层防护
- [x] **Rust 后端补齐** — OjConfig 增加 contest_id、Contest 实体扩展、load_configured_contest 命令、WorkspaceManager find_or_create（P36）、auto-save Tauri runtime（P39）
- [x] **评测页** — `views/SubmissionsView.vue`（筛选工具条 + 提交表格 + 分页；onlyMine 后端强制；`?problem=` 自动预筛）+ `views/SubmissionDetailView.vue`（判定横幅 + 测试点明细/子任务 + 只读代码）
- [x] **榜单页** — `views/RankView.vue` + `components/rank/{RankToolbar,ScoreboardTable,RankCell}.vue` + `components/contest/ContestStatsBar.vue`（ACM/OI 分流、封榜提示、打星/女生全量快照模式）
- [x] **公告页** — `views/AnnouncementsView.vue`（卡片 feed + 长文折叠）+ 客户端已读状态（Rust 文件持久化，ActivityBar 未读红点）
- [x] **设置页** — `views/SettingsView.vue`（OJ / 编辑器 / 布局 / 主题置灰 / **数据目录** / **重置与清理** / 关于 七个分组，P55 配置值域统一与消费落地）
- [x] **赛场视觉体系** — `styles/contest.css`（`.contest-theme` 令牌 + `--ui-step` 响应式密度）+ `components/common/PageHeading.vue`（01–05 编号页头）+ `utils/focus-scope.ts`（弹层焦点作用域指令）+ 5 个 `@fontsource-variable/*` 本地打包字体（PR #36）
- [x] **客户端缓存优化** — `infra/cache.rs` 落地 `TtlCache` + `JsonDiskCache` 原语；比赛元信息（TTL 120s，内存+磁盘）、题面（TTL 30min，内存+磁盘，`oj.cacheProblemStatement` 开关）、终态提交详情/测试点（TTL 2h，仅内存 + 登出清理）；前端 `myStatus` 增量失效 + 榜单用户操作路径去抖 memo。判据与失效路径见 `doc/Architecture.md`「客户端缓存策略」

---

## 阶段 8：后续增强

> MVP 之后按优先级推进。

### 8.1 事件总线深度重构（已完成）

> 主题：把「事实通知」与「必须完成的核心动作」彻底分开。旧 `EventBus` 的同步订阅表 + 延迟线程同时承担了两种语义，导致三处隐式同步依赖。

- [x] **CoreEventBus（tokio broadcast）** — `core/event/{core_event,core_event_bus,consumer}.rs`：单一 broadcast 通道（容量 1024）、`publish` 同步非阻塞、受监督消费者（`Lagged` → resync、`Closed` → 干净退出、panic → 退避重启）。删除旧 `AppEvent` / `EventCategory` / 手写 `EventBus`
- [x] **SessionStore 显式持久化** — `core/entity/session.rs` + `core/repository/session_repo.rs` + `infra/fs_session_repo.rs`：Provider 凭证轮换**当场显式落盘**（`rotate_token`），此后只发脱敏的 `TokenRotated`；token 不再进入事件流
- [x] **OJ 切换显式清缓存** — `switch_oj` 调 `contest/problem/submission.on_oj_switched()`，`OjSwitched` 降为纯事实通知（不再有订阅者）
- [x] **配置热生效显式化** — `update_config` / `reload_config` 显式调 `sync_auto_save_from_context`，`ConfigChanged` 降为纯通知
- [x] **工作区落盘事件合并** — `WorkspaceSaved{workspace_id,revision,automatic}` 取代 `Saved` / `AutoSaveTriggered`，并删除无消费者的 `Loaded` / `Switched`
- [x] **事件载荷收敛** — 删除查询结果型与死事件（`ListLoaded` / `CountdownTick` / `CodeChanged` / `WindowClosing`），`LoginSuccess{User}` → `LoggedIn{user_id}`，`Judged{result}` → `SubmissionJudged{status}`
- [x] **三类消费者** — Tauri 前端桥 / 审计（`infra/audit.rs`）/ PluginHost，共用同一条底层事件流
- [x] **插件事件边界** — `PluginEvent` + `PluginEventEnvelope`（版本 + 每订阅单调序号）+ `PluginEventAdapter`（白名单 / 裁剪 / 脱敏）+ `PluginHost`（授权与订阅管理、`ResyncRequired`；后续接口施工以有界队列落实隔离）；运行时由独立Manager/QuickJS worker接入
- [x] **前端最小适配** — `workspace-saved` 载荷增加 `revision`（幂等过滤）；**全部轮询保留**（事件只做 UI 刷新触发）

### 8.2 待办

- [ ] **QDUOJ Adapter** — `adapter/qduoj/`
- [ ] **HUSTOJ Adapter** — `adapter/hustoj/`
- [ ] **ProviderRegistry 完善** — 支持多 OJ 动态注册与切换
- [x] **Plugin 宿主（PR #40/#41）** — 契约 v2 + 最小可运行 QuickJS 宿主；架构/契约/施工见 [doc/plugin/](plugin/README.md)
- [x] **Plugin 参考业务插件** — 首次 AC 气球派发参考实现（`sdk/examples/balloon`：封榜门控/跨页去重/幂等派发/不补派 + 逻辑测试（fake host 强制宿主规则，数量以 vitest 输出为准）+ 验收指南）
- [ ] **Plugin 业务集成验证** — 真实 HOJ/Hydro + 外部服务联调：按 `sdk/examples/balloon/README.md` 验收清单人工执行（公网 HTTPS echo 端点 + 假队伍名）
- [ ] **CI/CD 流水线** — M1 PR 门禁已落地（`ci.yml`：frontend/rust-test 必需 + rust-lint 暂非阻塞，见 [doc/cicd/](cicd/README.md)）；待办：M2 测试补位 → M3 集成/E2E → M4 Windows 草稿 Release → M5 签名跨平台 → M6 updater

### 插件系统后续（详见 [doc/plugin/roadmap.md](plugin/roadmap.md)）

- [ ] 管理基础：目录盘点与逐项诊断、持久启用/禁用、原子配置/授权写入及应用结果
- [ ] 最小管理页：登录前可访问，展示运行与启用状态、权限/网络批准、配置和安全诊断
- [ ] 本地分发维护：安装预检、发布物摘要/更新差异、失败恢复及保留数据的移除
- [ ] 按实际需求补配置 schema 子集、系统凭据引用、数据备份和全局资源预算
- [ ] 跨平台构建（Linux/macOS）与真实 OJ/外部服务验收
