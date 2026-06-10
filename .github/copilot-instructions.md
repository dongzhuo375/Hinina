# Hinina — Copilot 指令

## 项目定位

Hinina 是基于 Tauri 2 + Rust + Vue 3 的 OJ 桌面竞赛客户端，面向 ACM/ICPC 和 XCPC 场景。本项目不是通用 IDE、不是代码编辑器平台。

## 技术栈

| 层 | 技术 |
|-------|-----------|
| 桌面框架 | Tauri 2 |
| 后端 | Rust（Reqwest, Serde, Anyhow, Thiserror, Tracing） |
| 前端 | Vue 3, Vite, TypeScript, Pinia, TailwindCSS, Naive UI |
| 编辑器 | Monaco Editor |
| 存储 | 纯文件存储，不使用 SQL 数据库 |

## 架构

整体采用 Clean Architecture（Rust 端）+ MVVM（Vue 端），前后端通过 Tauri IPC 通信。禁止引入 DDD、CQRS、Event Sourcing 等过度复杂模式。

### Rust 后端 (`src-tauri/`)

| 层 | 关键模块 |
|-------|------------|
| `core/entity` | User, Contest, Problem, Submission, **Workspace** |
| `core/provider` | 拆分的 trait：`AuthProvider`, `ContestProvider`, `ProblemProvider`, `SubmissionProvider` |
| `core/event` | `AppEvent` 枚举 + EventBus |
| `service/` | auth, contest, problem, submission, **workspace**, config, theme |
| `adapter/` | hoj, qduoj, hustoj（各自组合实现需要的 trait） |
| `infra/` | http, storage, cache, logger |

### Vue 前端 (`src/`)

分层顺序：**View → Store → Service → Bridge**

- **Store**（Pinia）：仅管理状态，不包含网络请求、数据转换或业务逻辑
- **Service**：按领域封装业务逻辑（`contest.service.ts`, `problem.service.ts` 等）
- **Bridge**：对 Tauri IPC invoke 的薄封装

### 关键设计规则

- **Workspace First**：`Workspace` 是核心领域对象，负责代码存储、自动保存、崩溃恢复、比赛隔离、模板管理、缓存
- **Provider trait 必须拆分**：禁止单一巨型 `trait OJProvider`，按 Auth/Contest/Problem/Submission 拆分组合
- **EventBus 原则**：查询与命令走 Service，状态变更走 EventBus。禁止将所有逻辑事件化
- **插件系统**：v0.x 仅预留架构，不实现运行时。插件只能访问 `plugin/api/`，禁止直接调用内部 Service

## 项目边界

本项目**不包含**：用户注册、题库管理、OJ 管理后台（这些是服务端职责）。

## 开发约定

- **分支策略**：GitHub Flow，`main` 受保护，功能分支开发，squash merge
- **分支命名**：`feat/<描述>`, `fix/<描述>`, `docs/<描述>`, `chore/<描述>`
- **PR 要求**：至少一人 Review 通过后方可合并（CI/CD 流水线短期不搭建，待后期补充）
- **权威文档**：`doc/开发手册.md`，涉及架构决策、NFR、风险等时请先查阅此文档
- **文件树同步**：每次新增、删除或移动文件/目录时，必须同步更新 `doc/Architecture.md` 中对应的文件树与职责说明
- **模块文档同步**：每次对源文件进行新增、删除、修改职责或接口变更时，须同步更新 `doc/modules/` 下对应的 `.md` 文档（含职责、核心类型/函数、依赖关系、逻辑流程）
- **依赖安装路径**：新增依赖（npm/cargo 等）不得下载到 C 盘。安装前若无已有配置指定路径（如 `.npmrc`、`.cargo/config.toml`），须主动询问用户确认安装位置
