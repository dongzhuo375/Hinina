# Hinina 项目架构与文件树

> 最后更新：2026-09-15 | 分支：`feat/polish-ui`
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
        │   ├── error.rs                  # AppError 枚举 + user_message() + context()（补环节名但保留变体）+ AppResult<T> + From 转换
        │   ├── tests/
        │   │   └── error_tests.rs        # AppError::context 测试（变体绝不被改写，前端 isAuthError 分流依赖它）
        │   ├── entity/
        │   │   ├── mod.rs
        │   │   ├── config.rs            # AppConfig 实体（用户/OJ/编辑器/主题/布局配置 + contest_id）
        │   │   ├── user.rs               # User 实体
        │   │   ├── contest.rs            # Contest（+ rank_show_name/seal_rank/seal_rank_time/allow_end_submit）+ ContestProblem（+ color 气球色）
        │   │   ├── problem.rs            # Problem + Sample 实体
        │   │   ├── submission.rs         # Submission + JudgementStatus + JudgementResult
        │   │   ├── rank.rs               # 榜单实体：RankCell / ContestRankRow / ContestRankPage / RankQuery / ProblemLimits（ACM 与 OI 两套 VO 在 Adapter 归一到此）
        │   │   ├── workspace.rs          # Workspace 核心实体（阶段 3 完善）
        │   │   └── tests/
        │   │       └── workspace_tests.rs     # Workspace 单元测试
        │   ├── provider/
        │   │   ├── mod.rs
        │   │   ├── auth.rs               # AuthProvider trait
        │   │   ├── contest.rs            # ContestProvider trait（+ list_contest_problems / get_contest_rank）
        │   │   ├── problem.rs            # ProblemProvider trait（+ get_user_problem_status）
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
        │   │   ├── mod.rs                # AuthService：登录编排/会话持久化/登出/凭证轮换回写/三态会话校验（SessionValidity）
        │   │   ├── error.rs              # AuthError
        │   │   └── tests/
        │   │       └── auth_tests.rs     # AuthService 单元测试（会话持久化/轮换回写/失效清理）
        │   ├── contest/
        │   │   ├── mod.rs                # ContestService：比赛获取/列表缓存/比赛切换/get_rank（榜单不缓存，内榜每次实时重算）
        │   │   ├── error.rs              # ContestError
        │   │   └── tests/
        │   │       └── contest_tests.rs  # ContestService 单元测试（错误变体穿透：get_rank/list_contests/load_contest_with_problems）
        │   ├── problem/
        │   │   ├── mod.rs                # ProblemService：题目获取/打开题目/我的题目状态/load_problem_limits（内存+磁盘双层缓存、并发上限 4、部分失败跳过）
        │   │   ├── error.rs              # ProblemError
        │   │   └── tests/
        │   │       └── problem_tests.rs  # limits 缓存测试（首次落盘/二次命中零请求/损坏文件降级/401 不回退默认值）
        │   ├── submission/
        │   │   ├── mod.rs                # SubmissionService：代码提交/评测轮询/超时（认证错误立即上抛，不重试到超时）
        │   │   ├── error.rs              # SubmissionError
        │   │   └── tests/
        │   │       └── submission_tests.rs  # SubmissionService 单元测试（变体穿透、认证错误短路、瞬时抖动仍重试、超时语义）
        │   └── workspace/
        │       ├── mod.rs
        │       ├── error.rs              # WorkspaceError
        │       ├── manager.rs            # WorkspaceManager：完整生命周期 + set_language/persist_meta（语言等元数据随保存落盘）
        │       └── tests/
        │           └── manager_tests.rs  # 工作区生命周期测试（含语言跨实例持久化）
        ├── adapter/
        │   ├── mod.rs
        │   ├── hoj/
        │   │   ├── mod.rs                # HOJAdapter：实现 4 个 Provider trait + get/post_json_authed（共用 handle_token_rotation 做 Refresh-Token 轮换）
        │   │   │                         #   + parse_hoj_json（全部响应的唯一解析入口：去 null → 识别体内鉴权失败 → 类型化解析）
        │   │   │                         #   + session_validity_from_response（会话三态判据，网络异常绝不可折成「已失效」）
        │   │   ├── types.rs              # HOJ DTO：ApiResponse/Login/Contest/Problem/Submission/ContestRank(ACM+OI)/UserProblemStatus + 状态码映射 + strip_nulls
        │   │   ├── error.rs              # HOJError
        │   │   └── tests/
        │   │       ├── mod_tests.rs      # Adapter 行为测试（token 轮换、parse_cid、parse_hoj_json、体内鉴权失败判定、会话三态判据、preview）
        │   │       ├── types_tests.rs    # DTO 解析测试（榜单 ACM/OI 归一、封榜只有 tryNum、打星 rank=-1、字段缺失与 null 容错）
        │   │       └── fixtures/
        │   │           └── contest_list_anon.json  # 真实 get-contest-list 响应（已脱敏），锁定 null 容错回归
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
        │   ├── http.rs                   # HttpClient 封装（超时/重试/UA/Cookie；只返回原始响应体与响应头，不做反序列化）
        │   ├── storage.rs                # Storage 底层文件工具
        │   ├── cache.rs                  # Cache 预留
        │   ├── logger.rs                 # Logger（基于 Tracing）
        │   ├── fs_workspace_repo.rs      # FsWorkspaceRepository（阶段 2 完成）
        │   ├── fs_config_repo.rs         # FsConfigRepository（阶段 2 完成）
        │   ├── fs_plugin_repo.rs         # FsPluginRepository（骨架）
        │   ├── provider_registry_impl.rs # ProviderRegistryImpl
        │   └── tests/
        │       ├── storage_tests.rs      # Storage 单元测试
        │       ├── http_tests.rs         # HttpClient 单元测试（401→Auth、403/5xx→Network、退避延迟、classify_status 重试判据）
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
            ├── auth_cmd.rs               # login / logout / get_session / validate_session（三态）
            ├── contest_cmd.rs            # list_contests / select_contest / load_configured_contest / get_contest_rank
            ├── problem_cmd.rs            # get_problem / list_problems / get_user_problem_status / get_contest_problem_limits
            ├── submission_cmd.rs         # submit_code / get_judgement
            ├── workspace_cmd.rs          # load_workspace / save_workspace / switch_workspace / current_workspace / update_workspace_file / set_workspace_language
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
├── main.ts                               # Vue 应用入口（Pinia + Router + Naive UI + 全局会话守卫装配）
├── App.vue                               # 根组件（n-config-provider + n-dialog-provider）
├── env.d.ts                              # Vite 环境类型声明
├── router/
│   └── index.ts                          # Vue Router（/login + /contest 嵌套子树：problems / problem/:displayId / rank / submissions / announcements / settings）+ requiresAuth 认证守卫
├── views/
│   ├── LoginView.vue                     # 登录页（左右分栏：登录表单/已登录身份块 + 几何 SVG 氛围区/比赛简介/倒计时）
│   │                                     #   已登录且比赛未开始时留在本页等待，倒计时归零自动进入赛场
│   ├── ContestLayout.vue                 # 比赛工作台外壳：TopBar + ActivityBar + <router-view> + StatusBar
│   ├── ProblemSetView.vue                # 题目总览（统计条 + 卡片网格，limits 渐进填充）
│   ├── ProblemSolveView.vue              # 解题页（题面分节 ｜ 编辑器 + 控制台条，可拖拽分栏）
│   ├── RankView.vue                      # 实时榜单（工具条 + 表格 + 分页，10s±2s 轮询、后台暂停、结束即停）
│   └── PlaceholderView.vue               # 「功能开发中」占位页（评测 / 公告 / 设置共用）
├── components/
│   ├── layout/
│   │   ├── TopBar.vue                    # 顶栏（拖拽区 + 窗口控制 + 状态徽章 + 倒计时胶囊 + 比赛简介抽屉 + 用户 pill）
│   │   ├── ActivityBar.vue               # 左侧活动栏（题目/榜单/评测/公告 + 底部设置，router-link 驱动高亮）
│   │   └── StatusBar.vue                 # 底部状态条（连接状态 + 客户端版本）
│   ├── contest/
│   │   └── ContestStatsBar.vue           # 统计卡（解题进度 / 实时排名 / 总罚时，数据源=榜单我的行）
│   ├── editor/
│   │   ├── CodeEditor.vue                # Monaco Editor 封装（浅色主题 + 语言工具条 + 自动备份指示 + focus()）
│   │   └── EditorConsoleBar.vue          # 编辑器底部控制台条（最新记录 pill / 提交记录入口 / 提交代码 / 光标状态行）
│   ├── problem/
│   │   ├── ProblemCard.vue               # 题目卡片（字母徽章取 HOJ 气球色、limits、通过数、我的状态、快捷入口）
│   │   ├── ProblemTabStrip.vue           # 题目快速切换条（A/B/C… chips，标记已 AC / 已尝试）
│   │   └── ProblemStatement.vue          # 题面分节展示（描述/输入/输出/样例+复制/提示，Markdown 渲染 + 相对图片 URL 改写）
│   ├── rank/
│   │   ├── RankToolbar.vue               # 榜单工具条（服务端 keyword 搜索 300ms 防抖 + 分组 tab + 赛制图例）
│   │   ├── ScoreboardTable.vue           # 榜单表格（粘性表头/我的行/前两列，ACM 与 OI 分流渲染）
│   │   └── RankCell.vue                  # ACM 单元格（一血/通过/未通过/封榜/赛后提交/未作答）
│   └── common/
│       ├── LoadingSpinner.vue            # 通用加载动画
│       └── ErrorMessage.vue              # 通用错误提示 + 重试按钮
├── stores/
│   ├── authStore.ts                      # 用户认证状态（登录/登出/会话恢复/三态校验 + sessionResolved 守卫标记）
│   ├── session.ts                        # 会话级领域状态清理（登出/切换账号时重置比赛/题目/提交/榜单/工作区并回收定时器）
│   ├── sessionGuard.ts                   # 全局会话守卫（认证类 IPC 失败 → 判定失效 → 清理并回登录页），由 main.ts 装配
│   ├── contestStore.ts                   # 比赛 + 题目摘要状态 + loadContest 并发去重 + 登录页匿名比赛简报状态（brief*）
│   ├── problemStore.ts                   # 当前题目详情 + limits 缓存 + 我的题目状态（limitsOf/statusOf 派生读取）
│   ├── rankStore.ts                      # 榜单状态与轮询编排（uid 去重、参与人数修正、分组筛选、我的行、后台暂停）
│   ├── submissionStore.ts                # 提交记录 + 评测轮询编排（终态/超时停止，登出时统一回收定时器）
│   ├── workspaceStore.ts                 # 工作区 + 代码编辑器状态（语言切换即时持久化到后端元数据）
│   └── __tests__/                        # authStore / contestStore / rankStore .spec.ts（会话状态机、加载去重、榜单去重与轮询）
├── services/
│   ├── auth.service.ts                   # 登录/登出/会话检查/三态会话校验（localStorage 缓存，登出失败也清本地）
│   ├── config.service.ts                 # 配置读取唯一入口（进程内缓存）+ 派生参数（OJ 基址、轮询调度）
│   ├── contest.service.ts                # 加载配置的比赛 + 登录页匿名比赛简报编排（config → contestId → 列表筛选）
│   ├── problem.service.ts                # 获取题目详情/列表 + 我的题目状态 + 批量 limits
│   ├── rank.service.ts                   # 榜单查询（分页/关键词/去打星参数编排）
│   ├── submission.service.ts             # 提交代码/轮询评测
│   ├── workspace.service.ts              # 工作区创建/保存/恢复/语言持久化
│   └── __tests__/                        # auth.service.spec.ts（本地缓存清理与三态归一契约）
├── bridge/
│   ├── index.ts                          # ipcInvoke 统一封装 + IpcError（AppError 载荷归一化为 Error，单点日志且不记录参数）
│   ├── auth.bridge.ts                    # login / logout / get_session / validate_session
│   ├── contest.bridge.ts                 # load_configured_contest / list_contests（匿名，登录页比赛信息）
│   ├── problem.bridge.ts                 # get_problem / list_problems / getUserProblemStatus / getContestProblemLimits
│   ├── rank.bridge.ts                    # get_contest_rank
│   ├── submission.bridge.ts              # submit_code / get_judgement
│   ├── workspace.bridge.ts               # load_workspace / save_workspace / current_workspace / updateWorkspaceFile / setWorkspaceLanguage
│   ├── config.bridge.ts                  # get_config
│   └── __tests__/                        # index.spec.ts（AppError → IpcError 跨端契约、日志不泄露参数）
├── types/
│   ├── user.ts                           # User 实体 + SessionValidity（valid/invalid/unknown 三态）
│   ├── contest.ts                        # Contest + ContestProblem 实体
│   ├── problem.ts                        # Problem + Sample 实体
│   ├── submission.ts                     # JudgementStatus + JudgementResult
│   ├── rank.ts                           # RankCell / ContestRankRow / ContestRankPage / RankQuery / ProblemLimits（榜单与题目限制跨端契约）
│   ├── workspace.ts                      # Workspace 实体
│   └── config.ts                         # AppConfig 及其子配置
├── utils/
│   ├── markdown.ts                       # Markdown 渲染（marked）+ 相对图片 URL 改写为 HOJ 绝对地址
│   ├── contest.ts                        # 比赛阶段推导纯函数（getContestPhase / hasContestStarted，登录页与顶部栏共用）
│   ├── submission.ts                     # 评测终态判据（isTerminalStatus，与 Rust is_terminal_status 对齐）
│   ├── polling.ts                        # 轮询原语（planPollDelayMs 抖动错峰 + createPoller 递归 setTimeout：重入保护/可暂停/定时器可注入）
│   ├── rank.ts                           # 榜单渲染纯映射（单元格文案与样式类别、显示名回退、uid 去重、参与人数推算、罚时格式化）
│   ├── limits.ts                         # 题目时限/内存格式化与语言倍率换算（C/C++ 1 倍，其它语言 ×2）
│   └── __tests__/                        # contest / submission / session-check / polling / rank / limits .spec.ts（阶段判据、终态判据、预检调度边界、轮询节奏、榜单映射、limits 换算）
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
- **前端会话与导航**：应用入口统一为 `/login`；`router.beforeEach` 在首次导航时恢复会话，并拦截 `meta.requiresAuth` 路由（未登录一律回登录页）；登录页依据比赛阶段（`utils/contest`）决定是否进入赛场 —— 比赛未开始时留在登录页等待，倒计时归零后自动进入；登出时经 `stores/session.ts` 清空会话级领域状态
- **会话失效处理**：三态校验（`valid` / `invalid` / `unknown`）贯穿 Adapter → `AuthService::validate_session` → command → 前端 store；`unknown`（网络异常）一律**保留**登录态并重试，只有服务端明确判定失效才清理会话回登录页 —— 赛前误踢选手的代价远高于多等一轮校验，且反复重登可能触发 HOJ 暴力破解锁定（同 IP + 同用户名 30 分钟 20 次）。全局兜底由 `stores/sessionGuard.ts` 承担：任何认证类 IPC 失败即判定失效（在组合根注入观察者，Bridge 不感知 store/router）
- **三态契约必须由 Adapter 兑现**：`AuthProvider::validate_session` 的返回值语义是 `Ok(true)` 有效 / `Ok(false)` 服务端**明确**判定失效 / `Err(_)` 无法判定。**绝不可把网络错误折成 `Ok(false)`** —— 那会让 `SessionValidity::Unknown` 分支成为死代码，一次赛前网络抖动就把选手踢回登录页。HOJ 侧的判据抽成纯函数 `session_validity_from_response` 以便测试锁定：仅 `AppError::Auth` 算明确失效，非 200 的其它状态码（400/500）归 `Unknown`，网络/超时/解析失败一律上抛。同理，长轮询（`SubmissionService::poll_judgement`，默认超时 300s）**不得吞掉认证错误**：必须立即上抛，否则选手干等五分钟后只收到「评测超时」，守卫也拿不到 Auth 变体
- **赛前预检错峰**：登录页等待开赛时按 `utils/session-check.ts` 的策略校验会话 —— 距开赛 >10min 每 5min±60s 周期复检，进入 [T-10min, T-3min] 窗口后在剩余区间随机取点做一次性预检，迟到启动则 0–3s 抖动后立即执行，距开赛 ≤30s 不再预检。目的是把全场客户端的校验请求散布开，避免开赛前形成同步尖峰；**进场（T-0 导航）不错峰**，准点进场是公平性要求
- **IPC 错误归一化**：Rust `AppError` 经 serde 序列化为 `{ Variant: msg }` 对象，`bridge/index.ts` 在唯一出口转换为 `IpcError extends Error`，保证上层 `e instanceof Error` 与 `e.message` 可用；日志不记录调用参数（含明文密码）
- **错误变体是分流依据，后端不得改写**：前端 `isAuthError`（`variant === 'Auth'`）与 `stores/sessionGuard.ts` 的会话失效兜底完全依赖变体。补上下文一律用 `AppError::context()`（保留变体，只在消息前拼环节名），**禁止** `AppError::Network(format!("xx 请求失败: {}", e))` 这类重新包装 —— 它会把反序列化失败、认证失败一律改写成「网络错误」，现场看到「网络错误: … 序列化错误: …」自相矛盾的嵌套消息，把 DTO 问题当断网查，还会让 401 不再触发登出。**Service 层传播 Provider 错误同样适用此约定**（`contest` / `problem` / `submission` / `auth` 全部用 `e.context("…")`）：`get_rank` 是全场最高频的认证调用（每 10s 一次），变体被改写会让 token 过期时榜单静默 stale、提交只弹一条文案、选手永远回不到登录页
- **OJ 响应解析归 Adapter，infra 只传字节**：`infra/http.rs` 只返回原始响应体与响应头（含状态码判定与 5xx 退避重试），不做反序列化；OJ 特有的响应归一化在 Adapter 的唯一入口完成。HOJ 侧有两个必须处理的协议事实：① 对未设置字段返回 `null` 而非省略（实测 `get-contest-list` 的 `sealRank`/`rankShowName`/`count`/`now` 全为 null），而 serde 的 `#[serde(default)]` **只在字段缺失时生效**，显式 null 会让整个响应解析失败 → 解析前统一 `strip_nulls`（`false`/`0`/`""` 不是 null，必须保留，否则封榜、打星、零分语义会被抹掉）；② 鉴权失败放在**响应体的 status**（HTTP 仍是 200，实测匿名访问 `get-contest-problem` 返回 `{"status":403,"msg":"请您先登录！"}`）→ 必须翻译成 `AppError::Auth`，且 403 要保守判定（仅当消息指向登录/凭证时才算会话失效，否则「私有赛未注册」会把已登录选手误踢回登录页）
- **HTTP 401 由 infra 映射为 `Auth` 变体**：401 的标准语义就是「未认证」，属 HTTP 通用语义而非 OJ 私有约定，故由 `infra/http.rs` 的 `status_error` 承担；**403 保持 `Network`**（可能是业务性无权访问）。这条映射是会话校验能成立的前提 —— `get_json_authed` 遇到 401 时若仍归为 `Network`，`session_validity_from_response` 会把它当「无法判定」上抛，导致 token 真正过期时反而永不登出。实测 HOJ 两种报法都存在：`get-user-auth-info` 走 HTTP 401，`get-contest-problem` 走 HTTP 200 + 体内 403，两条路径都必须认- **真实响应夹具**：`adapter/hoj/tests/fixtures/contest_list_anon.json` 取自真实接口、仅脱敏自由文本，完整保留键名与 null 分布；配套一条正向测试（真实响应可解析）与一条反向测试（不去 null 必然失败），防止后来者把 `strip_nulls` 当冗余删掉
- **配置与轮询归属**：配置读取统一经 `services/config.service.ts`（进程内缓存 + 兜底），View/Store 不得直接调用 `config.bridge`；评测轮询定时器由 `submissionStore` 编排（终态判据见 `utils/submission.ts`，超时兜底），View 只表达提交意图
- **比赛工作台外壳**：`ContestLayout` 承载 TopBar + ActivityBar + `<router-view>` + StatusBar，各功能页是平级路由而非单页三栏；窗口拖拽与窗口控制只在 TopBar（登录页由 `App.vue` 提供兜底窗口条）。View 与 component **禁止**直接 import `@/bridge`（分层判据，可 grep 断言）
- **轮询统一原语**：周期性刷新（榜单、题目总览）走 `utils/polling.ts` 的 `createPoller`（递归 setTimeout + 抖动 + 重入保护 + `document.hidden` 暂停），定时器句柄由 store 持有（模块级普通变量，不进 `ref/reactive`），离开路由或比赛结束（`status == 1`）必须停止；榜单 10s±2s，题目总览 30s±5s。提交结果轮询是**按提交 ID 的一次性收敛轮询**（终态判据 + 总超时，见 `utils/submission.ts`），仍由 `submissionStore` 自行编排，迁移到统一原语留待后续
- **榜单数据源职责**：卡片「我的状态」取 `get-user-problem-status`（轻量、不受榜单分页/搜索影响）；统计卡「解题进度 / 实时排名 / 总罚时」取榜单我的行（服务端前置复制，天然可得）；`ac/total` 与气球色取比赛题目列表。HOJ 会把当前用户与关注列表**前置复制**进 `records`，渲染前必须按 `uid` 去重，`total` 因此偏大、不能直接当参赛人数（用 `total - 本页重复数`）；`rank == -1` 是打星队伍；封榜以 `contest.sealRank + sealRankTime` 自行判断，**不依赖 `forceRefresh`**（对非管理员无效）
- **ACM / OI 归一**：两套 VO（`ac/total/submissionInfo{对象}` vs `totalScore/submissionInfo{分数}/timeInfo{毫秒}`）在 Adapter 层归一为 OJ 无关实体，前端不感知赛制差异；单元格判档是纯函数（`utils/rank.resolveRankCell` / `resolveOiRankCell`），组件只做样式映射。ACM `totalTime` 是**秒**、OI 是**毫秒**，混用会差 1000 倍
- **题目 limits 缓存**：列表接口不返回 limits，只能按题请求 `get-contest-problem-details`；`ProblemService::load_problem_limits` 做「内存 + 磁盘（`cache/problem_limits/{cid}.json`）」双层缓存、并发上限 4、部分失败跳过、全部失败才上抛；401/403 **不得静默回退默认值**（未注册私有赛必须让选手看见真因）。展示需标注语言倍率（题面是 C/C++ 基准，其它语言时间与内存 ×2）
- **状态文案以接口返回为准**：评测状态直接用后端 `JudgementStatus` 原词（Accepted / Wrong Answer…），不强行缩写为 AC/WA；`get-user-problem-status` 的 0/1/2 映射为「未作答 / 已通过 / 尝试过」
- **工作区语言必须落盘**：语言不属于任何代码文件，`update_workspace_file` 带不上它；`workspaceStore.changeLanguage` 乐观更新本地并调用 `set_workspace_language` 立即持久化元数据，否则切题或重启后退回默认语言，会把 Java 代码当 C++ 提交
- **离线客户端约束**：不引入外部字体与图标字体（设计稿的 Google Fonts / Material Symbols 一律改内联 SVG），不为此新增 npm 依赖；本轮只做浅色主题（Monaco `vs`）
