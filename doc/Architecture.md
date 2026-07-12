# Hinina 项目架构与文件树

> 最后更新：2026-07-12 | 分支：`feat/stage4-service`
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
    │   └── default.json                  # Tauri 2 默认权限集
    └── src/
        ├── main.rs                       # Rust 入口点，9 步初始化序列
        ├── lib.rs                        # 库根，公开模块树
        ├── core/
        │   ├── mod.rs                    # core 模块声明
        │   ├── context.rs                # AppContext 统一应用上下文
        │   ├── error.rs                  # AppError 枚举 + user_message() + AppResult<T> + From 转换
        │   ├── entity/
        │   │   ├── mod.rs
        │         │   ├── config.rs            # AppConfig 实体（用户/OJ/编辑器/主题/布局配置）
        │         │   ├── user.rs               # User 实体
        │   │   ├── contest.rs            # Contest 实体
        │   │   ├── problem.rs            # Problem + Sample 实体
        │   │   ├── submission.rs         # Submission + JudgementStatus + JudgementResult
        │   │   ├── workspace.rs          # Workspace 核心实体（阶段 3 完善）
        │   │   └── tests/
        │   │       └── workspace_tests.rs     # Workspace 单元测试
        │   ├── provider/
        │   │   ├── mod.rs
        │   │   ├── auth.rs               # AuthProvider trait
        │   │   ├── contest.rs            # ContestProvider trait
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
        │   │   ├── mod.rs                # AuthService：登录编排/会话持久化/登出
        │   │   └── error.rs              # AuthError
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
        │   │   ├── mod.rs
        │   │   ├── types.rs              # HOJ DTO 类型（骨架）
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
            ├── mod.rs                    # register_commands() 入口
            ├── auth_cmd.rs               # login / logout / get_session
            ├── contest_cmd.rs            # list_contests / select_contest
            ├── problem_cmd.rs            # get_problem / list_problems
            ├── submission_cmd.rs         # submit_code / get_judgement
            ├── workspace_cmd.rs          # load / save / switch / current
            ├── config_cmd.rs             # get_config / update_config
            └── theme_cmd.rs              # get_theme / set_theme
```

---

## 项目根目录（需新建的配置文件）

```
Hinina/
├── index.html                            # Vite 入口 HTML
├── package.json                          # 前端依赖清单
├── vite.config.ts                        # Vite 构建配置
├── tsconfig.json                         # TypeScript 配置
├── tsconfig.node.json                    # Vite/Node 端 TS 配置
├── tailwind.config.js                    # TailwindCSS 配置
├── postcss.config.js                     # PostCSS 配置
├── .gitignore                            # Git 忽略规则
├── .prettierrc                           # Prettier 配置
└── .eslintrc.cjs                         # ESLint 配置
```

### Vue3 前端 — `src/`（待创建）

```
src/
├── main.ts                               # Vue 应用入口
├── App.vue                               # 根组件
├── env.d.ts                              # Vite 环境类型声明
├── router/
│   └── index.ts                          # Vue Router
├── views/
│   ├── LoginView.vue
│   ├── ContestListView.vue
│   ├── ContestDetailView.vue
│   ├── ProblemView.vue                   # 核心页面（分栏布局）
│   └── SubmissionView.vue
├── components/
│   ├── layout/
│   │   ├── AppHeader.vue
│   │   ├── AppSidebar.vue
│   │   └── AppStatusBar.vue
│   ├── editor/
│   │   ├── CodeEditor.vue                # Monaco Editor 封装
│   │   └── EditorToolbar.vue
│   ├── problem/
│   │   ├── ProblemStatement.vue
│   │   └── ProblemSamples.vue
│   ├── submission/
│   │   ├── SubmissionList.vue
│   │   └── JudgementResult.vue
│   ├── contest/
│   │   └── ContestCard.vue
│   └── common/
│       ├── LoadingSpinner.vue
│       └── ErrorMessage.vue
├── stores/
│   ├── authStore.ts
│   ├── contestStore.ts
│   ├── problemStore.ts
│   ├── submissionStore.ts
│   └── workspaceStore.ts
├── services/
│   ├── auth.service.ts
│   ├── contest.service.ts
│   ├── problem.service.ts
│   ├── submission.service.ts
│   └── workspace.service.ts
├── bridge/
│   ├── index.ts
│   ├── auth.bridge.ts
│   ├── contest.bridge.ts
│   ├── problem.bridge.ts
│   ├── submission.bridge.ts
│   └── workspace.bridge.ts
├── types/
│   ├── user.ts
│   ├── contest.ts
│   ├── problem.ts
│   ├── submission.ts
│   └── workspace.ts
└── styles/
    ├── variables.css
    └── global.css
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
