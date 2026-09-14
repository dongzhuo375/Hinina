# Hinina 项目架构与文件树

> 最后更新：2026-07-13 | 分支：`feat/stage7-frontend`
>
> 本文档记录项目完整文件树，每个文件/目录后附简要职责说明。

---

## 当前文件树（已存在）

```
Hinina/
├── README.md                             # 项目简介（For XCPC）
├── LICENSE                               # MIT 开源许可证
├── .git/                                 # Git 版本控制目录（不入库）
├── .github/
│   └── copilot-instructions.md           # Copilot CLI 指令（架构速查、开发约定）
├── doc/
│   ├── 开发手册.md                        # 项目权威开发手册（架构设计、NFR、风险、协作规范）
│   ├── Architecture.md                   # 本文件：项目文件树与职责说明
│   ├── todo.md                            # NEW: 开发路线图（8 阶段执行顺序）
│   ├── problem.md                         # NEW: 已知问题与待决策项
│   └── modules/
│       └── README.md                     # 模块文档索引（格式约定与维护规则）
└── src-tauri/
    ├── Cargo.toml                        # Rust 项目清单（依赖、版本、feature flags）
    ├── tauri.conf.json                   # Tauri 2 配置文件
    ├── build.rs                          # Tauri 构建脚本
    ├── icons/                            # 应用图标目录（待填充）
    ├── capabilities/
    │   └── default.json                  # Tauri 2 默认权限集（文件/网络/窗口控制/拖拽）
    └── src/
        ├── main.rs                       # Rust 入口点，9 步初始化序列
        ├── lib.rs                        # 库根，公开模块树
        ├── core/
        │   ├── mod.rs                    # core 模块声明
        │   ├── context.rs                # AppContext 统一应用上下文
        │   ├── error.rs                  # AppError 枚举 + user_message() + AppResult<T> + From 转换
        │   ├── entity/
        │   │   ├── mod.rs
        │   │   ├── config.rs            # AppConfig 实体（用户/OJ/编辑器/主题/布局配置 + contest_id）
        │   │   ├── user.rs               # User 实体
        │   │   ├── contest.rs            # Contest + ContestProblem 实体（阶段 7 扩展字段）
        │   │   ├── problem.rs            # Problem + Sample 实体
        │   │   ├── submission.rs         # Submission + JudgementStatus + JudgementResult
        │   │   ├── workspace.rs          # Workspace 核心实体（阶段 3 完善）
        │   │   └── tests/
        │   │       └── workspace_tests.rs     # Workspace 单元测试
        │   ├── provider/
        │   │   ├── mod.rs
        │   │   ├── auth.rs               # AuthProvider trait
        │   │   ├── contest.rs            # ContestProvider trait（+ list_contest_problems）
        │   │   ├── problem.rs            # ProblemProvider trait
        │   │   ├── submission.rs         # SubmissionProvider trait
        │   │   ├── oj_type.rs            # OJType 枚举
        │   │   └── registry.rs           # ProviderRegistry trait
        │   ├── event/
        │   │   ├── mod.rs
        │   │   ├── app_event.rs          # AppEvent + 6 个子事件枚举 + category() 映射
        │   │   ├── event_bus.rs          # EventBus（阶段 3 实现：publish/subscribe/unsubscribe）
        │   │   ├── event_category.rs     # EventCategory 枚举
        │   │   └── tests/
        │   │       └── event_bus_tests.rs     # EventBus 单元测试
        │   └── repository/
        │       ├── mod.rs
        │       ├── workspace_repo.rs     # WorkspaceRepository trait
        │       ├── config_repo.rs        # ConfigRepository trait
        │       └── plugin_repo.rs        # PluginRepository trait
        ├── service/
        │   ├── mod.rs
        │   ├── config/
        │   │   ├── mod.rs                # ConfigService：加载/保存/变更检测/热重载
        │   │   └── error.rs              # ConfigError
        │   ├── theme/
        │   │   ├── mod.rs                # ThemeService：主题切换/配色方案管理
        │   │   └── error.rs              # ThemeError
        │   ├── auth/
        │   │   ├── mod.rs                # AuthService：登录编排/会话持久化/登出/凭证轮换回写
        │   │   ├── error.rs              # AuthError
        │   │   └── tests/
        │   │       └── auth_tests.rs     # AuthService 单元测试（会话持久化/轮换回写/失效清理）
        │   ├── contest/
        │   │   ├── mod.rs                # ContestService：比赛获取/列表缓存/比赛切换
        │   │   └── error.rs              # ContestError
        │   ├── problem/
        │   │   ├── mod.rs                # ProblemService：题目获取/打开题目
        │   │   └── error.rs              # ProblemError
        │   ├── submission/
        │   │   ├── mod.rs                # SubmissionService：代码提交/评测轮询/超时
        │   │   └── error.rs              # SubmissionError
        │   └── workspace/
        │       ├── mod.rs
        │       ├── error.rs              # WorkspaceError
        │       └── manager.rs            # WorkspaceManager：完整生命周期实现
        ├── adapter/
        │   ├── mod.rs
        │   ├── hoj/
        │   │   ├── mod.rs                # HOJAdapter：实现 4 个 Provider trait（阶段 5 完成）
        │   │   ├── types.rs              # HOJ DTO：ApiResponse/Login/Contest/Problem/Submission + 状态码映射
        │   │   └── error.rs              # HOJError
        │   ├── qduoj/
        │   │   ├── mod.rs
        │   │   ├── types.rs              # QDUOJ DTO 类型（骨架）
        │   │   └── error.rs              # QDUOJError
        │   └── hustoj/
        │       ├── mod.rs
        │       ├── types.rs              # HUSTOJ DTO 类型（骨架）
        │       └── error.rs              # HUSTOJError
        ├── infra/
        │   ├── mod.rs
        │   ├── http.rs                   # HttpClient 封装
        │   ├── storage.rs                # Storage 底层文件工具
        │   ├── cache.rs                  # Cache 预留
        │   ├── logger.rs                 # Logger（基于 Tracing）
        │   ├── fs_workspace_repo.rs      # FsWorkspaceRepository（阶段 2 完成）
        │   ├── fs_config_repo.rs         # FsConfigRepository（阶段 2 完成）
        │   ├── fs_plugin_repo.rs         # FsPluginRepository（骨架）
        │   ├── provider_registry_impl.rs # ProviderRegistryImpl
        │   └── tests/
        │       ├── storage_tests.rs      # Storage 单元测试
        │       ├── fs_workspace_repo_tests.rs  # FsWorkspaceRepository 单元测试
        │       └── fs_config_repo_tests.rs     # FsConfigRepository 单元测试
        ├── plugin/
        │   ├── mod.rs
        │   ├── host/
        │   │   ├── mod.rs
        │   │   ├── manifest.rs           # NEW: PluginManifest + PluginPermission
        │   │   └── extension.rs          # NEW: ExtensionPoint + 4 个子扩展点
        │   ├── api/
        │   │   ├── mod.rs
        │   │   ├── workspace.rs
        │   │   ├── problem.rs
        │   │   ├── contest.rs
        │   │   ├── ui.rs
        │   │   └── event.rs
        │   ├── permission/
        │   │   └── mod.rs
        │   ├── registry/
        │   │   └── mod.rs
        │   └── runtime/
        │       └── mod.rs
        └── commands/                     # NEW: Tauri Command 薄封装
            ├── mod.rs                    # register_commands() 入口（含 #[cfg(test)] tests 引用）
            ├── auth_cmd.rs               # login / logout / get_session
            ├── contest_cmd.rs            # list_contests / select_contest / load_configured_contest
            ├── problem_cmd.rs            # get_problem / list_problems
            ├── submission_cmd.rs         # submit_code / get_judgement
            ├── workspace_cmd.rs          # load_workspace / save_workspace / switch_workspace / current_workspace / update_workspace_file
            ├── config_cmd.rs             # get_config / reload_config / update_config
            ├── theme_cmd.rs              # get_theme / set_theme
            └── tests/
                └── mod_tests.rs          # Command 层关键路径测试（P40）
```

---

## 项目根目录（已存在）

```
Hinina/
├── index.html                            # Vite 入口 HTML
├── package.json                          # 前端依赖清单
├── vite.config.ts                        # Vite 构建配置
├── tsconfig.json                         # TypeScript 配置
├── tsconfig.node.json                    # Vite/Node 端 TS 配置
├── .gitignore                            # Git 忽略规则
```

### Vue3 前端 — `src/`（阶段 7 已完成）

```
src/
├── main.ts                               # Vue 应用入口（Pinia + Router + Naive UI）
├── App.vue                               # 根组件（n-config-provider + n-dialog-provider）
├── env.d.ts                              # Vite 环境类型声明
├── router/
│   └── index.ts                          # Vue Router（/login, /contest）
├── views/
│   ├── LoginView.vue                     # 登录页（左右分栏：登录表单 + 几何 SVG 氛围区/比赛简介/倒计时）
│   └── ContestView.vue                   # 核心页面（三栏分割：题目列表｜题面｜编辑器+提交）
├── components/
│   ├── layout/
│   │   ├── TitleBar.vue                  # 窗口标题栏（拖拽区 + 最小化/最大化/关闭，decorations:false）
│   │   └── AppHeader.vue                 # 顶部栏（比赛标题 + 倒计时 + 用户）
│   ├── editor/
│   │   └── CodeEditor.vue                # Monaco Editor 封装（手动 worker 配置）
│   ├── problem/
│   │   ├── ProblemSidebar.vue            # 题目列表侧边栏
│   │   └── ProblemStatement.vue          # 题面展示（Markdown 渲染 + 相对图片 URL 改写）
│   ├── submission/
│   │   └── SubmissionPanel.vue           # 提交记录列表 + 评测状态 Badge
│   └── common/
│       ├── LoadingSpinner.vue            # 通用加载动画
│       └── ErrorMessage.vue              # 通用错误提示 + 重试按钮
├── stores/
│   ├── authStore.ts                      # 用户认证状态
│   ├── contestStore.ts                   # 比赛 + 题目摘要状态
│   ├── problemStore.ts                   # 当前题目详情状态
│   ├── submissionStore.ts                # 提交记录 + 轮询状态
│   └── workspaceStore.ts                 # 工作区 + 代码编辑器状态
├── services/
│   ├── auth.service.ts                   # 登录/登出/会话检查（localStorage 缓存）
│   ├── contest.service.ts                # 加载配置的比赛
│   ├── problem.service.ts                # 获取题目详情/列表
│   ├── submission.service.ts             # 提交代码/轮询评测
│   └── workspace.service.ts              # 工作区创建/保存/恢复
├── bridge/
│   ├── index.ts                          # ipcInvoke 统一封装
│   ├── auth.bridge.ts                    # login / logout / get_session
│   ├── contest.bridge.ts                 # load_configured_contest / list_contests（匿名，登录页比赛信息）
│   ├── problem.bridge.ts                 # get_problem / list_problems
│   ├── submission.bridge.ts              # submit_code / get_judgement
│   ├── workspace.bridge.ts               # load_workspace / save_workspace / current_workspace / updateWorkspaceFile
│   └── config.bridge.ts                  # get_config
├── types/
│   ├── user.ts                           # User 实体
│   ├── contest.ts                        # Contest + ContestProblem 实体
│   ├── problem.ts                        # Problem + Sample 实体
│   ├── submission.ts                     # JudgementStatus + JudgementResult
│   ├── workspace.ts                      # Workspace 实体
│   └── config.ts                         # AppConfig 及其子配置
├── utils/
│   └── markdown.ts                       # Markdown 渲染（marked）+ 相对图片 URL 改写为 HOJ 绝对地址
└── styles/
    └── global.css                        # TailwindCSS + CSS 变量（电光紫主题 #7C5CFF）+ 暗色主题
```

---

## 架构概览

```
┌─────────────────────────────────────────────────────────┐
│                    Vue3 前端 (src/)                       │
│  View ──→ Store (Pinia) ──→ Service ──→ Bridge (IPC)     │
└─────────────────────────┬───────────────────────────────┘
                          │  Tauri IPC (invoke)
┌─────────────────────────┴───────────────────────────────┐
│                   Rust 后端 (src-tauri/)                  │
│  Service ←── Provider (trait) ←── Adapter (HOJ/QDUOJ/…) │
│     │              │                                     │
│  Entity        EventBus        Infra (http/storage/…)   │
│     │              │                                     │
│  Workspace     AppEvent         Plugin (v0.x 预留)       │
└─────────────────────────────────────────────────────────┘
```

### 分层职责速查

| 层 | Rust 端 | Vue 端 |
|----|---------|--------|
| **领域** | `core/entity` + `core/provider` traits | `src/types/` 类型定义 |
| **应用** | `service/` 业务编排 | `src/services/` 业务逻辑 |
| **适配** | `adapter/` OJ 实现 | `src/bridge/` IPC 封装 |
| **基础设施** | `infra/` http/storage/cache/logger | Vite/Naive UI/TailwindCSS |
| **表现** | — | `views/` + `components/` |
| **状态** | EventBus | `stores/` Pinia |
| **扩展** | `plugin/`（v0.x 仅预留接口） | — |

### 关键设计约束

- **Workspace First**：Workspace 是核心领域对象，负责代码存储、自动保存、崩溃恢复、比赛隔离
- **Provider trait 拆分**：禁止单一巨型 trait，按 Auth/Contest/Problem/Submission 独立定义
- **EventBus 原则**：查询与命令走 Service，状态变更走 EventBus。禁止所有逻辑事件化
- **插件系统**：v0.x 仅预留架构，不实现运行时。插件只能访问 `plugin/api/`，禁止直接调用内部 Service
- **无 SQL 数据库**：纯文件存储，不引入 SQLite 等数据库依赖
- **前端分层**：View → Store → Service → Bridge，Store 不放业务逻辑与网络请求
