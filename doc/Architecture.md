# Hinina 项目架构与文件树

> 最后更新：2026-09-15 | 分支：`feat/judging-announcements-settings`
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
│   ├── todo.md                            # 开发路线图（8 阶段执行顺序）
│   ├── problem.md                         # 已知问题与待决策项（本地 Review 文档，不入库）
│   ├── HOJ/                              # HOJ API 文档（API 总览 / 榜单 / 题目 limits）
│   ├── screen/                           # 界面设计稿（login / problem_set / problem_solve / rank / submissios / notice，各含 DESIGN.md + code.html + screen.png）
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
        ├── main.rs                       # Rust 入口点，9 步初始化序列 + setup 装配工作区落盘事件桥（Saved/AutoSaveTriggered → 前端 workspace-saved）
        ├── lib.rs                        # 库根，公开模块树
        ├── core/
        │   ├── mod.rs                    # core 模块声明
        │   ├── context.rs                # AppContext 统一应用上下文
        │   ├── error.rs                  # AppError 枚举 + user_message() + context()（补环节名但保留变体）+ AppResult<T> + From 转换
        │   ├── tests/
        │   │   └── error_tests.rs        # AppError::context 测试（变体绝不被改写，前端 isAuthError 分流依赖它）
        │   ├── entity/
        │   │   ├── mod.rs
        │   │   ├── config.rs            # AppConfig 实体（用户/OJ 实例清单(active+instances+contest_ref)/编辑器/主题/布局配置）+ normalize_legacy_values（旧值一次性归一：C++→cpp、dark→light、0.45→0.48、hojUrl/contestId/lastOjType→instances/contestRef/active）
        │   │   ├── user.rs               # User 实体
        │   │   ├── contest.rs            # Contest（+ rank_show_name/seal_rank/seal_rank_time/allow_end_submit/oi_rank_score_type）+ ContestProblem（+ color 气球色）
        │   │   ├── problem.rs            # Problem（+ languages 本题允许提交语言，HOJ 显示名）+ Sample 实体
        │   │   ├── announcement.rs       # Announcement + AnnouncementPage（公告实体，时间为 epoch 秒；已读状态是客户端特性，见 service/contest）
        │   │   ├── submission.rs         # JudgementStatus（HOJ 全状态码 0-15；is_terminal 核心层终态判据，三处对齐）+ JudgementResult + SubmissionRecord/Page/Query/Detail + JudgeCase/SubTaskCases/SubmissionCases
        │   │   ├── rank.rs               # 榜单实体：RankCell / ContestRankRow / ContestRankPage / RankQuery / ProblemLimits（ACM 与 OI 两套 VO 在 Adapter 归一到此）
        │   │   ├── workspace.rs          # Workspace 核心实体（阶段 3 完善）
        │   │   └── tests/
        │   │       ├── workspace_tests.rs     # Workspace 单元测试
        │   │       └── config_tests.rs        # normalize_legacy_values 归一化测试（旧默认值才归一，用户自设值不动）
        │   ├── provider/
        │   │   ├── mod.rs
        │   │   ├── auth.rs               # AuthProvider trait
        │   │   ├── contest.rs            # ContestProvider trait（+ list_contest_problems / get_contest_rank / list_announcements）
        │   │   ├── problem.rs            # ProblemProvider trait（+ get_user_problem_status）
        │   │   ├── submission.rs         # SubmissionProvider trait（+ list_contest_submissions / get_submission_detail / get_submission_cases）
        │   │   ├── oj_id.rs             # OjId newtype（OJ 身份 = 数据而非枚举；session_file() 显式会话文件名契约，id 与历史枚举 Debug 输出一致）
        │   │   └── registry.rs           # ProviderRegistry trait + ProviderSet（注册侧聚合 Option×4；查询侧按能力 current_xxx()，不提供聚合 current()）
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
        │   │   ├── mod.rs                # ConfigService：加载/保存/变更检测/热重载（加载路径统一过 normalize_legacy_values）
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
        │   │   ├── mod.rs                # ContestService：比赛获取/列表缓存（TTL 来自配置）/比赛元信息缓存（内存+磁盘，固定 TTL 120s，题面总览页轮询请求减半）/比赛切换/get_rank（榜单不缓存）/list_announcements（公告不缓存）+ 公告已读状态持久化（announcements_read/{cid}_{uid}.json，合并去重、损坏降级为空+warn）
        │   │   ├── error.rs              # ContestError
        │   │   └── tests/
        │   │       └── contest_tests.rs  # ContestService 单元测试（错误变体穿透 + TTL 缓存语义：命中零请求/过期重取/refresh 强制/失败不留 stale + 元信息缓存：命中跳过 get_contest 而题目列表仍实时/磁盘跨实例命中/按比赛隔离/refresh 清两层/错误不入缓存 + 公告已读读写与损坏降级）
        │   ├── problem/
        │   │   ├── mod.rs                # ProblemService：题目获取/打开题目（题面内存+磁盘缓存，TTL 30min，受 oj.cacheProblemStatement 开关控制）/我的题目状态/load_problem_limits（内存+磁盘双层缓存、并发上限 4、部分失败跳过）
        │   │   ├── error.rs              # ProblemError
        │   │   └── tests/
        │   │       └── problem_tests.rs  # limits 与题面缓存测试（首次落盘/二次命中零请求/开关关闭直连且不落盘/跨实例命中/键隔离/错误不入缓存/401 不回退默认值）
        │   ├── submission/
        │   │   ├── mod.rs                # SubmissionService：代码提交/评测轮询/超时（认证错误立即上抛）+ 提交历史/详情/测试点查询（列表不缓存；终态详情与测试点走仅内存 TTL 缓存，评测中永不缓存；clear_user_caches 由登出编排）
        │   │   ├── error.rs              # SubmissionError
        │   │   └── tests/
        │   │       └── submission_tests.rs  # SubmissionService 单元测试（变体穿透、认证错误短路、瞬时抖动仍重试、超时语义、历史/详情/测试点穿透）
        │   └── workspace/
        │       ├── mod.rs
        │       ├── error.rs              # WorkspaceError
        │       ├── manager.rs            # WorkspaceManager：完整生命周期 + set_language/persist_meta（语言等元数据随保存落盘）；落盘语义=debounce-to-memory（update_file 只写内存，落盘仅经 save() 与 auto-save；替换 current 前先落盘旧的；auto-save 以修订号判定能否清脏）
        │       └── tests/
        │           └── manager_tests.rs  # 工作区生命周期测试（含语言跨实例持久化）
        ├── adapter/
        │   ├── mod.rs                    # AdapterDeps（仅 infra 依赖，禁止塞 Service）+ AdapterFactory{id, build} + factories() 内建清单（接入新 OJ = 新子目录 + 此处一行）
        │   ├── tests/
        │   │   └── adapter_tests.rs      # 工厂防线测试（id 唯一 + 全部可构建 + 四能力齐备 + HOJ 会话文件名契约）
        │   ├── hoj/
        │   │   ├── mod.rs                # HOJAdapter：ID 常量 + FACTORY 工厂 + 实现 4 个 Provider trait + get/post_json_authed（共用 handle_token_rotation 做 Refresh-Token 轮换）
        │   │   │                         #   + parse_hoj_json（全部响应的唯一解析入口：去 null → 识别体内鉴权失败 → 类型化解析）
        │   │   │                         #   + session_validity_from_response（会话三态判据，网络异常绝不可折成「已失效」）
        │   │   │                         #   + 公告/提交历史/提交详情/测试点四端点（get-contest-announcement、contest-submissions、get-submission-detail 完整映射、get-all-case-result）
        │   │   ├── types.rs              # HOJ DTO：ApiResponse/Login/Contest/Problem/Submission(JudgeVO 列表字段)/Announcement/JudgeCase/ContestRank(ACM+OI)/UserProblemStatus + 全状态码映射（PA 独立变体，P41 修复）+ strip_nulls
        │   │   ├── error.rs              # HOJError
        │   │   └── tests/
        │   │       ├── mod_tests.rs      # Adapter 行为测试（token 轮换、parse_cid、parse_hoj_json、体内鉴权失败判定、会话三态判据、preview）
        │   │       ├── types_tests.rs    # DTO 解析测试（榜单 ACM/OI 归一、封榜只有 tryNum、打星 rank=-1、字段缺失与 null 容错、map_status 全表、公告/提交列表/测试点夹具）
        │   │       └── fixtures/
        │   │           ├── contest_list_anon.json      # 真实 get-contest-list 响应（已脱敏），锁定 null 容错回归
        │   │           ├── announcement_list.json      # 按文档手工构造（未经真实联调，P57 待联调清单）
        │   │           ├── contest_submissions.json    # 同上
        │   │           └── case_result.json            # 同上
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
        │   ├── http.rs                   # HttpClient 封装（超时可注入 with_timeout —— 由 oj.timeout_secs 驱动、重试/UA/Cookie；请求头由调用方以 HeaderMap 注入 —— 认证方式是 Adapter 层概念；只返回原始响应体与响应头，不做反序列化）
        │   ├── storage.rs                # Storage 底层文件工具
        │   ├── cache.rs                  # 缓存原语（TtlCache：TTL + 容量上限，近似 FIFO 淘汰；JsonDiskCache：cache/{ns}/{key}.json，条目带 fetchedAt 跨重启计时、损坏容忍、过期懒删除）
        │   ├── logger.rs                 # Logger（Tracing 双路输出：stderr + {base_dir}/logs/hinina.log，启动时 >5MB 截断；敏感信息不落日志靠调用点约束——IPC 日志不记参数）
        │   ├── fs_workspace_repo.rs      # FsWorkspaceRepository（阶段 2 完成）
        │   ├── fs_config_repo.rs         # FsConfigRepository（阶段 2 完成）
        │   ├── fs_plugin_repo.rs         # FsPluginRepository（骨架）
        │   ├── provider_registry_impl.rs # ProviderRegistryImpl（单表 HashMap<OjId, ProviderSet> + capability() 能力取件帮手）
        │   └── tests/
        │       ├── storage_tests.rs      # Storage 单元测试
        │       ├── http_tests.rs         # HttpClient 单元测试（401→Auth、403/5xx→Network、退避延迟、classify_status 重试判据、with_timeout 构造）
        │       ├── logger_tests.rs       # Logger 文件输出测试（落盘/追加/超限截断）
        │       ├── fs_workspace_repo_tests.rs  # FsWorkspaceRepository 单元测试
        │       ├── fs_config_repo_tests.rs     # FsConfigRepository 单元测试
        │       ├── provider_registry_impl_tests.rs # 注册表查询侧契约（未注册/缺能力→ProviderNotFound、覆盖注册、active 规整）
        │       └── cache_tests.rs        # 缓存原语测试（TTL/容量/复活回归/磁盘往返）
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
        └── commands/                     # Tauri Command 薄封装
            ├── mod.rs                    # register_commands() 入口（含 #[cfg(test)] tests 引用）
            ├── auth_cmd.rs               # login(username, password)（OJ 切换已解耦至 switch_oj）/ logout（编排：清会话 + 清用户域缓存）/ get_session / validate_session（三态）
            ├── oj_cmd.rs                 # switch_oj（显式切换：校验已注册 → 切 Registry → 持久化 oj.active → 发布 OJSwitched）
            ├── contest_cmd.rs            # list_contests / select_contest / load_configured_contest（读 oj.contest_ref，不透明字符串引用）/ get_contest_rank / list_contest_announcements / get_read_announcement_ids / mark_announcements_read（uid 取自会话）
            ├── problem_cmd.rs            # get_problem / list_problems / get_user_problem_status / get_contest_problem_limits
            ├── submission_cmd.rs         # submit_code / get_judgement / list_contest_submissions（onlyMine 后端恒 true）/ get_submission_detail / get_submission_cases
            ├── workspace_cmd.rs          # load_workspace / save_workspace / switch_workspace / current_workspace / update_workspace_file / set_workspace_language
            ├── config_cmd.rs             # get_config / reload_config / update_config / get_storage_info（存储目录/日志路径/版本，设置页「关于」）
            ├── theme_cmd.rs              # get_theme / set_theme
            └── tests/
                └── mod_tests.rs          # Command 层关键路径测试（P40 + 分页默认值/StorageInfo 序列化/uid 回退/空筛选归一）
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
├── main.ts                               # Vue 应用入口（Pinia + Router + Naive UI + 全局会话守卫装配 + 工作区落盘事件订阅 + 关窗前落盘握手 + KaTeX 公式样式全局引入，字体本地打包不经 CDN）
├── App.vue                               # 根组件（n-config-provider + n-dialog-provider）
├── env.d.ts                              # Vite 环境类型声明
├── router/
│   └── index.ts                          # Vue Router（/login + /contest 嵌套子树：problems / problem/:displayId / rank / submissions / submissions/:submitId / announcements / settings）+ requiresAuth 认证守卫
├── views/
│   ├── LoginView.vue                     # 登录页（左右分栏：登录表单/已登录身份块 + 几何 SVG 氛围区/比赛简介/倒计时）
│   │                                     #   已登录且比赛未开始时留在本页等待，倒计时归零自动进入赛场
│   ├── ContestLayout.vue                 # 比赛工作台外壳：TopBar + ActivityBar + <router-view> + StatusBar；比赛就绪后启动公告轮询（红点全页面鲜活）
│   ├── ProblemSetView.vue                # 题目总览（统计条 + 卡片网格，limits 渐进填充，30s±5s 轮询；我的状态改为增量失效后按需重拉）
│   ├── ProblemSolveView.vue              # 解题页（题面分节 ｜ 编辑器 + 控制台条，可拖拽分栏；splitRatio 读配置 + 拖拽回写；切题/失焦/页面隐藏/离开页面时落盘工作区）
│   ├── RankView.vue                      # 实时榜单（工具条 + 表格 + 分页，10s±2s 轮询、后台暂停、结束即停；打星/女生队全量快照模式）
│   ├── SubmissionsView.vue               # 评测页（筛选工具条 + 全场提交表格 + 分页，onlyMine 后端强制；?problem= 自动预筛；非终态行 5s±1s 温和刷新）
│   ├── SubmissionDetailView.vue          # 提交详情页（判定横幅 + CE 面板 + 测试点表格/子任务分组 + Monaco 只读代码区，评测中轮询至终态）
│   ├── AnnouncementsView.vue             # 公告页（浅色卡片 feed：长文折叠 + 未读圆点，进入即全部已读）
│   └── SettingsView.vue                  # 设置页（OJ（当前 OJ 下拉=显式 switch_oj + 实例地址 + 比赛 ID/引用）/编辑器/布局/主题置灰/关于 五分组，读改写 AppConfig + 生效性提示）
├── components/
│   ├── layout/
│   │   ├── TopBar.vue                    # 顶栏（拖拽区 + 窗口控制 + 状态徽章 + 倒计时胶囊 + 比赛简介抽屉 + 用户 pill）
│   │   ├── ActivityBar.vue               # 左侧活动栏（题目/榜单/评测/公告 + 底部设置，router-link 驱动高亮；公告项挂真实未读红点徽标）
│   │   └── StatusBar.vue                 # 底部状态条（连接状态 + 客户端版本）
│   ├── contest/
│   │   └── ContestStatsBar.vue           # 统计卡（解题进度 / 实时排名 / 总罚时，数据源=榜单我的行）
│   ├── editor/
│   │   ├── CodeEditor.vue                # Monaco Editor 封装（语言工具条（候选=题目允许语言列表）+ 自动备份指示 + focus()；readonly 模式供详情页复用；字号/Tab/主题读配置，弹层改动即时生效 + debounce 落盘）
│   │   ├── EditorSettingsPopover.vue     # 编辑器设置弹层（字号 8–32 滑杆 / Tab 宽度 2/4/8 / 编辑器主题 vs·vs-dark / 恢复默认；受控组件，应用与落盘由 CodeEditor 承担）
│   │   └── EditorConsoleBar.vue          # 编辑器底部控制台条（最新记录 pill=服务端真实最新提交+首个失败测试点 / 提交记录 (n) 入口 / 提交代码 / 光标与缩进状态行）
│   ├── problem/
│   │   ├── ProblemCard.vue               # 题目卡片（字母徽章取 HOJ 气球色、limits、通过数、我的状态 AC 优先、评测记录带 ?problem= 跳转、快捷提交弹窗入口）
│   │   ├── QuickSubmitDialog.vue         # 快捷提交对话框（语言下拉 + Monaco + 拖拽/选择 .cpp/.c/.java/.py ≤256KB 扩展名识别语言 + 提交与内联评测结果）
│   │   ├── ProblemTabStrip.vue           # 题目快速切换条（A/B/C… chips，标记已 AC / 已尝试）
│   │   └── ProblemStatement.vue          # 题面分节展示（描述/输入/输出/样例+复制/提示，Markdown 渲染 + 相对图片 URL 改写）
│   ├── rank/
│   │   ├── RankToolbar.vue               # 榜单工具条（服务端 keyword 搜索 300ms 防抖 + 分组 tab + 赛制图例 + OI 计分规则徽章）
│   │   ├── ScoreboardTable.vue           # 榜单表格（粘性表头/我的行/前两列，ACM 与 OI 分流渲染）
│   │   └── RankCell.vue                  # ACM 单元格（一血/通过/未通过/封榜/赛后提交/未作答）
│   └── common/
│       ├── LoadingSpinner.vue            # 通用加载动画
│       └── ErrorMessage.vue              # 通用错误提示 + 重试按钮
├── stores/
│   ├── authStore.ts                      # 用户认证状态（登录/登出/会话恢复/三态校验 + sessionResolved 守卫标记）
│   ├── session.ts                        # 会话级领域状态清理（登出/切换账号时重置比赛/题目/提交/榜单/公告/工作区并回收定时器）
│   ├── sessionGuard.ts                   # 全局会话守卫（认证类 IPC 失败 → 判定失效 → 清理并回登录页），由 main.ts 装配
│   ├── contestStore.ts                   # 比赛 + 题目摘要状态 + loadContest 并发去重 + whenLoaded 统一等待入口（P59）+ 登录页匿名比赛简报状态（brief*）
│   ├── problemStore.ts                   # 当前题目详情 + limits 缓存 + 我的题目状态（limitsOf/statusOf 派生读取；myStatusStale 增量失效，提交终态触发重拉）
│   ├── rankStore.ts                      # 榜单状态与轮询编排（uid 去重、参与人数口径修正、分组筛选、我的行、后台暂停；打星/女生队全量快照模式：跨页拉取+客户端过滤分页；用户操作路径查询去抖 in-flight 合并 + 3s memo，轮询与手动刷新不走去抖）
│   ├── submissionStore.ts                # 提交记录 + 评测收敛轮询（createPoller，终态/超时停止，登出统一回收；终态时触发 problemStore.invalidateMyStatus）+ 服务端提交历史（history 筛选/分页）+ fetchProblemSummary
│   ├── announcementStore.ts              # 公告列表 + 客户端已读状态（unreadCount 红点数据源、markAllRead 乐观更新+失败回滚、60s±10s 轮询）
│   ├── workspaceStore.ts                 # 工作区 + 代码编辑器状态（两级状态机：syncPending=未推送内存 / isDirty=未落盘；语言权威值=HOJ 显示名，切换即时持久化；源文件名经 utils/language 派生；落盘事件订阅安装器）
│   └── __tests__/                        # authStore / contestStore / rankStore / submissionStore / announcementStore / workspaceStore .spec.ts（会话状态机、加载去重与 whenLoaded、榜单去重/轮询/全量模式、评测收敛轮询、公告未读语义、工作区落盘状态机与 flush 顺序）
├── services/
│   ├── auth.service.ts                   # 登录/登出/会话检查/三态会话校验（localStorage 缓存，登出失败也清本地）
│   ├── config.service.ts                 # 配置读写唯一入口（进程内缓存 + 兜底）+ updateConfig 读改写 + 派生参数（OJ 基址、轮询调度、编辑器偏好、默认语言归一、分栏比例）
│   ├── contest.service.ts                # 加载配置的比赛 + 登录页匿名比赛简报编排（config → contestId → 列表筛选）
│   ├── problem.service.ts                # 获取题目详情/列表 + 我的题目状态 + 批量 limits
│   ├── rank.service.ts                   # 榜单查询（分页/关键词/去打星参数编排）
│   ├── submission.service.ts             # 提交代码/轮询评测 + 提交历史/详情/测试点查询
│   ├── announcement.service.ts           # 公告列表 + 已读集合组装（markRead 返回后端合并后的权威集合）
│   ├── system.service.ts                 # 客户端存储信息（存储目录/日志路径/版本，设置页「关于」）
│   ├── workspace.service.ts              # 工作区创建/保存/恢复/语言持久化 + 落盘事件订阅（onWorkspaceSaved）
│   └── __tests__/                        # auth.service / config.service .spec.ts（本地缓存清理与三态归一契约、配置缓存/派生兜底/updateConfig 读改写）
├── bridge/
│   ├── index.ts                          # ipcInvoke 统一封装 + IpcError（AppError 载荷归一化为 Error，单点日志且不记录参数）
│   ├── auth.bridge.ts                    # login / logout / get_session / validate_session
│   ├── contest.bridge.ts                 # load_configured_contest / list_contests（匿名，登录页比赛信息）
│   ├── problem.bridge.ts                 # get_problem / list_problems / getUserProblemStatus / getContestProblemLimits
│   ├── rank.bridge.ts                    # get_contest_rank
│   ├── submission.bridge.ts              # submit_code / get_judgement / list_contest_submissions / get_submission_detail / get_submission_cases
│   ├── announcement.bridge.ts            # list_contest_announcements / get_read_announcement_ids / mark_announcements_read
│   ├── system.bridge.ts                  # get_storage_info
│   ├── workspace.bridge.ts               # load_workspace / save_workspace / current_workspace / updateWorkspaceFile / setWorkspaceLanguage + onWorkspaceSaved（workspace-saved 事件订阅）
│   ├── config.bridge.ts                  # get_config / update_config / reload_config
│   └── __tests__/                        # index.spec.ts（AppError → IpcError 跨端契约、日志不泄露参数）
├── types/
│   ├── user.ts                           # User 实体 + SessionValidity（valid/invalid/unknown 三态）
│   ├── contest.ts                        # Contest（+ oiRankScoreType OI 计分规则）+ ContestProblem 实体
│   ├── problem.ts                        # Problem（+ languages 本题允许提交语言）+ Sample 实体
│   ├── announcement.ts                   # Announcement + AnnouncementPage（时间为 epoch 秒）
│   ├── submission.ts                     # JudgementStatus（HOJ 全状态码 18 变体）+ JudgementResult + SubmissionRecord/Page/Detail + JudgeCase/SubTaskCases/SubmissionCases + SubmissionListQuery
│   ├── system.ts                         # StorageInfo（存储目录/日志路径/版本）
│   ├── rank.ts                           # RankCell / ContestRankRow / ContestRankPage / RankQuery / ProblemLimits（榜单与题目限制跨端契约）
│   ├── workspace.ts                      # Workspace 实体
│   └── config.ts                         # AppConfig 及其子配置（含 oj.cacheProblemStatement 题面缓存开关）
├── utils/
│   ├── markdown.ts                       # Markdown + LaTeX 公式渲染（marked，KaTeX 在 tokenizer 层接管 $/$$，中文无空格 nonStandard）+ Vditor ::: 排版容器（hljs-center 居中块）+ DOMPurify 出口统一消毒（P49/P63，mathMl/svg 档 + semantics/annotation 无障碍树补白；剥离只为安全，表现性标记保真渲染）+ 相对图片 URL 改写为 HOJ 绝对地址
│   ├── contest.ts                        # 比赛阶段推导纯函数（getContestPhase / hasContestStarted，登录页与顶部栏共用）
│   ├── submission.ts                     # 评测终态判据（isTerminalStatus，与 Rust 对齐）+ 状态文案/缩写/色调唯一映射（statusLabel/statusAbbr/statusTone/STATUS_OPTIONS）+ 时间/内存/长度格式化 + findFirstFailedCase
│   ├── editor.ts                         # 编辑器偏好值域唯一权威模块（主题候选 vs/vs-dark + normalizeEditorTheme 归一 / 字号 8–32 / Tab 存储域 1–8 与候选档位 2·4·8；前端各消费方一律取此处常量，Rust sanitize 同域）
│   ├── logger.ts                         # 前端日志唯一入口（createLogger 作用域前缀 + debug/info 仅开发环境、warn/error 恒输出；不落盘，持久化日志归 Rust tracing）
│   ├── error.ts                          # 错误文案收敛唯一出口（errorMessage：Error/字符串取信息，空值与非 Error 载荷回退兜底文案；不依赖 bridge，纯函数）
├── language.ts                       # 语言域唯一权威模块（权威值 = HOJ 显示名；monacoIdOf 高亮派生 / sourceFileNameOf 源文件名 / normalizeHojLanguage 历史值归一 / hojLanguageOfFileName 扩展名反推 / isCLikeLanguage 倍率判定）
│   ├── polling.ts                        # 轮询原语（planPollDelayMs 抖动错峰 + createPoller 递归 setTimeout：重入保护/可暂停/定时器可注入）
│   ├── rank.ts                           # 榜单渲染纯映射（单元格文案与样式、显示名回退、uid 去重、跨页合并 mergeRankPages、分组过滤/客户端分页、参与人数口径修正、罚时格式化）
│   ├── limits.ts                         # 题目时限/内存格式化与语言倍率换算（C/C++ 1 倍，其它语言 ×2）
│   └── __tests__/                        # contest / submission / session-check / polling / rank / limits / markdown / markdown-dom / editor / logger / error .spec.ts（阶段判据、终态穷尽映射、预检调度边界、轮询节奏、榜单映射与口径、limits 换算、渲染契约、jsdom 消毒激活态的生产链路契约、编辑器偏好值域、日志级别分流与作用域前缀、错误文案收敛）
└── styles/
    └── global.css                        # TailwindCSS + CSS 变量（电光紫主题 #7C5CFF）+ 暗色主题 + KaTeX 公式全局样式（块级公式横向滚动防裁切，题面/公告/简介共用）；**不覆写 Monaco 背景**（编辑器底色由 theme.editorTheme 自绘，否则 vs-dark 只换字色）
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
- **View → Service 直连判据**（M1 成文）：红线只有一条 —— View / component **禁止 import `@/bridge`**（可 grep 断言）。在此之上按数据生命周期分流：**跨视图共享或需跨视图存活的状态**（比赛、榜单、提交历史、公告与已读、工作区）必须走 Store；**路由级瞬态数据**（随视图销毁即丢弃的一次性查询，如提交详情的 detail/cases、设置页表单初值、存储信息）允许 View/component 直连 Service，本地 `ref` 承载 —— 为瞬态数据建 store 只会带来 store 膨胀与清理义务，零共享收益。瞬态数据若需轮询，轮询器由视图自持（`createPoller`，`onUnmounted` 必停）；判据存疑时问一句「第二个视图会读它吗」，会 → Store，不会 → Service 直连
- **前端会话与导航**：应用入口统一为 `/login`；`router.beforeEach` 在首次导航时恢复会话，并拦截 `meta.requiresAuth` 路由（未登录一律回登录页）；登录页依据比赛阶段（`utils/contest`）决定是否进入赛场 —— 比赛未开始时留在登录页等待，倒计时归零后自动进入；登出时经 `stores/session.ts` 清空会话级领域状态
- **会话失效处理**：三态校验（`valid` / `invalid` / `unknown`）贯穿 Adapter → `AuthService::validate_session` → command → 前端 store；`unknown`（网络异常）一律**保留**登录态并重试，只有服务端明确判定失效才清理会话回登录页 —— 赛前误踢选手的代价远高于多等一轮校验，且反复重登可能触发 HOJ 暴力破解锁定（同 IP + 同用户名 30 分钟 20 次）。全局兜底由 `stores/sessionGuard.ts` 承担：任何认证类 IPC 失败即判定失效（在组合根注入观察者，Bridge 不感知 store/router）
- **三态契约必须由 Adapter 兑现**：`AuthProvider::validate_session` 的返回值语义是 `Ok(true)` 有效 / `Ok(false)` 服务端**明确**判定失效 / `Err(_)` 无法判定。**绝不可把网络错误折成 `Ok(false)`** —— 那会让 `SessionValidity::Unknown` 分支成为死代码，一次赛前网络抖动就把选手踢回登录页。HOJ 侧的判据抽成纯函数 `session_validity_from_response` 以便测试锁定：仅 `AppError::Auth` 算明确失效，非 200 的其它状态码（400/500）归 `Unknown`，网络/超时/解析失败一律上抛。同理，评测查询（`SubmissionService::get_judgement`，单次查询）**不得吞掉认证错误**：必须原样上抛，否则前端收敛轮询会把 401 当瞬时抖动重试到超时，选手干等五分钟后只收到「评测超时」，守卫也拿不到 Auth 变体
- **赛前预检错峰**：登录页等待开赛时按 `utils/session-check.ts` 的策略校验会话 —— 距开赛 >10min 每 5min±60s 周期复检，进入 [T-10min, T-3min] 窗口后在剩余区间随机取点做一次性预检，迟到启动则 0–3s 抖动后立即执行，距开赛 ≤30s 不再预检。目的是把全场客户端的校验请求散布开，避免开赛前形成同步尖峰；**进场（T-0 导航）不错峰**，准点进场是公平性要求
- **IPC 错误归一化**：Rust `AppError` 经 serde 序列化为 `{ Variant: msg }` 对象，`bridge/index.ts` 在唯一出口转换为 `IpcError extends Error`，保证上层 `e instanceof Error` 与 `e.message` 可用；日志不记录调用参数（含明文密码）
- **错误文案与日志各有一个出口**：面向用户的错误文案统一经 `utils/error.errorMessage(e, '兜底')` 收敛（空 message、非 Error 载荷一律回退兜底文案，杜绝白屏式空白提示），禁止各处再写 `e instanceof Error ? e.message : '…'`；日志统一经 `utils/logger.createLogger('<模块名>')`（`[模块名]` 前缀、`debug`/`info` 仅开发环境、`warn`/`error` 恒输出），禁止散落 `console.*`。前端日志**只进 console 不落盘**（持久化由 Rust `tracing` 负责），且与 bridge 同款安全约束：不记录敏感参数
- **错误变体是分流依据，后端不得改写**：前端 `isAuthError`（`variant === 'Auth'`）与 `stores/sessionGuard.ts` 的会话失效兜底完全依赖变体。补上下文一律用 `AppError::context()`（保留变体，只在消息前拼环节名），**禁止** `AppError::Network(format!("xx 请求失败: {}", e))` 这类重新包装 —— 它会把反序列化失败、认证失败一律改写成「网络错误」，现场看到「网络错误: … 序列化错误: …」自相矛盾的嵌套消息，把 DTO 问题当断网查，还会让 401 不再触发登出。**Service 层传播 Provider 错误同样适用此约定**（`contest` / `problem` / `submission` / `auth` 全部用 `e.context("…")`）：`get_rank` 是全场最高频的认证调用（每 10s 一次），变体被改写会让 token 过期时榜单静默 stale、提交只弹一条文案、选手永远回不到登录页
- **OJ 响应解析归 Adapter，infra 只传字节**：`infra/http.rs` 只返回原始响应体与响应头（含状态码判定与 5xx 退避重试），不做反序列化；**请求头也由调用方以通用 `HeaderMap` 注入** —— 认证方式是 Adapter 层概念（HOJ 的 JWT 走 `Authorization` 头、Hydro 走 Cookie 会话、有的 OJ 还要 CSRF 令牌），infra 不感知任何凭证形态，曾以 `auth_token` 参数 + 硬编码 `Authorization` 头把 HOJ 假设埋进传输层，已修正。HOJ 侧有两个必须处理的协议事实：① 对未设置字段返回 `null` 而非省略（实测 `get-contest-list` 的 `sealRank`/`rankShowName`/`count`/`now` 全为 null），而 serde 的 `#[serde(default)]` **只在字段缺失时生效**，显式 null 会让整个响应解析失败 → 解析前统一 `strip_nulls`（`false`/`0`/`""` 不是 null，必须保留，否则封榜、打星、零分语义会被抹掉）；② 鉴权失败放在**响应体的 status**（HTTP 仍是 200，实测匿名访问 `get-contest-problem` 返回 `{"status":403,"msg":"请您先登录！"}`）→ 必须翻译成 `AppError::Auth`，且 403 要保守判定（仅当消息指向登录/凭证时才算会话失效，否则「私有赛未注册」会把已登录选手误踢回登录页）
- **HTTP 401 由 infra 映射为 `Auth` 变体**：401 的标准语义就是「未认证」，属 HTTP 通用语义而非 OJ 私有约定，故由 `infra/http.rs` 的 `status_error` 承担；**403 保持 `Network`**（可能是业务性无权访问）。这条映射是会话校验能成立的前提 —— `get_json_authed` 遇到 401 时若仍归为 `Network`，`session_validity_from_response` 会把它当「无法判定」上抛，导致 token 真正过期时反而永不登出。实测 HOJ 两种报法都存在：`get-user-auth-info` 走 HTTP 401，`get-contest-problem` 走 HTTP 200 + 体内 403，两条路径都必须认- **真实响应夹具**：`adapter/hoj/tests/fixtures/contest_list_anon.json` 取自真实接口、仅脱敏自由文本，完整保留键名与 null 分布；配套一条正向测试（真实响应可解析）与一条反向测试（不去 null 必然失败），防止后来者把 `strip_nulls` 当冗余删掉
- **OJ 身份与配置是数据，不是编译期常量**：`OjId(String)` 取代闭集枚举 `OJType`（接一个新 OJ 不再要求修改 Domain）；会话文件名 = `sessions/{id}.json` 显式契约（内建 id 与历史枚举 Debug 输出一致，`sessions/HOJ.json` 零迁移，有测试锁定；配置实例 id 拒绝路径分隔符与 `..`，`Storage::resolve` 为第二道防线）；`OJSwitched` 事件载荷为可序列化字符串（订阅者：contest/problem/submission 三个 Service 清各自 OJ 域缓存）。失去编译期穷尽检查的替代防线：启动时校验 active 已注册（未注册 warn + **回退首个已注册 OJ**——硬编码回退 HOJ 在 HOJ 被禁用/移除时是死路）、查询未命中返回 `ProviderNotFound`、`adapter/tests` 断言 `factories()` id 唯一且全部可构建
- **注册侧聚合、查询侧按能力**：一个 OJ 的能力集合是 `ProviderSet`（字段 Option×4 —— 保住「新 Adapter 可先只实现部分接口」的扩展路径，缺能力报 `ProviderNotFound` 而非注册失败），组合根对每个 OJ 一次 `register(id, set)`；Service 查询只拿单项能力（`current_contest()?` 等一行转发），**禁止提供返回聚合体的 `current()`** —— 那会让 Service 拿到它不需要的三个能力，接口隔离从接口层面退化成约定层面
- **`AdapterDeps` 只准 infra 依赖**：适配器工厂构造签名只接收 http_client / event_bus / storage，**禁止把 Service 塞进 `AdapterDeps`**（与「插件只能访问 `plugin/api`、禁止直调内部 Service」同理 —— 适配器一旦反向依赖应用层，依赖边界彻底糊掉）。`AdapterFactory { id, build }` 的形状即 v1.0 插件 manifest 的雏形：将来把编译期工厂清单换成运行时扫描插件目录，上层（registry / context / Service）不用再改
- **接入新 OJ = 1 个子目录 + `factories()` 一行 + `oj.instances` 一条配置**：`OjConfig` 按 `OjInstance{ id, baseUrl, enabled, options }` 实例清单组织（`options` 刻意弱类型 Map —— 强类型枚举会让「新 OJ 要改 core」原样复活）；`contest_ref` 是**不透明字符串引用**（HOJ 数字串 / 其它 OJ 任意资源 ID，装得下 Hydro 的 hex ObjectId），空串 = 未配置；旧格式（hojUrl / contestId / lastOjType）经 serde 过渡字段在 `normalize_legacy_values` 一次性迁移、永不写回。OJ 切换走显式 `switch_oj` 命令（校验已注册 → 切 Registry → 持久化 `oj.active` → 发布 `OJSwitched`），**不是 login 的副作用**。切换的后果按端分工：Rust 侧由 `OJSwitched` 订阅者清 OJ 域缓存（contest/problem/submission 三个 Service）；前端在**调用点**重置会话上下文（`resetSessionForOjSwitch`：领域状态清零 + `sessionResolved` 复位 + 回登录页由守卫按新 OJ 会话文件恢复）—— 切换是前端发起的命令，发起方编排后果（与登出同款模式），不引入 Tauri 事件桥；各 OJ 会话文件按 id 隔离，切换保留旧 OJ 登录态（切回免登录），故前端重置不得走 `authStore.logout()`（会误删新 OJ 的会话文件）
- **配置与轮询归属**：配置读取统一经 `services/config.service.ts`（进程内缓存 + 兜底），View/Store 不得直接调用 `config.bridge`；评测轮询的**节拍与超时唯一归属前端** `submissionStore`（createPoller 驱动，终态判据见 `utils/submission.ts`，deadline 兜底），后端 `get_judgement` 是单次查询、无内层循环 —— 双层轮询会让前端抖动沦为装饰、`stopPolling` 停不掉在途后端循环；View 只表达提交意图
- **比赛工作台外壳**：`ContestLayout` 承载 TopBar + ActivityBar + `<router-view>` + StatusBar，各功能页是平级路由而非单页三栏；窗口拖拽与窗口控制只在 TopBar（登录页由 `App.vue` 提供兜底窗口条）。View 与 component **禁止**直接 import `@/bridge`（分层判据，可 grep 断言）
- **轮询统一原语**：周期性刷新（榜单、题目总览、公告）走 `utils/polling.ts` 的 `createPoller`（递归 setTimeout + 抖动 + 重入保护 + `document.hidden` 暂停），定时器句柄由 store 持有（模块级普通变量，不进 `ref/reactive`），离开路由或比赛结束（`status == 1`）必须停止。提交结果轮询是**按提交 ID 的一次性收敛轮询**（终态判据 + 总超时，见 `utils/submission.ts`），已统一到 `createPoller`（P54），但**刻意不配置 hidden 暂停** —— 选手切窗口查资料回来就该看到结果，暂停只会拉长「评测中」焦虑期。全部轮询场景的节奏矩阵：

  | 场景 | 节奏 | hidden 暂停 | 停止判据 | 归属 |
  |------|------|------------|---------|------|
  | 榜单实时刷新 | 10s ± 2s | ✅ | 离开榜单页 / 比赛结束 / 全量快照模式转手动 | `rankStore` |
  | 题目总览刷新 | 30s ± 5s | ✅ | 离开路由 | `ProblemSetView`（视图自持） |
  | 公告未读保鲜 | 60s ± 10s | ✅ | 外壳卸载 / 比赛结束 / 登出 | `announcementStore`（ContestLayout 启动） |
  | 评测页当前页温和刷新 | 5s ± 1s | ✅ | 当前页无「评测中」行 / 离页 | `SubmissionsView`（视图自持） |
  | 提交结果收敛轮询 | 配置 `oj.pollIntervalSecs`（默认 2s），抖动 ±20% 封顶 500ms | ❌（刻意） | 终态 / 总超时（默认 300s）/ 登出 | `submissionStore`（每提交一个 Poller；后端 `get_judgement` 为**单次查询**，节拍与超时全部由前端拥有，抖动真实生效） |
  | 提交详情页收敛轮询 | 同上（读同一配置） | ❌（刻意） | 终态（终态后补拉一次测试点）/ 总超时 / 离页 | `SubmissionDetailView`（视图自持，评测中只拉 detail 不拉 cases，避免请求放大） |
- **公告已读状态是客户端特性**：HOJ 无已读概念，已读 ID 集合由 Rust 端按「比赛 + 用户」持久化（`announcements_read/{cid}_{uid}.json`，合并去重、损坏降级为空 + warn）；未读红点 = 列表与已读集合的差集，由外壳启动的公告轮询在全部页面保持鲜活，进入公告页即全部标记已读（乐观更新，持久化失败回滚 —— 红点复发优于假已读）
- **提交列表「只看本人」由后端强制**：`list_contest_submissions` 命令层恒置 `onlyMine = true`，前端不传该参数、不可绕过（产品决策：评测页只显示本人提交）
- **榜单数据源职责**：卡片「我的状态」取 `get-user-problem-status`（轻量、不受榜单分页/搜索影响，AC 判定与榜单我的行取并集且 **AC 优先**）；统计卡「解题进度 / 实时排名 / 总罚时」取榜单我的行（服务端前置复制，天然可得）；`ac/total` 与气球色取比赛题目列表。HOJ 会把当前用户与关注列表**前置复制**进 `records`，渲染前必须按 `uid` 去重，`total` 因此偏大、不能直接当参赛人数（口径：`total − 本页重复数`，且当「我的前置副本在页内而自然名次不在本页窗口」时再 −1，见 `utils/rank.resolveParticipantCountFromPage`；关注用户的页外副本仍是已知残差）；`rank == -1` 是打星队伍；封榜以 `contest.sealRank + sealRankTime` 自行判断，**不依赖 `forceRefresh`**（对非管理员无效）
- **语言权威值 = HOJ 显示名**：提交契约（`submit-problem-judge` 的 `language: "C++"`）、题目详情 `languages` 允许列表、工作区元数据与配置 `defaultLanguage` 全部使用 HOJ 显示名；Monaco 高亮 id 与源文件名是**派生值**，只在消费点经 `utils/language` 映射（`monacoIdOf` / `sourceFileNameOf`），绝不反向作为存储值 —— Monaco id 有损（C++17/C++20 同归 'cpp'）且无法承载 Go/Rust 等语言。语言下拉候选以**题目详情返回的允许列表**为准（HOJ 按题限制语言），服务端未提供时回退内置 `DEFAULT_LANGUAGES`；历史遗留的 Monaco id（P55 时代的 'cpp'）由前后端归一函数双向兜底迁移。未知语言的文件名回退 `main.txt` 而非猜测 `.cpp` —— 判题端按后缀判语言，猜错后缀等于用错语言评测
- **打星队/女生队跨页过滤**：服务端只有 `removeStar`（正式参赛队走它，跨页正确）；打星/女生无服务端参数，切「全量快照模式」—— 顺序拉全部分页（上限 40 页 / 2000 行，超限标记 truncated 并提示）、跨页 uid 去重后客户端过滤 + 客户端分页，轮询暂停改手动刷新（全量重拉太重）。HOJ 榜单页是整榜全量重算后分页，连发即让服务端背靠背算整榜 —— **页间强制 400ms 节流**摊开突发；拉取循环持代际令牌并逐页校验筛选态，中途切筛选/登出即中止且不写快照（过期数据不复活、不并发双写）
- **OI 计分规则只读展示**：`oiRankScoreType`（Recent/Highest）是比赛属性（get-contest-info 返回），不是请求参数 —— 客户端在榜单工具条渲染徽章，不提供切换
- **ACM / OI 归一**：两套 VO（`ac/total/submissionInfo{对象}` vs `totalScore/submissionInfo{分数}/timeInfo{毫秒}`）在 Adapter 层归一为 OJ 无关实体，前端不感知赛制差异；单元格判档是纯函数（`utils/rank.resolveRankCell` / `resolveOiRankCell`），组件只做样式映射。ACM `totalTime` 是**秒**、OI 是**毫秒**，混用会差 1000 倍
- **题目 limits 缓存**：列表接口不返回 limits，只能按题请求 `get-contest-problem-details`；`ProblemService::load_problem_limits` 做「内存 + 磁盘（`cache/problem_limits/{cid}.json`）」双层缓存、并发上限 4、部分失败跳过、全部失败才上抛；401/403 **不得静默回退默认值**（未注册私有赛必须让选手看见真因）。展示需标注语言倍率（题面是 C/C++ 基准，其它语言时间与内存 ×2）
- **状态文案以接口返回为准**：评测状态直接用后端 `JudgementStatus` 原词（Accepted / Wrong Answer…），不强行缩写为 AC/WA；`get-user-problem-status` 的 0/1/2 映射为「未作答 / 已通过 / 尝试过」
- **工作区语言必须落盘**：语言不属于任何代码文件，`update_workspace_file` 带不上它；`workspaceStore.changeLanguage` 乐观更新本地并调用 `set_workspace_language` 立即持久化元数据，否则切题或重启后退回默认语言，会把 Java 代码当 C++ 提交
- **代码落盘语义 = debounce-to-memory**：编辑器改动经 2s 防抖推送到**后端内存**（`update_workspace_file` 不写盘），磁盘写入只有两条路径 —— 后台 auto-save 周期与显式 `save_workspace`。因此「自动保存间隔」真正决定落盘频率（旧实现的写透让该配置形同虚设），前端状态分两级：`syncPending`（未推内存）/ `isDirty`（未落盘）。三条配套硬约定：① **任何替换内存工作区的操作先落盘旧的**（`create` / `load` / `switch` 共用 `save_current_if_dirty`，前端 `loadWorkspace` 前先 `flushPendingSync`）；② **落盘时机由调用点编排**：切题 / 失焦 / 页面隐藏 / 离开解题页（`ProblemSolveView`）与关窗（`main.ts` 的 `onCloseRequested` 握手）—— auto-save 周期最长 300 秒，这些时刻只靠周期就会丢改动；③ **auto-save 以修订号判定能否清脏**（快照与修订号在同一读锁内取得；写失败或快照后有新改动时保留脏、不发布事件），否则「快照写盘」会被当成新内容已落盘。前端「已自动备份」指示的唯一真相来源是后端 `workspace-saved` 事件（`main.rs` 事件桥下发，仅转发真正落盘的 `Saved` / `AutoSaveTriggered`）
- **客户端缓存策略**（本轮落地，判据 = 数据可变性分层）：**下次看到之前不会变**的数据 → 缓存（比赛元信息 TTL 120s、题面 TTL 30min，均内存 + 磁盘；终态提交详情/测试点 TTL 2h，**仅内存**）；**只由我自己的动作改变**的数据 → 本地增量 + 失效重取（我的题目状态：提交终态时 `problemStore.invalidateMyStatus()`，总览页按需重拉，不再 30s 整表重拉）；**随时可能被别人改变**的数据 → 只轮询，最多做同查询去抖（榜单用户操作路径 in-flight 合并 + 3s memo；**轮询与手动刷新不走 memo**）。四条硬约定：① **缓存是优化不是正确性依赖** —— 读失败回退网络、写失败只 warn、解析损坏视为未命中；② **只缓存成功结果** —— 401/403 等错误永不入缓存，否则会话失效会被掩盖、`sessionGuard` 拿不到 `Auth` 变体；③ **键必须带作用域**（`contest_id` / `submit_id`），**用户域数据不落盘**且登出由 `auth_cmd::logout` 编排 `SubmissionService::clear_user_caches()` 清空；④ **失效路径五条**：TTL、切比赛（键隔离）、**切 OJ**（`OJSwitched` 事件：三个 Service 的缓存键不含 OJ 维度，跨 OJ 同 cid 会撞号，切换即清）、登出、配置开关（`oj.cacheProblemStatement`）。可观测性：命中走 `debug`（字段 `cache` / 实体 id / `hit`），淘汰与写失败走 `warn`。**明确不做**：榜单名次缓存（实时性即公平性）、评测中状态缓存、公告内容缓存、会话校验缓存、按 URL 的通用 HTTP 响应缓存（会连错误体与按 uid 定制的响应一起缓存）
- **离线客户端约束**：不引入外部字体与图标字体（设计稿的 Google Fonts / Material Symbols 一律改内联 SVG），不为此新增 npm 依赖；客户端界面只做浅色主题（dark UI 未实现，`theme.themeName` 恒为 `light`），**编辑器区域例外**：解题页编辑器设置可在 Monaco 内置 `vs` / `vs-dark` 间切换（落在 `theme.editorTheme`，两者互不干扰）。依赖例外有二：安全依赖 `dompurify`（`renderMarkdown` 出口统一消毒——题面/简介/公告等全部 `v-html` 内容来自 OJ 服务端，编辑者面较宽，不按「服务端完全可信」假设，见 P49/P63）与公式依赖 `katex` + `marked-katex-extension`（题面 LaTeX 数学公式渲染；字体随 katex 包本地打包进 dist、**不经 CDN**，离线安全）
