# Hydro OJ 接口参考文档

> 本文档基于 **Hydro OJ** 开源项目源码（`D:\project\Hydro-master`）逐行分析生成，覆盖核心包 `hydrooj` 与前端辅助包 `ui-default` / `ui-next` 的全部 HTTP 路由、WebSocket 连接与新版 JSON-RPC 接口。
>
> 编写目标：可直接用于实现 Hydro 客户端 / Adapter / 第三方集成。
>
> 所有结论均标注了源码文件与行号，便于回溯核对。

---

## 目录

- [0. 阅读指南与总览](#0-阅读指南与总览)
- [1. 通用约定](#1-通用约定)
  - [1.1 基础地址与域（Domain）前缀](#11-基础地址与域domain前缀)
  - [1.2 请求派发模型](#12-请求派发模型)
  - [1.3 参数来源与类型校验](#13-参数来源与类型校验)
  - [1.4 统一请求上下文 `this.args`](#14-统一请求上下文-thisargs)
  - [1.5 响应格式](#15-响应格式)
  - [1.6 错误响应](#16-错误响应)
  - [1.7 认证与会话](#17-认证与会话)
  - [1.8 权限模型：PRIV 与 PERM](#18-权限模型priv-与-perm)
  - [1.9 分页](#19-分页)
  - [1.10 序列化规则（`framework/framework/serializer.ts`）](#110-序列化规则frameworkframeworkserializerts)
  - [1.11 路由名 → URL 的生成](#111-路由名-url-的生成)
  - [1.12 限流](#112-限流)
  - [1.13 常见调用示例](#113-常见调用示例)
- [2. 认证、会话与用户账户](#2-认证会话与用户账户)
  - [认证与会话（源码级详解）](#认证与会话源码级详解)
  - [用户与会话接口（`packages/hydrooj/src/handler/user.ts`）](#用户与会话接口packageshydroojsrchandleruserts)
  - [首页与个人中心接口（`packages/hydrooj/src/handler/home.ts`）](#首页与个人中心接口packageshydroojsrchandlerhomets)
  - [实时事件接口（`packages/hydrooj/src/handler/connection.ts`）](#实时事件接口packageshydroojsrchandlerconnectionts)
- [3. 题目、提交与评测](#3-题目提交与评测)
  - [题目接口（problem）与通用补充约定](#题目接口problem与通用补充约定)
  - [评测记录（record）](#评测记录record)
  - [评测机（judge）](#评测机judge)
  - [服务状态（status）](#服务状态status)
  - [评测状态码](#评测状态码)
  - [评测流程（submit → 入队 → 下发 → 回调）](#评测流程submit-入队-下发-回调)
  - [题目配置（`config.yaml`）字段](#题目配置configyaml字段)
  - [语言与代码模板](#语言与代码模板)
- [4. 比赛、作业与排行榜](#4-比赛作业与排行榜)
  - [比赛（Contest）路由](#比赛contest路由)
  - [作业（Homework）路由](#作业homework路由)
  - [比赛排行榜（Scoreboard）详解](#比赛排行榜scoreboard详解)
- [5. 训练、讨论、域与系统管理](#5-训练讨论域与系统管理)
  - [训练（Training）](#训练training)
  - [讨论（Discussion）](#讨论discussion)
  - [排名与域管理（Domain）](#排名与域管理domain)
  - [系统管理（Manage）](#系统管理manage)
  - [杂项（Misc）](#杂项misc)
  - [域（Domain）与权限模型](#域domain与权限模型)
- [6. JSON-RPC 接口与 UI 辅助接口](#6-json-rpc-接口与-ui-辅助接口)
  - [JSON-RPC 接口体系](#json-rpc-接口体系)
  - [已注册的 API 操作参考](#已注册的-api-操作参考)
  - [UI 路由（ui-default / ui-next）](#ui-路由ui-default-ui-next)
  - [前端如何调用后端接口](#前端如何调用后端接口)

---

## 0. 阅读指南与总览

### 0.1 两套并行的接口体系

Hydro 同时存在**两套**接口体系，用途不同：

| 体系 | 入口 | 类型 | 说明 |
|------|------|------|------|
| **传统路由（Handler）** | `/p/:pid`、`/contest/:tid`、`/record/:rid` … | 页面 + JSON | 主要接口。默认渲染 HTML 模板；**带 `Accept: application/json` 即返回原始 JSON**。 |
| **新版 JSON-RPC（`ctx.api`）** | `/d/:domainId/api/:op` + WebSocket `/d/:domainId/api/:op/conn` | 纯 JSON | 新式强类型接口，`Query` / `Mutation` / `Subscription` 三种语义，支持字段投影（projection）。目前操作数量较少但为官方推荐方向。 |

> **最重要的一条**：任何 Handler 路由只要请求头带 `Accept: application/json`，就会跳过 HTML 渲染、直接返回 `this.response.body` 的 JSON。这是把 Hydro 当纯 API 使用的核心手段。

### 0.2 认证速查

| 方式 | 写法 |
|------|------|
| Cookie | `Cookie: sid=<32位token>` |
| Bearer | `Authorization: Bearer <32位token>` |
| Query（兜底） | `?sid=<32位token>` |

登录：`POST /login`，参数 `uname` / `password` / `rememberme`。

### 0.3 最小调用示例

```bash
# 1. 登录，保存 cookie
curl -sS -c cookies.txt -X POST http://localhost:8888/login \
  -H 'Accept: application/json' \
  -d 'uname=alice&password=secret&rememberme=on'

# 2. 以 JSON 方式拉取题目列表
curl -sS -b cookies.txt \
  -H 'Accept: application/json' \
  'http://localhost:8888/d/system/p?page=1&limit=20'
```

### 0.4 全部路由索引

> 路径均为**系统域**下的写法；其它域需在开头加 `/d/:domainId` 前缀。
> 认证列：`—` 匿名；`登录` = `PRIV_USER_PROFILE`；`超管` = `PRIV_EDIT_SYSTEM`；其余为具体常量。
> 「方法」列中 `GET` 表示该类定义了 `get()`；`POST` 表示定义了 `post()` 或 `postXxx()`；`WS` 为 WebSocket `Connection`。

#### 路由索引表

##### 认证与用户（`handler/user.ts`、`handler/home.ts`）

| 方法 | 路径 | 路由名 | 处理器 | 认证 |
|------|------|--------|--------|------|
| GET / POST | `/login` | `user_login` | `UserLoginHandler` | — |
| GET | `/oauth/:type/login` | `user_oauth` | `OauthHandler` | — |
| GET | `/oauth/:type/callback` | `user_oauth_callback` | `OauthCallbackHandler` | — |
| GET / POST | `/user/sudo` | `user_sudo` | `UserSudoHandler` | 登录 |
| GET | `/user/tfa` | `user_tfa` | `UserTFAHandler` | — |
| GET / POST | `/user/webauthn` | `user_webauthn` | `UserWebauthnHandler` | — |
| GET / POST | `/register` | `user_register` | `UserRegisterHandler` | `PRIV_REGISTER_USER` |
| GET / POST | `/register/:code` | `user_register_with_code` | `UserRegisterWithCodeHandler` | `PRIV_REGISTER_USER` |
| GET / POST | `/logout` | `user_logout` | `UserLogoutHandler` | 登录 |
| GET / POST | `/lostpass` | `user_lostpass` | `UserLostPassHandler` | — |
| GET / POST | `/lostpass/:code` | `user_lostpass_with_code` | `UserLostPassWithCodeHandler` | — |
| **POST** | `/user/delete` | `user_delete` | `UserDeleteHandler` | 登录 |
| GET | `/user/:uid` | `user_detail` | `UserDetailHandler` | — |
| GET / POST | `/contestmode` | `contest_mode` | `ContestModeHandler` | 超管 |
| GET | `/` | `homepage` | `HomeHandler` | — |
| GET / POST | `/home/security` | `home_security` | `HomeSecurityHandler` | 登录 |
| GET | `/home/changeMail/:code` | `user_changemail_with_code` | `UserChangemailWithCodeHandler` | 登录 |
| GET / POST | `/home/settings/:category` | `home_settings` | `HomeSettingsHandler` | 登录 |
| **POST** | `/home/avatar` | `home_avatar` | `HomeAvatarHandler` | 登录 |
| GET / POST | `/home/domain` | `home_domain` | `HomeDomainHandler` | 登录 |
| GET / POST | `/home/domain/create` | `home_domain_create` | `HomeDomainCreateHandler` | `PRIV_CREATE_DOMAIN` |
| GET / POST | `/home/messages` | `home_messages` | `HomeMessagesHandler` | 登录 |
| WS | `/websocket` | `websocket_gateway` | `WebsocketEventsConnectionManagerHandler` | — |

##### 题目（`handler/problem.ts`、`handler/import.ts`、`handler/compat.ts`）

| 方法 | 路径 | 路由名 | 处理器 | 域权限 |
|------|------|--------|--------|--------|
| GET / POST | `/p` | `problem_main` | `ProblemMainHandler` | `PERM_VIEW_PROBLEM` |
| GET | `/problem/random` | `problem_random` | `ProblemRandomHandler` | `PERM_VIEW_PROBLEM` |
| GET / POST | `/p/:pid` | `problem_detail` | `ProblemDetailHandler` | — |
| GET / POST | `/p/:pid/submit` | `problem_submit` | `ProblemSubmitHandler` | `PERM_SUBMIT_PROBLEM` |
| GET / POST | `/p/:pid/hack/:rid` | `problem_hack` | `ProblemHackHandler` | `PERM_SUBMIT_PROBLEM` |
| GET / POST | `/p/:pid/edit` | `problem_edit` | `ProblemEditHandler` | — |
| **GET** | `/p/:pid/config` | `problem_config` | `ProblemConfigHandler` | — |
| GET / POST | `/p/:pid/files` | `problem_files` | `ProblemFilesHandler` | `PERM_VIEW_PROBLEM` |
| GET | `/p/:pid/file/:filename` | `problem_file_download` | `ProblemFileDownloadHandler` | — |
| GET / POST | `/p/:pid/solution` | `problem_solution` | `ProblemSolutionHandler` | `PERM_VIEW_PROBLEM` |
| GET / POST | `/p/:pid/solution/:sid` | `problem_solution_detail` | `ProblemSolutionHandler` | `PERM_VIEW_PROBLEM` |
| GET | `/p/:pid/solution/:psid/raw` | `problem_solution_raw` | `ProblemSolutionRawHandler` | `PERM_VIEW_PROBLEM` |
| GET | `/p/:pid/solution/:psid/:psrid/raw` | `problem_solution_reply_raw` | `ProblemSolutionRawHandler` | `PERM_VIEW_PROBLEM` |
| GET | `/p/:pid/stat` | `problem_statistics` | `ProblemStatisticsHandler` | `PERM_VIEW_PROBLEM` |
| GET / POST | `/problem/create` | `problem_create` | `ProblemCreateHandler` | `PERM_CREATE_PROBLEM` |
| GET / POST | `/problem/import/hydro` | `problem_import_hydro` | `ProblemImportHydroHandler` | `PERM_CREATE_PROBLEM` |
| **GET** | `/p/category/:category` | `problem_category_compat` | `ProblemCategoryCompatHandler` | — |

##### 提交与评测（`handler/record.ts`、`handler/judge.ts`、`handler/status.ts`）

| 方法 | 路径 | 路由名 | 处理器 | 认证 |
|------|------|--------|--------|------|
| **GET** | `/record` | `record_main` | `RecordListHandler` | — |
| GET / POST | `/record/:rid` | `record_detail` | `RecordDetailHandler` | — |
| WS | `/record-conn` | `record_conn` | `RecordMainConnectionHandler` | — |
| WS | `/record-detail-conn` | `record_detail_conn` | `RecordDetailConnectionHandler` | — |
| GET / POST | `/judge/files` | `judge_files_download` | `JudgeFilesDownloadHandler` | `PRIV_JUDGE` |
| **POST** | `/judge/upload` | `judge_files_upload` | `JudgeFileUpdateHandler` | `PRIV_JUDGE` |
| WS | `/judge/conn` | `judge_conn` | `JudgeConnectionHandler` | `PRIV_JUDGE` |
| **GET** | `/status` | `status` | `StatusHandler` | — |
| **POST** | `/status/update` | `status_update` | `StatusUpdateHandler` | — |

##### 比赛与作业（`handler/contest.ts`、`handler/homework.ts`）

| 方法 | 路径 | 路由名 | 处理器 | 域权限 |
|------|------|--------|--------|--------|
| GET / POST | `/contest/create` | `contest_create` | `ContestEditHandler` | — |
| **GET** | `/contest` | `contest_main` | `ContestListHandler` | `PERM_VIEW_CONTEST` |
| GET / POST | `/contest/team` | `contest_team` | `ContestTeamHandler` | 登录 |
| GET / POST | `/contest/:tid` | `contest_detail` | `ContestDetailHandler` | `PERM_VIEW_CONTEST` |
| GET / POST | `/contest/:tid/problems` | `contest_problemlist` | `ContestProblemListHandler` | `PERM_VIEW_CONTEST` |
| GET / POST | `/contest/:tid/edit` | `contest_edit` | `ContestEditHandler` | `PERM_VIEW_CONTEST` |
| GET / POST | `/contest/:tid/print` | `contest_print` | `ContestPrintHandler` | `PERM_VIEW_CONTEST` |
| GET / POST | `/contest/:tid/api/printing/team` | `contest_print_alt` | `ContestPrintHandler` | `PERM_VIEW_CONTEST` |
| GET / POST | `/contest/:tid/management` | `contest_manage` | `ContestManagementHandler` | — |
| GET / POST | `/contest/:tid/clarification` | `contest_clarification` | `ContestClarificationHandler` | — |
| **GET** | `/contest/:tid/code` | `contest_code` | `ContestCodeHandler` | `PERM_VIEW_CONTEST` |
| GET | `/contest/:tid/file/:type/:filename` | `contest_file_download` | `ContestFileDownloadHandler` | `PERM_VIEW_CONTEST` |
| GET / POST | `/contest/:tid/user` | `contest_user` | `ContestUserHandler` | `PERM_VIEW_CONTEST` |
| GET / POST | `/contest/:tid/balloon` | `contest_balloon` | `ContestBalloonHandler` | `PERM_VIEW_CONTEST` |
| GET / POST | `/contest/:tid/scoreboard` | `contest_scoreboard` | `ContestScoreboardHandler` | `PERM_VIEW_CONTEST_SCOREBOARD` |
| GET / POST | `/contest/:tid/scoreboard/:view` | `contest_scoreboard_view` | `ContestScoreboardHandler` | `PERM_VIEW_CONTEST_SCOREBOARD` |
| **GET** | `/homework` | `homework_main` | `HomeworkMainHandler` | `PERM_VIEW_HOMEWORK` |
| GET / POST | `/homework/create` | `homework_create` | `HomeworkEditHandler` | — |
| GET / POST | `/homework/:tid` | `homework_detail` | `HomeworkDetailHandler` | `PERM_VIEW_HOMEWORK` |
| GET / POST | `/homework/:tid/code` | `homework_code` | `ContestCodeHandler` | `PERM_VIEW_HOMEWORK` |
| GET / POST | `/homework/:tid/edit` | `homework_edit` | `HomeworkEditHandler` | — |
| GET / POST | `/homework/:tid/file` | `homework_files` | `HomeworkFilesHandler` | `PERM_VIEW_HOMEWORK` |
| GET | `/homework/:tid/file/:type/:filename` | `homework_file_download` | `ContestFileDownloadHandler` | `PERM_VIEW_HOMEWORK` |
| GET / POST | `/homework/:tid/scoreboard` | `homework_scoreboard` | `ContestScoreboardHandler` | `PERM_VIEW_HOMEWORK_SCOREBOARD` |
| GET / POST | `/homework/:tid/scoreboard/:view` | `homework_scoreboard_view` | `ContestScoreboardHandler` | `PERM_VIEW_HOMEWORK_SCOREBOARD` |

##### 训练、讨论、域、系统、文件

| 方法 | 路径 | 路由名 | 处理器 | 认证/域权限 |
|------|------|--------|--------|-------------|
| **GET** | `/training` | `training_main` | `TrainingMainHandler` | `PERM_VIEW_TRAINING` |
| GET / POST | `/training/create` | `training_create` | `TrainingEditHandler` | — |
| GET / POST | `/training/:tid` | `training_detail` | `TrainingDetailHandler` | `PERM_VIEW_TRAINING` |
| GET / POST | `/training/:tid/edit` | `training_edit` | `TrainingEditHandler` | — |
| GET / POST | `/training/:tid/file` | `training_files` | `TrainingFilesHandler` | `PERM_VIEW_TRAINING` |
| GET | `/training/:tid/file/:filename` | `training_file_download` | `TrainingFileDownloadHandler` | `PERM_VIEW_TRAINING` |
| **GET** | `/discuss` | `discussion_main` | `DiscussionMainHandler` | `PERM_VIEW_DISCUSSION` |
| GET / POST | `/discuss/:did` | `discussion_detail` | `DiscussionDetailHandler` | — |
| GET / POST | `/discuss/:did/edit` | `discussion_edit` | `DiscussionEditHandler` | — |
| GET | `/discuss/:did/raw` | `discussion_raw` | `DiscussionRawHandler` | — |
| GET | `/discuss/:did/:drid/raw` | `discussion_reply_raw` | `DiscussionRawHandler` | — |
| GET | `/discuss/:did/:drid/:drrid/raw` | `discussion_tail_reply_raw` | `DiscussionRawHandler` | — |
| **GET** | `/discuss/:type/:name` | `discussion_node` | `DiscussionNodeHandler` | — |
| GET / POST | `/discuss/:type/:name/create` | `discussion_create` | `DiscussionCreateHandler` | 登录 + `PERM_CREATE_DISCUSSION` |
| **GET** | `/ranking` | `ranking` | `DomainRankHandler` | `PERM_VIEW_RANKING` |
| GET / POST | `/domain/dashboard` | `domain_dashboard` | `DomainDashboardHandler` | — |
| GET / POST | `/domain/edit` | `domain_edit` | `DomainEditHandler` | — |
| GET / POST | `/domain/user` | `domain_user` | `DomainUserHandler` | — |
| GET / POST | `/domain/permission` | `domain_permission` | `DomainPermissionHandler` | — |
| GET / POST | `/domain/role` | `domain_role` | `DomainRoleHandler` | — |
| GET / POST | `/domain/group` | `domain_group` | `DomainUserGroupHandler` | — |
| GET / POST | `/domain/join_applications` | `domain_join_applications` | `DomainJoinApplicationsHandler` | — |
| GET / POST | `/domain/join` | `domain_join` | `DomainJoinHandler` | 登录 |
| **GET** | `/domain/search` | `domain_search` | `DomainSearchHandler` | 登录 |
| **GET** | `/manage` | `manage` | `SystemMainHandler` | 超管 |
| GET / POST | `/manage/dashboard` | `manage_dashboard` | `SystemDashboardHandler` | 超管 |
| GET / POST | `/manage/script` | `manage_script` | `SystemScriptHandler` | 超管 |
| GET / POST | `/manage/setting` | `manage_setting` | `SystemSettingHandler` | 超管 |
| GET / POST | `/manage/config` | `manage_config` | `SystemConfigHandler` | 超管 |
| GET / POST | `/manage/userimport` | `manage_user_import` | `SystemUserImportHandler` | 超管 |
| GET / POST | `/manage/userpriv` | `manage_user_priv` | `SystemUserPrivHandler` | 超管 |
| WS | `/manage/check-conn` | `manage_check` | `SystemCheckConnHandler` | 超管 |
| **GET** | `/language/:lang` | `switch_language` | `SwitchLanguageHandler` | — |
| GET / POST | `/file` | `home_files` | `FilesHandler` | — |
| **GET** | `/file/:uid/:filename` | `fs_download` | `FSDownloadHandler` | — |
| **GET** | `/storage` | `storage` | `StorageHandler` | — |
| **GET** | `/account/:uid` | `switch_account` | `SwitchAccountHandler` | 超管 |
| **POST** | `/heap-snapshot` | `heap_snapshot` | `HeapSnapshotHandler` | 超管 |

##### 新版 JSON-RPC 与 UI 辅助

| 方法 | 路径 | 路由名 | 处理器 | 认证 |
|------|------|--------|--------|------|
| GET / POST | `/api/:op` | `api` | `ApiHandler` | 由具体 op 决定 |
| WS | `/api/:op/conn` | `api_conn` | `ApiConnectionHandler` | 由具体 op 决定 |
| GET | `/manage/config/schema.json` | `config_schema` | `SystemConfigSchemaHandler` | 超管 |
| GET | `/wiki/help` | `wiki_help` | `WikiHelpHandler` | — |
| GET | `/wiki/about` | `wiki_about` | `WikiAboutHandler` | — |
| GET | `/set_theme/:theme` | `set_theme` | `SetThemeHandler` | — |
| GET | `/legacy` | `set_legacy` | `LegacyModeHandler` | — |
| GET / POST | `/markdown` | `markdown` | `MarkdownHandler` | — |
| GET / POST | `/media` | `media` | `RichMediaHandler` | — |
| GET | `/lazy/:version/:name` | `constant` | `UiConstantsHandler` | — |
| GET | `/resource/:version/:name` | `constant` | `UiConstantsHandler` | — |
| GET | `/plugins/:version/:name` | `ui_next_constants` | `UiNextConstantHandler` | — |

##### 插件注册的附加路由（可选启用）

| 方法 | 路径 | 路由名 | 来源包 | 认证 |
|------|------|--------|--------|------|
| GET | `/contest/:tid/scoreboard/xcpcio` | `contest_scoreboard_view`（view=`xcpcio`） | `scoreboard-xcpcio` | `PERM_VIEW_CONTEST_SCOREBOARD` |
| GET | `/contest/:tid/resolver-cdp/:token` | `contest_resolver_cdp` | `scoreboard-xcpcio` | token 校验 |
| GET / POST | `/contest/:tid/autosubmit` | `contest_autosubmit` | `onsite-toolkit` | — |
| GET | `/metrics` | `metrics` | `prom-client` | — |
| GET / POST | `/center/report` | `data_report` | `center` | — |
| GET / POST | `/problem/import/fps` | `problem_import_fps` | `fps-importer` | `PERM_CREATE_PROBLEM` |
| GET / POST | `/problem/import/hoj` | `problem_import_hoj` | `import-hoj` | `PERM_CREATE_PROBLEM` |
| GET / POST | `/problem/import/qduoj` | `problem_import_qduoj` | `import-qduoj` | `PERM_CREATE_PROBLEM` |
| GET / POST | `/blog/:uid` | `blog_main` | `blog` | — |
| GET / POST | `/blog/:uid/:did` | `blog_detail` | `blog` | — |
| GET / POST | `/onlyoffice-jwt` | `onlyoffice-jwt` | `onlyoffice` | — |

---

## 1. 通用约定

> 本章所有结论均直接来自源码，关键位置已标注文件与行号。
> 核心文件：`framework/framework/base.ts`、`framework/framework/server.ts`、`framework/framework/decorators.ts`、`framework/framework/validator.ts`、`framework/framework/api.ts`、`packages/hydrooj/src/service/layers/*.ts`。

### 1.1 基础地址与域（Domain）前缀

Hydro 是多租户 OJ，**所有业务路由都挂在「域」前缀下**：

| 场景 | URL 形态 | 说明 |
|------|----------|------|
| 系统域（`system`） | `/p/1000`、`/contest/...` | 域 ID 为 `system` 时可省略前缀 |
| 其它域 | `/d/:domainId/p/1000` | 显式域前缀，优先级最高 |
| 绑定域名的域 | `/p/1000` | 通过 `Host` 头反查域 |

域解析实现在 `packages/hydrooj/src/service/layers/domain.ts`：

1. 先匹配路径 `/^\/d\/([^/]+)\//`，命中则**剥离前缀**（`ctx.request.path` 会被改写），并强制使用该 `domainId`；
2. 未命中时用 `DomainModel.getByHost(host)` 按 `Host` 头（可被 `server.xhost` 指定的头覆盖）反查；
3. 反查结果与路径域不一致时返回 302 重定向到正确域前缀；
4. 域不存在时设置 `pendingError = NotFoundError`，并把 `domainId` 兜底为 `system`；
5. 命中黑名单（`blacklist` 集合中的 `ip::<ip>`）时直接返回文本 `blacklisted`。

> 注意：请求日志与 `this.request.originalPath` 保留**未剥离**的原始路径。

### 1.2 请求派发模型

每个路由由一个 Handler 类处理，方法名即 HTTP 方法：

| Handler 方法 | 触发条件 |
|--------------|----------|
| `async all(...)` | **存在时优先调用**，响应所有 HTTP 方法 |
| `async get(...)` | `GET` |
| `async post(...)` | `POST` |
| `async postXxx(...)` | `POST` 且请求体含 `operation: "xxx"` |
| `async __prepare(...)` / `prepare(...)` | 方法派发前的准备钩子 |
| `async after(...)` | 方法派发后的收尾钩子 |

`operation` 的转换规则（`framework/framework/server.ts:551-554`）：

```js
const operation = (method === 'post' && ctx.request.body?.operation)
    ? `_${ctx.request.body.operation}`.replace(/_([a-z])/g, (s) => s[1].toUpperCase())
    : '';
// operation = 'accept' -> 调用 h.postAccept()
```

**规则**：`operation` 首字母会被转成大写并拼到 `post` 后面。
例如 `operation: "accept"` → `postAccept`；`operation: "generate_testdata"` → `postGenerateTestdata`。

派发顺序（`server.ts:565-590`，`steps` 数组）：

```
init → prepare → all → <method> → [post<Operation>] → after → cleanup
```

若 `operation` 指定的方法不存在，抛 `InvalidOperationError`（405）；
若 `get`/`post` 方法不存在，抛 `MethodNotAllowedError`（405）。

### 1.3 参数来源与类型校验

参数装饰器定义在 `framework/framework/decorators.ts`：

| 装饰器 | `source` | 取值对象（源码 `decorators.ts:81-87`） |
|--------|----------|--------------------------------------|
| `@param(name, ...)` | `all` | **合并后的 `this.args`**：`{ domainId, ...路径参数, ...query, ...body }` |
| `@query(name, ...)` / `@get(name, ...)` | `get` | 仅 `this.request.query` |
| `@post(name, ...)` | `post` | 仅 `this.request.body` |
| `@route(name, ...)` | `route` | `{ ...this.request.params, domainId }`（URL 路径参数） |

> 所以 `@param` 既能读 query 也能读 body（body 优先），而 `@query`/`@post` 是严格区分的。

**关键规则**：

1. 装饰器**书写顺序（自上而下）= 方法形参顺序**；`domainId` 隐含为**第一个形参**（若首参名以 `domainId` 开头）。其余形参按装饰器顺序依次传入。
2. 形参顺序示例：
   ```ts
   @param('page', Types.PositiveInt, true)
   @param('q', Types.Content, true)
   async get(domainId: string, page: number, q: string) { }
   ```
3. 第三个参数 `isOptional`：
   - `true` → 缺省时传 `undefined`，不做校验；
   - `false`/省略 → **必填**，缺失或校验失败抛 `ValidationError`（403）。
4. 第四个参数可以是 `null`（跳过）或自定义校验函数 `(v) => boolean`；
5. 第五个参数是转换函数 `(v) => T`。
6. `domainId` 为空字符串时会直接抛 `ValidationError('domainId')`。
7. 请求路径参数同时会被合并进 `this.args`（见 1.4）。

#### 1.3.1 `Types` 类型表（`framework/framework/validator.ts`）

| 类型 | 输出 | 校验规则 |
|------|------|----------|
| `Types.Content` | `string` | trim 后非空，长度 < 65536 |
| `Types.Key` | `string` | `/^[\w-]{1,255}$/`（saslprep） |
| `Types.Name` | `string` | 1–255 字符（**已废弃**） |
| `Types.Username` | `string` | 3–31 字符，或 2 个汉字 |
| `Types.UidOrName` | `string` | 3–31 字符 / 2 汉字 / 纯数字（可带 `-`） |
| `Types.Password` | `string` | 长度 6–255 |
| `Types.Email` | `string` | 标准邮箱正则 |
| `Types.Filename` | `string` | 不含 `\/?#~!\|*`，且 `sanitize-filename` 后不变 |
| `Types.DomainId` | `string` | `/^[a-zA-Z]\w{3,31}$/`（首字母 + 4–32 位） |
| `Types.ProblemId` | `string \| number` | 可带 `prefix-` 前缀；纯数字时转为 `number` |
| `Types.Role` | `string` | 1–31 位 `\w` 或汉字 |
| `Types.Title` | `string` | 1–64 字符且非空白 |
| `Types.ShortString` / `Types.String` | `string` | 1–255 / 任意非空 |
| `Types.Int` | `number` | 可带符号整数，安全整数范围内 |
| `Types.UnsignedInt` | `number` | `-0` 或非负整数 |
| `Types.PositiveInt` | `number` | 正整数（`/^\+?[1-9][0-9]*$/`） |
| `Types.Float` | `number` | 有限浮点数 |
| `Types.Boolean` | `boolean` | **可选**；`false`/`off`/`no`/`0` 为假，其余为真 |
| `Types.ObjectId` | `ObjectId` | `ObjectId.isValid` |
| `Types.Date` | `string` | `YYYY-M-D` → 规范化为 `YYYY-MM-DD` |
| `Types.Time` | `string` | `H:M` → 规范化为 `HH:MM` |
| `Types.Range([...])` | 枚举值 | 值必须命中给定数组/对象 key |
| `Types.NumericArray` | `number[]` | 字符串按 `,` 拆分后逐个转数字 |
| `Types.CommaSeperatedArray` | `string[]` | 字符串按 `,` 拆分 |
| `Types.ArrayOf(type, isOptional)` | `T[]` | 单值自动包装成数组 |
| `Types.AnyOf(...types)` | 联合 | 命中任一类型 |
| `Types.Set` | `Set` | 单值/数组 → Set |
| `Types.Emoji` | `string` | 首个 emoji |
| `Types.Any` | `any` | 不校验 |

### 1.4 统一请求上下文 `this.args`

`packages/hydrooj/src/service/layers/base.ts:31-33`：

```ts
const args = {
    domainId, ...ctx.params, ...ctx.query, ...ctx.request.body, __start: Date.now(),
};
```

- 合并优先级：**路径参数 < query < body**（后者覆盖前者）；
- `this.args.__start` 是请求开始时间戳；
- 该对象同时作为 `log`/`oplog` 的参数记录来源（`packages/hydrooj/src/model/oplog.ts:47-56`）。

### 1.5 响应格式

响应由 `framework/framework/base.ts` 的中间件统一处理，**是否返回 JSON 取决于请求头 `Accept`**：

```ts
json: (ctx.request.headers.accept || '').includes('application/json'),
```

#### 1.5.1 输出分支（`base.ts:66-100`）

| 条件 | 输出 |
|------|------|
| `response.type` 已被 Handler 显式设置 | 原样输出 |
| `response.pjax` 存在且请求带 `pjax` 参数 | `application/json`，`{ "fragments": [{ "html": "..." }] }` |
| `request.json`（`Accept: application/json`） | **原始 JSON**：`JSON.stringify(response.body, serializer)` |
| `response.redirect` 存在 | 同上走 JSON 分支（`body.url` 会被写入重定向地址） |
| `?noTemplate=1` | 原始 JSON |
| `response.template` 为空 | 原始 JSON |
| 其它（有 `response.template`） | 渲染 HTML 模板，`Content-Type: text/html` |

> **重要**：**任何** Handler 路由只要带 `Accept: application/json` 就能拿到 JSON 数据。
> 这是把 Hydro 当纯 API 使用时的核心手段。

#### 1.5.2 二进制与重定向

| 方式 | 行为 |
|------|------|
| `this.binary(buffer, filename)` | 设置 `Content-Type`/`Content-Disposition`，返回二进制流 |
| `this.response.attach(filename, streamOrBuffer)` | 同上，支持流 |
| `this.response.redirect = url` | **非 JSON 请求**时返回 `302` + `Location`；**JSON 请求**时返回 `200` + `{ "url": "<目标地址>" }` |
| `this.response.disposition = 'attachment; filename="..."'` | 设置 `Content-Disposition` 头 |
| `this.response.addHeader(name, value)` | 自定义响应头 |

#### 1.5.3 `X-Hydro-Inject` 注入头（`base.ts:83-96`）

请求头 `X-Hydro-Inject` 可注入额外上下文（逗号分隔）：

| 值 | 效果 |
|----|------|
| `pageName` | 响应头 `x-hydro-page` = 路由名，`x-hydro-template` = 模板名 |
| `UiContext` | 响应体追加 `UiContext` 字段 |
| `UserContext` | 响应体追加 `UserContext` 字段 |
| `routeMap` | 响应体追加 `routeMap` 字段（全部路由名 → 路径映射） |

#### 1.5.4 缓存

- 若 Handler 设置 `this.response.etag`，会写入 `ETag` 头并附加 `Cache-Control: public`；
- 请求头 `If-None-Match` 命中时返回 **304**；
- 只要请求经过 Handler，响应都会带 `Vary: Accept`。

### 1.6 错误响应

错误由 `base.ts:110-125` 统一捕获：

```ts
const error = errorMessage(err);
response.status = error instanceof UserFacingError ? error.code : 500;
if (request.json) response.body = { error };
```

#### 1.6.1 JSON 模式下的错误体（**实测**）

`errorMessage()` 返回的是 Error 对象本身，`JSON.stringify` 只会序列化**自有可枚举属性**，因此实际响应体为：

```json
{
  "error": {
    "params": ["alice"],
    "name": "LoginError",
    "code": 403
  }
}
```

> ⚠️ 注意两个易踩坑点：
> 1. **没有 `message` 字段** —— `HydroError.message` 是原型上的 getter，不可枚举，不会被序列化；错误文案需要客户端根据 `name` + `params` 自行渲染。
> 2. **没有 `stack` 字段** —— `stack` 在 V8 中是非枚举自有属性。
>
> 非 JSON 请求时改为渲染 `error.html`（`UserFacingError`）或 `bsod.html`（系统错误）。

#### 1.6.2 错误基类与状态码（`framework/framework/error.ts`）

| 类 | `code` |
|----|--------|
| `UserFacingError` | 400 |
| `BadRequestError` | 400 |
| `ForbiddenError` | 403 |
| `NotFoundError` | 404 |
| `MethodNotAllowedError` | 405 |
| `ValidationError`（继承 `ForbiddenError`） | 403 |
| `CsrfTokenError` | 403 |
| `InvalidOperationError`（继承 `MethodNotAllowedError`） | 405 |
| `FileTooLargeError` | 403 |
| `SystemError` | 500 |

#### 1.6.3 常用业务错误（`packages/hydrooj/src/error.ts`）

| `name` | 状态码 | 触发场景 |
|--------|--------|----------|
| `PrivilegeError` | 403 | 未登录（`params` 含 `PRIV_USER_PROFILE`，文案 `You're not logged in.`）或缺少 PRIV |
| `PermissionError` | 403 | 缺少域权限，文案 `You don't have the required permission ({0}) in this domain.` |
| `LoginError` | 403 | 密码错误，`params = [uname]` |
| `UserAlreadyExistError` | 403 | 用户名已存在 |
| `UserNotFoundError` | 404 | 用户不存在 |
| `ContestNotFoundError` / `ProblemNotFoundError` / `RecordNotFoundError` | 404 | 资源不存在 |
| `ContestNotAttendedError` | 403 | 未参加比赛 |
| `ContestAlreadyAttendedError` | 403 | 已参加比赛 |
| `ContestNotLiveError` | 403 | 比赛未开始 |
| `ContestAlreadyStartedError` | 403 | 比赛已开始 |
| `ContestNotEndedError` | 403 | 比赛未结束 |
| `ContestScoreboardHiddenError` | 403 | 榜单不可见 |
| `HomeworkNotLiveError` / `HomeworkNotAttendedError` | 403 | 作业未开放 / 未认领 |
| `TrainingAlreadyEnrollError` | 403 | 已报名训练 |
| `ProblemNotAllowLanguageError` | 403 | 该语言不允许提交 |
| `ProblemNotAllowPretestError` | 403 | 该题不支持自测 |
| `HackFailedError` | 403 | Hack 失败 |
| `OpcountExceededError` | 403 | 操作频率超限，`params = [op, periodSecs, maxOperations]` |
| `InvalidTokenError` | 403 | Token 无效（邮件验证码/找回密码等） |
| `BlacklistedError` | 403 | IP/用户被拉黑 |
| `DomainJoinForbiddenError` / `DomainJoinAlreadyMemberError` / `InvalidJoinInvitationCodeError` | 403 | 域加入相关 |
| `CannotDeleteSystemDomainError` / `OnlyOwnerCanDeleteDomainError` | 400 | 域删除相关 |
| `ProblemConfigError` | 400 | `config.yaml` 非法 |
| `PretestRejudgeFailedError` / `HackRejudgeFailedError` | 400 | 不能重测自测/Hack 记录 |
| `RequireProError` | 403 | 需要 Pro 授权 |
| `RemoteOnlineJudgeError` | 500 | 远程评测失败 |

### 1.7 认证与会话

实现在 `packages/hydrooj/src/service/layers/base.ts:36-40`：

```ts
const header = ctx.request.headers.authorization;
const sid = header
    ? header.split(' ')[1]          // Authorization: Bearer <token>
    : ctx.cookies.get('sid') || ctx.query.sid;
const session = sid ? await token.get(sid, token.TYPE_SESSION) : null;
```

#### 1.7.1 三种凭据传递方式

| 方式 | 写法 | 说明 |
|------|------|------|
| **Cookie（推荐）** | `Cookie: sid=<token>` | 登录后自动下发；HttpOnly **false** |
| **Bearer** | `Authorization: Bearer <token>` | 取空格分隔后的第 2 段，等价于 `sid` |
| **Query** | `?sid=<token>` | 兜底方案，主要用于 WebSocket / 分享连接 |

> ⚠️ `Authorization` 头一旦出现，就**完全覆盖** Cookie，即使格式不对也不会回退到 `sid` Cookie。

#### 1.7.2 会话存储

- 会话存储在 MongoDB `token` 集合（`packages/hydrooj/src/model/token.ts`），`tokenType = 0`（`TYPE_SESSION`）；
- Token ID 为 32 位随机字符串（`randomstring(32)`）；
- **滑动过期**：每次请求结束后都会刷新 `expireAt`；
  - 勾选「记住我」（`session.save`）→ `session.saved_expire_seconds`
  - 否则 → `session.unsaved_expire_seconds`
- 会话内容变更才写库；5 分钟内的会话若未变更则跳过更新（`isRecent` 判断）；
- Cookie 属性：`expires` = 过期时间，`httpOnly: false`；若配置了 `session.domain` 且请求为 HTTPS 且 Host 匹配，则附加 `domain` / `SameSite=None` / `Secure`；
- 服务端**未**设置 CSRF token 校验中间件，但 `CsrfTokenError` 已定义，部分 Handler 会自行校验。

#### 1.7.3 会话字段（`TokenDoc`，用于登录态判断）

| 字段 | 说明 |
|------|------|
| `_id` | 会话 ID（即 `sid`） |
| `uid` | 用户 ID，`0` = 未登录 |
| `scope` | 会话权限范围（`bigint` 字符串），匿名时为 `PERM_ALL` |
| `sudo` / `sudoArgs` | 二次验证（sudo）时间戳与待执行操作 |
| `save` | 是否「记住我」 |
| `viewLang` | 界面语言 |
| `createAt` / `updateAt` / `expireAt` | 时间戳 |
| `createIp` / `createUa` / `createHost` / `updateIp` / `updateUa` | 环境信息 |
| `recreate` | 标记需要重建会话 |

#### 1.7.4 匿名用户

- 未登录时 `ctx.session = { uid: 0, scope: PERM_ALL }`；
- `packages/hydrooj/src/service/layers/user.ts` 会以 `uid=0` 取域内用户对象，因此**匿名用户在业务域内仍拥有 `guest` 角色**，其权限由域配置决定；
- 匿名用户的 `viewLang` 字段会被删除；
- 已登录用户每次请求若 IP 变化会更新 `loginip`（需 `PRIV_USER_PROFILE`）。

#### 1.7.5 sudo（二次验证）

`packages/hydrooj/src/service/server.ts:51-70` 的 `@requireSudo` 装饰器：

- 会话中 `sudo` 时间戳在 1 小时内有效；
- 否则把当前请求暂存到 `session.sudoArgs`，并**重定向**到 `user_sudo`（`/user/sudo`）；
- 二次验证通过后，`UserSudoHandler.post` 会把 `this.session.sudoArgs` 作为响应体返回，由前端重放原请求。

### 1.8 权限模型：PRIV 与 PERM

Hydro 有**两套独立**的权限体系（定义见 `packages/common/permission.ts`）：

| 体系 | 类型 | 作用范围 | 校验方法 |
|------|------|----------|----------|
| **PRIV** | `number`（32 位位掩码） | **全局**，用户级 | `this.checkPriv(...)` |
| **PERM** | `bigint`（64+ 位掩码） | **域内**，角色级 | `this.checkPerm(...)` |

路由注册时的校验器（`framework/framework/server.ts:745-770`）会区分二者：

```ts
ctx.Route('contest_main', '/contest', ContestListHandler, PERM.PERM_VIEW_CONTEST);
//                                              ^ bigint -> checkPerm
ctx.Route('user_logout', '/logout', UserLogoutHandler, PRIV.PRIV_USER_PROFILE);
//                                              ^ number -> checkPriv
```

#### 1.8.1 隐式 `PERM_VIEW`（非常重要）

除显式声明 `noCheckPermView` 的处理器外，**所有 HTTP 路由都会隐式要求 `PERM.PERM_VIEW`**（`packages/hydrooj/src/service/server.ts:275`）：

```ts
on('handler/create/http', async (h) => {
    ...
    if ((!('noCheckPermView' in h) || !h.noCheckPermView)
        && !h.user.hasPriv(PRIV.PRIV_VIEW_ALL_DOMAIN)) h.checkPerm(PERM.PERM_VIEW);
    if (h.context.pendingError) throw h.context.pendingError;
});
```

- 即：客户端若在某域内连 `PERM_VIEW` 都没有，**任何**接口都会返回 `PermissionError` / `PrivilegeError`；
- 拥有 `PRIV_VIEW_ALL_DOMAIN` 的账号（通常是超管）可绕过该检查；
- 同一处还有全局限流 `h.limitRate('global', 5, 100)`（5 秒内最多 100 次请求），`--benchmark` 模式与 `notUsage` 处理器除外。

#### 1.8.2 `checkPriv` / `checkPerm` 行为（`packages/hydrooj/src/service/server.ts:173-181`）

```ts
checkPerm(...args) {
    if (!this.user.hasPerm(...args)) {
        if (this.user.hasPriv(PRIV.PRIV_USER_PROFILE)) throw new PermissionError(...args);
        throw new PrivilegeError(PRIV.PRIV_USER_PROFILE);   // 未登录
    }
},
checkPriv(...args) {
    if (!this.user.hasPriv(...args)) throw new PrivilegeError(...args);
},
```

> 因此**域权限失败时**：未登录 → `PrivilegeError`（`You're not logged in.`）；已登录 → `PermissionError`。

#### 1.8.3 PRIV 常量完整表

| 常量 | 位 | 值 |
|------|----|----|
| `PRIV_NONE` | — | 0 |
| `PRIV_EDIT_SYSTEM` | `1 << 0` | 1 |
| `PRIV_SET_PERM` | `1 << 1` | 2 |
| `PRIV_USER_PROFILE` | `1 << 2` | 4 |
| `PRIV_REGISTER_USER` | `1 << 3` | 8 |
| `PRIV_READ_PROBLEM_DATA` | `1 << 4` | 16 |
| `PRIV_READ_RECORD_CODE` | `1 << 7` | 128 |
| `PRIV_VIEW_HIDDEN_RECORD` | `1 << 8` | 256 |
| `PRIV_JUDGE` | `1 << 9` | 512 |
| `PRIV_CREATE_DOMAIN` | `1 << 10` | 1024 |
| `PRIV_VIEW_ALL_DOMAIN` | `1 << 11` | 2048 |
| `PRIV_MANAGE_ALL_DOMAIN` | `1 << 12` | 4096 |
| `PRIV_REJUDGE` | `1 << 13` | 8192 |
| `PRIV_VIEW_USER_SECRET` | `1 << 14` | 16384 |
| `PRIV_VIEW_JUDGE_STATISTICS` | `1 << 15` | 32768 |
| `PRIV_CREATE_FILE` | `1 << 16` | 65536 |
| `PRIV_UNLIMITED_QUOTA` | `1 << 17` | 131072 |
| `PRIV_DELETE_FILE` | `1 << 18` | 262144 |
| `PRIV_NEVER` | `1 << 20` | 1048576 |
| `PRIV_UNLIMITED_ACCESS` | `1 << 22` | 4194304 |
| `PRIV_VIEW_SYSTEM_NOTIFICATION` | `1 << 23` | 8388608 |
| `PRIV_SEND_MESSAGE` | `1 << 24` | 16777216 |
| `PRIV_MOD_BADGE` | `1 << 25` | 33554432 |
| `PRIV_ALL` | — | -1 |

**默认值**：`PRIV_DEFAULT = PRIV_USER_PROFILE + PRIV_CREATE_FILE + PRIV_SEND_MESSAGE`（= 4 + 65536 + 16777216）。
`PRIV_DEFAULT` 是**注册用户的默认全局权限**，即「已登录 + 可上传文件 + 可发私信」。

#### 1.8.4 PERM 常量完整表

| 分组 | 常量 | 位 | 值 |
|------|------|----|----|
| 域 | `PERM_NONE` | — | 0 |
| 域 | `PERM_VIEW` | `1<<0` | 1 |
| 域 | `PERM_EDIT_DOMAIN` | `1<<1` | 2 |
| 域 | `PERM_MOD_BADGE` | `1<<2` | 4 |
| 域 | `PERM_VIEW_USER_PRIVATE_INFO`（同 `PERM_VIEW_DISPLAYNAME`，已废弃别名） | `1<<67` | 2^67 |
| 题目 | `PERM_CREATE_PROBLEM` | `1<<4` | 16 |
| 题目 | `PERM_EDIT_PROBLEM` | `1<<5` | 32 |
| 题目 | `PERM_EDIT_PROBLEM_SELF` | `1<<6` | 64 |
| 题目 | `PERM_VIEW_PROBLEM` | `1<<7` | 128 |
| 题目 | `PERM_VIEW_PROBLEM_HIDDEN` | `1<<8` | 256 |
| 题目 | `PERM_SUBMIT_PROBLEM` | `1<<9` | 512 |
| 题目 | `PERM_READ_PROBLEM_DATA` | `1<<10` | 1024 |
| 记录 | `PERM_READ_RECORD_CODE` | `1<<12` | 4096 |
| 记录 | `PERM_REJUDGE_PROBLEM` | `1<<13` | 8192 |
| 记录 | `PERM_REJUDGE` | `1<<14` | 16384 |
| 记录 | `PERM_READ_RECORD_CODE_ACCEPT` | `1<<66` | 2^66 |
| 记录 | `PERM_VIEW_RECORD` | `1<<70` | 2^70 |
| 题解 | `PERM_VIEW_PROBLEM_SOLUTION` | `1<<15` | 32768 |
| 题解 | `PERM_CREATE_PROBLEM_SOLUTION` | `1<<16` | 65536 |
| 题解 | `PERM_VOTE_PROBLEM_SOLUTION` | `1<<17` | 131072 |
| 题解 | `PERM_EDIT_PROBLEM_SOLUTION` | `1<<18` | 262144 |
| 题解 | `PERM_EDIT_PROBLEM_SOLUTION_SELF` | `1<<19` | 524288 |
| 题解 | `PERM_DELETE_PROBLEM_SOLUTION` | `1<<20` | 1048576 |
| 题解 | `PERM_DELETE_PROBLEM_SOLUTION_SELF` | `1<<21` | 2097152 |
| 题解 | `PERM_REPLY_PROBLEM_SOLUTION` | `1<<22` | 4194304 |
| 题解 | `PERM_EDIT_PROBLEM_SOLUTION_REPLY_SELF` | `1<<24` | 16777216 |
| 题解 | `PERM_DELETE_PROBLEM_SOLUTION_REPLY` | `1<<25` | 33554432 |
| 题解 | `PERM_DELETE_PROBLEM_SOLUTION_REPLY_SELF` | `1<<26` | 67108864 |
| 题解 | `PERM_VIEW_PROBLEM_SOLUTION_ACCEPT` | `1<<65` | 2^65 |
| 讨论 | `PERM_VIEW_DISCUSSION` | `1<<27` | 134217728 |
| 讨论 | `PERM_CREATE_DISCUSSION` | `1<<28` | 268435456 |
| 讨论 | `PERM_HIGHLIGHT_DISCUSSION` | `1<<29` | 536870912 |
| 讨论 | `PERM_EDIT_DISCUSSION` | `1<<30` | 1073741824 |
| 讨论 | `PERM_EDIT_DISCUSSION_SELF` | `1<<31` | 2147483648 |
| 讨论 | `PERM_DELETE_DISCUSSION` | `1<<32` | 4294967296 |
| 讨论 | `PERM_DELETE_DISCUSSION_SELF` | `1<<33` | 8589934592 |
| 讨论 | `PERM_REPLY_DISCUSSION` | `1<<34` | 17179869184 |
| 讨论 | `PERM_EDIT_DISCUSSION_REPLY_SELF` | `1<<36` | 68719476736 |
| 讨论 | `PERM_DELETE_DISCUSSION_REPLY` | `1<<38` | 274877906944 |
| 讨论 | `PERM_DELETE_DISCUSSION_REPLY_SELF` | `1<<39` | 549755813888 |
| 讨论 | `PERM_DELETE_DISCUSSION_REPLY_SELF_DISCUSSION` | `1<<40` | 1099511627776 |
| 讨论 | `PERM_PIN_DISCUSSION` | `1<<61` | 2^61 |
| 讨论 | `PERM_ADD_REACTION` | `1<<62` | 2^62 |
| 讨论 | `PERM_LOCK_DISCUSSION` | `1<<64` | 2^64 |
| 比赛 | `PERM_VIEW_CONTEST` | `1<<41` | 2199023255552 |
| 比赛 | `PERM_VIEW_CONTEST_SCOREBOARD` | `1<<42` | 4398046511104 |
| 比赛 | `PERM_VIEW_CONTEST_HIDDEN_SCOREBOARD` | `1<<43` | 8796093022208 |
| 比赛 | `PERM_CREATE_CONTEST` | `1<<44` | 17592186044416 |
| 比赛 | `PERM_ATTEND_CONTEST` | `1<<45` | 35184372088832 |
| 比赛 | `PERM_EDIT_CONTEST` | `1<<50` | 1125899906842624 |
| 比赛 | `PERM_EDIT_CONTEST_SELF` | `1<<51` | 2251799813685248 |
| 比赛 | `PERM_VIEW_HIDDEN_CONTEST` | `1<<68` | 2^68 |
| 训练 | `PERM_VIEW_TRAINING` | `1<<46` | 70368744177664 |
| 训练 | `PERM_CREATE_TRAINING` | `1<<47` | 140737488355328 |
| 训练 | `PERM_EDIT_TRAINING` | `1<<48` | 281474976710656 |
| 训练 | `PERM_EDIT_TRAINING_SELF` | `1<<49` | 562949953421312 |
| 训练 | `PERM_PIN_TRAINING` | `1<<63` | 2^63 |
| 作业 | `PERM_VIEW_HOMEWORK` | `1<<52` | 4503599627370496 |
| 作业 | `PERM_VIEW_HOMEWORK_SCOREBOARD` | `1<<53` | 9007199254740992 |
| 作业 | `PERM_VIEW_HOMEWORK_HIDDEN_SCOREBOARD` | `1<<54` | 18014398509481984 |
| 作业 | `PERM_CREATE_HOMEWORK` | `1<<55` | 36028797018963968 |
| 作业 | `PERM_ATTEND_HOMEWORK` | `1<<56` | 72057594037927936 |
| 作业 | `PERM_EDIT_HOMEWORK` | `1<<57` | 144115188075855872 |
| 作业 | `PERM_EDIT_HOMEWORK_SELF` | `1<<58` | 288230376151711744 |
| 作业 | `PERM_VIEW_HIDDEN_HOMEWORK` | `1<<69` | 2^69 |
| 排名 | `PERM_VIEW_RANKING` | `1<<59` | 576460752303423488 |
| 保留 | `PERM_NEVER` | `1<<60` | 1152921504606846976 |
| 占位 | `PERM_ALL` / `PERM_ADMIN` | — | -1（全权限） |

**预置权限组合**：

- `PERM_BASIC` = `PERM_VIEW` + `PERM_VIEW_PROBLEM` + `PERM_VIEW_PROBLEM_SOLUTION` + `PERM_VIEW_PROBLEM_SOLUTION_ACCEPT` + `PERM_VIEW_DISCUSSION` + `PERM_VIEW_CONTEST` + `PERM_VIEW_CONTEST_SCOREBOARD` + `PERM_VIEW_HOMEWORK` + `PERM_VIEW_HOMEWORK_SCOREBOARD` + `PERM_VIEW_TRAINING` + `PERM_VIEW_RANKING`（只读）
- `PERM_DEFAULT` = `PERM_BASIC` 再加：`PERM_VIEW_USER_PRIVATE_INFO`、`PERM_EDIT_PROBLEM_SELF`、`PERM_SUBMIT_PROBLEM`、`PERM_CREATE_PROBLEM_SOLUTION`、`PERM_VOTE_PROBLEM_SOLUTION`、`PERM_EDIT_PROBLEM_SOLUTION_SELF`、`PERM_DELETE_PROBLEM_SOLUTION_SELF`、`PERM_REPLY_PROBLEM_SOLUTION`、`PERM_EDIT_PROBLEM_SOLUTION_REPLY_SELF`、`PERM_DELETE_PROBLEM_SOLUTION_REPLY_SELF`、`PERM_CREATE_DISCUSSION`、`PERM_EDIT_DISCUSSION_SELF`、`PERM_REPLY_DISCUSSION`、`PERM_ADD_REACTION`、`PERM_EDIT_DISCUSSION_REPLY_SELF`、`PERM_DELETE_DISCUSSION_REPLY_SELF`、`PERM_DELETE_DISCUSSION_REPLY_SELF_DISCUSSION`、`PERM_ATTEND_CONTEST`、`PERM_EDIT_CONTEST_SELF`、`PERM_ATTEND_HOMEWORK`、`PERM_EDIT_HOMEWORK_SELF`、`PERM_CREATE_TRAINING`、`PERM_EDIT_TRAINING_SELF`、`PERM_VIEW_RECORD`
- `PERM_ADMIN` = `PERM_ALL`

> `bigint` 序列化：JSON 输出时统一转成字符串 `"BigInt::<十进制值>"`（见 `framework/framework/serializer.ts`）。

### 1.9 分页

Hydro **不使用** HOJ 那种 `{ records, total, size, current, pages }` 结构。

`packages/hydrooj/src/service/server.ts:170-172` 提供 Handler 方法：

```ts
paginate<T>(cursor: FindCursor<T>, page: number, key: string | number)
    : Promise<[docs: T[], numPages: number, count: number]>
```

底层实现 `packages/hydrooj/src/service/db.ts:153-166`：

```ts
if (page <= 0) throw new ValidationError('page');
const [count, pageDocs] = await Promise.all([...count..., cursor.skip((page-1)*pageSize).limit(pageSize).toArray()]);
const numPages = Math.floor((count + pageSize - 1) / pageSize);
return [pageDocs, numPages, count];
```

- `pageSize` 由 `setting.pagination.<key>` 决定（如 `pagination.problem`），未配置时默认 **20**；
- 也可以直接传数字作为每页条数；
- **响应字段命名约定**：
  - `page` → 当前页码；
  - `<prefix>pcount` → 总页数（如 `tpcount`、`ppcount`、`upcount`、`dpcount`）；
  - 数据列表字段名由 Handler 自定（`tdocs`、`pdocs`、`rdocs`、`ddocs`、`udocs`、`docs` 等）。

### 1.10 序列化规则（`framework/framework/serializer.ts`）

所有 JSON 响应都会经过该 `replacer`：

| 规则 | 说明 |
|------|------|
| 键名以 `_` 开头（`_id` 除外） | **被丢弃**（因此 `_udoc`、`_tdoc` 等内部字段不会出现在响应中） |
| `bigint` | 序列化为字符串 `"BigInt::<值>"` |
| 对象含 `serialize()` 方法 | 调用 `obj.serialize(handler)` 并用其结果替换 |

> 这条规则解释了为什么 `PERM` 权限、`docId` 等 bigint 字段在 JSON 里是字符串。
> `_id` 会被保留，但通常是 `ObjectId`，序列化为 24 位十六进制字符串。

### 1.11 路由名 → URL 的生成

- 服务端：`this.url(routeName, ...kwargs)`（`packages/hydrooj/src/service/server.ts:126-160`）；
- 路由映射表：`server.routeMap`，`GET` 时可通过 `X-Hydro-Inject: routeMap` 注入到响应体；
- `this.url('#')` 返回 `'#'`（占位）。

### 1.12 限流

`this.limitRate(op, periodSecs, maxOperations, defaultKey?)`（`packages/hydrooj/src/service/server.ts:185-195`）：

- 拥有 `PRIV_UNLIMITED_ACCESS` 的用户直接跳过；
- 可通过系统设置 `limit.<op>` 覆盖默认阈值；
- 默认 key 模板为 `{{ip}}@{{user}}`（当 `limit.by_user` 为真）或 `{{ip}}`；
- 超限抛 `OpcountExceededError`。

### 1.13 常见调用示例

#### 1.13.1 登录并保持会话（Cookie）

```http
POST /login
Content-Type: application/x-www-form-urlencoded
Accept: application/json

uname=alice&password=secret&rememberme=on
```

响应头：

```http
Set-Cookie: sid=<32位随机串>; Expires=...; Path=/
Content-Type: application/json
```

```json
{
  "UserContext": { "_id": 2, "uname": "alice", "priv": 16777220, "..." : "..." }
}
```

后续请求带上该 `sid`：

```http
GET /p/1000
Accept: application/json
Cookie: sid=<32位随机串>
```

#### 1.13.2 使用 Bearer Token

```http
GET /d/system/p/1000
Accept: application/json
Authorization: Bearer <32位随机串>
```

#### 1.13.3 纯 JSON API 通用模板

```bash
curl -sS 'http://localhost:8888/d/system/p' \
  -H 'Accept: application/json' \
  -H 'Cookie: sid=xxx' \
  --data 'page=1&limit=20' -G | jq .
```

#### 1.13.4 错误处理

```json
{
  "error": {
    "params": ["PRIV_USER_PROFILE"],
    "name": "PrivilegeError",
    "code": 403
  }
}
```

---

## 2. 认证、会话与用户账户

### 认证与会话（源码级详解）

> 本节是第 1 章「[1.1 基础地址与域前缀](#11-基础地址与域domain前缀)」「[1.7 认证与会话](#17-认证与会话)」的**展开详解**，包含更多实现细节与边界行为。
> 需要快速查阅时请直接看第 1 章；本节用于排查具体实现问题。

本节内容来自以下源码，未做推测：

- `packages/hydrooj/src/service/layers/base.ts`、`layers/domain.ts`、`layers/user.ts`
- `packages/hydrooj/src/service/server.ts`
- `framework/framework/base.ts`、`framework/framework/server.ts`、`framework/framework/decorators.ts`、`framework/framework/validator.ts`
- `packages/hydrooj/src/model/token.ts`、`model/user.ts`、`model/oauth.ts`、`model/setting.ts`

#### 路由与域（domain）前缀

路由由 `ctx.Route(name, path, HandlerClass, ...permPrivChecker)` 注册，注册时传入的 `path` 是**完整路径**（例如 `/login`、`/p/:pid`），框架不会再自动加 `/p` 前缀。

- 系统域（`system`）：直接访问 `path`，例如 `GET /login`、`GET /user/2`。
- 其它域：把 `/d/:domainId` 前置到 `path` 之前，例如 `GET /d/mydomain/login`。

框架生成 URL 时（`Handler#url`）也遵循同一规则：仅当目标域不是「根域」时才加 `/d/:domainId` 前缀。

#### 域（domain）解析顺序

`packages/hydrooj/src/service/layers/domain.ts`（服务端层，HTTP 与 WebSocket 都会执行）：

1. **路径前缀 `/d/:domainId/`**（正则 `^\/d\/([^/]+)\//`）：优先级最高，会从 `ctx.request.path` 中剥离该前缀（`ctx.originalPath` 保留原值）。
2. **Host 头**：若配置了 `server.xhost`（安装脚本会设为 `x-forwarded-host`），则取该请求头的值，否则取 `ctx.request.host`；再用 `DomainModel.getByHost(host)` 反查域。
3. 都取不到时回退到 `system` 域。

补充行为：

- 若按 Host 推断出的域与路径中指定的域**不一致**，会 `302` 重定向到把路径中的域替换为推断域的地址（`ctx.redirect`），本次请求不继续处理。
- 若路径中指定的域不存在，则记录 `ctx.pendingError = NotFoundError(domainId)`，并在 `handler/create/http` 阶段抛出（返回 404）；此时 `ctx.domainId` 回退为 `system`。
- 请求 IP 命中黑名单（`BlackListModel.get('ip::<ip>')`）时直接返回纯文本 `blacklisted`（HTTP 200），不进入任何处理器。
- 请求 IP 的取值：优先取 `server.xff` 指定的头（取逗号分隔的第一段），否则 `ctx.request.ip`。

> 本仓库**没有** `X-Hydro-Domain` 请求头，也**不支持**用 `?domainId=` 决定「当前域」。`domainId` 只作为 URL 生成参数、路由参数（`/d/:domainId`）与 API 入参（`args.domainId`）出现。

#### 认证方式

##### Session Cookie（`sid`）

`packages/hydrooj/src/service/layers/base.ts`：

```ts
const header = ctx.request.headers.authorization;
const sid = header
    ? header.split(' ')[1]                 // Bearer token
    : ctx.cookies.get('sid') || ctx.query.sid;
const session = sid ? await token.get(sid, token.TYPE_SESSION) : null;
ctx.session = Object.create(session || { uid: 0, scope: PERM.PERM_ALL.toString() });
```

- Cookie 名称：**`sid`**。
- Cookie 值：session 文档的 `_id`（32 位随机字符串，`randomstring(32)`）。
- **不签名**：`ctx.cookies.set('sid', ..., { expires, httpOnly: false })`，没有使用 koa 的签名机制（`session.keys` 虽然会传给 Koa，但 `signed` 未开启），因此 `sid` 就是原始 token 值。
- Cookie 选项：
  - `httpOnly: false`；
  - `expires = now + expireSeconds`；
  - 当 `system.get('session.domain')` 非空、请求为 HTTPS 且 host 以该域结尾时，追加 `domain=<session.domain>`、`sameSite: 'none'`、`secure: true`。
- WebSocket 请求不会下发 Cookie（`if (!request.websocket)`）。

**有效期（滑动过期）**：`expireSeconds` 取决于 `session.save`：

| 场景 | 配置项 | 默认值 |
|---|---|---|
| 登录时 `rememberme` 为真（`session.save = true`） | `session.saved_expire_seconds` | `3600 * 24 * 30`（30 天） |
| 其它情况 | `session.unsaved_expire_seconds` | `3600 * 3`（3 小时） |

每次请求结束后，`base.ts` 都会把 session 写回 `token` 集合：

- 若本次请求**没有修改会话**（`ctx.session` 无自有属性）且会话在 5 分钟内更新过（`isRecent`），则跳过写库。
- 会话发生变化时才写回：`ctx.session._id` 存在且未标记 `recreate` → `token.update(_id, TYPE_SESSION, expireSeconds, ...)`，刷新 `updateAt`/`expireAt`（滑动过期）；无 `_id`（或标记了 `recreate`）→ 先删除旧 token，再 `token.add(TYPE_SESSION, expireSeconds, ...)` 生成新的 `_id`。

**Session 存储模型**（`packages/hydrooj/src/model/token.ts`，集合 `token`）：

| 字段 | 说明 |
|---|---|
| `_id` | session id（Cookie 值） |
| `tokenType` | `0`（`TokenModel.TYPE_SESSION`） |
| `uid` | 登录用户 uid；匿名/登出为 0 |
| `scope` | 权限掩码字符串（默认 `PERM.PERM_ALL.toString()`） |
| `save` | 是否「记住我」 |
| `createAt` / `updateAt` / `expireAt` | 时间戳；`expireAt` 上有 TTL 索引（`expireAfterSeconds: 0`）自动清理 |
| `createIp` / `createUa` / `createHost` / `updateIp` / `updateUa` | 会话来源信息 |
| `sudo` / `sudoUid` / `sudoArgs` | sudo 提权状态 |
| `viewLang` | 会话级界面语言覆盖 |
| `oauthBind` / `oauthRedirect` | OAuth 绑定/回跳临时状态 |
| `challenge` / `webauthnVerify` | WebAuthn 挑战值 |

Token 类型常量：`TYPE_SESSION=0`、`TYPE_REGISTRATION=2`、`TYPE_CHANGEMAIL=3`、`TYPE_OAUTH=4`、`TYPE_LOSTPASS=5`、`TYPE_EXPORT=6`、`TYPE_IMPORT=7`、`TYPE_WEBAUTHN=8`；`TYPE_TEXTS` 为对应的英文名（`Session`、`Registration`、`Change Email`、`OAuth`、`Lost Password`、`Export`、`Import`、`WebAuthn`）。

##### `Authorization: Bearer <token>`

```ts
const header = ctx.request.headers.authorization;
const sid = header
    ? header.split(' ')[1]          // Authorization: Bearer <token>
    : ctx.cookies.get('sid') || ctx.query.sid;
const session = sid ? await token.get(sid instanceof Array ? sid[0] : sid, token.TYPE_SESSION) : null;
```

- 请求头一旦存在就**完全覆盖** Cookie 与查询参数：即使格式非法（取不到第 2 段）也**不会**回退到 Cookie；`base.ts` 直接取空格分隔的**第 2 段**作为 `sid`。
- token 就是 `token` 集合中 `tokenType = 0`（Session）的文档 `_id`，与 Cookie 中的 `sid` 完全等价。
- 校验方式：`token.get(sid, TYPE_SESSION)`；查不到即视为匿名（`uid = 0`），不会报错。
- 该 token 由服务端在每次会话写回时生成，**没有独立的「API Token」类型**；注册、改邮箱、找回密码、WebAuthn 等其它类型的 token 不能用于认证。
- 优先级：`Authorization` 头 > `sid` Cookie > `?sid=` 查询参数（`?sid=` 主要给跨域/无法带 Cookie 的场景，如 WebSocket 与 SharedWorker）。

##### WebSocket

见后文「实时事件接口（`websocket_gateway`）」。要点：

- 连接地址为 `ws(s)://<host>/websocket`（`UiContext.ws_prefix + 'websocket'`，`ws_prefix` 由 `server.ws` 配置，默认 `/`）。
- 跨域 Cookie 不可用时用 `?sid=<sessionId>` 传递会话。
- 订阅频道时还可以在消息体中携带 `credential`（session id），见 `connection.ts#subscribe`。
- 网关模式（受信内部连接）使用请求头 `x-hydro-websocket-gateway: <websocket.secret>` 鉴权。

#### 会话对象与匿名用户

- `ctx.session` 通过 `Object.create(session || { uid: 0, scope: PERM.PERM_ALL.toString() })` 创建，即**未登录时 session 只有 `uid = 0` 与 `scope = PERM_ALL`**。
- 用户层（`layers/user.ts`）用 `ctx.session.uid` 与 `ctx.session.scope` 调用 `UserModel.getById(domainId, uid, scope)` 得到**域内**用户对象，因此匿名用户（uid 0）在业务域内仍拥有该域的 `guest` 角色；若查不到用户则重置为 `uid = 0` 并重新获取。
- 匿名用户会被删除 `viewLang` 字段；已登录用户（拥有 `PRIV_USER_PROFILE`）在请求 IP 变化时会更新 `loginip`（`user.setById(uid, { loginip })`）。
- `ctx.HydroContext.user = await user.private()`：这是模板与 JSON 注入使用的 `UserContext`。
- 匿名用户对象为 `UserModel.defaultUser`（uid 0）：

```json
{
  "_id": 0,
  "uname": "Unknown User",
  "avatar": "gravatar:unknown@hydro.local",
  "mail": "unknown@hydro.local",
  "priv": 0,
  "perm": 0n,
  "role": "default",
  "regat": "2000-01-01T00:00:00.000Z",
  "loginat": "2000-01-01T00:00:00.000Z"
}
```

（`perm` 为 bigint，JSON 输出时被序列化为字符串 `"BigInt::0"`。）

`UserContext` 序列化字段（`User#getFields`）：

- 公开字段：`_id`、`uname`、`mail`、`perm`、`role`、`priv`、`regat`、`loginat`、`avatar`、`avatarUrl`，以及带 `FLAG_PUBLIC` 的偏好字段（内置仅有 `badge`）；`avatarUrl` 仅在已计算时出现。
- 私有字段（调用者拥有 `PERM_VIEW_USER_PRIVATE_INFO`，或对象是 `User#private()` 的结果）额外包含 `school`、`studentId`、`phone`、`displayName` 等 `FLAG_PRIVATE` 字段。

> `_udoc`、`_dudoc`、`_salt`、`_hash`、`_tfa`、`_authenticators` 等以下划线开头的内部字段永远不会出现在 JSON 输出中（序列化器会丢弃除 `_id` 外的所有下划线字段）。

#### 权限校验与未授权行为

**路由级校验**（`framework/framework/server.ts#register` 的 `Checker`）：`ctx.Route(name, path, H, ...permPrivChecker)` 中第 4 个及以后的参数支持三种形式，可混用，执行顺序为「自定义 checker → `checkPerm` → `checkPriv`」：

| 形式 | 含义 |
|---|---|
| `number`（如 `PRIV.PRIV_USER_PROFILE`） | 调用 `this.checkPriv(priv)`，用户 priv 位必须包含该位 |
| `bigint`（如 `PERM.PERM_VIEW_PROBLEM`） | 调用 `this.checkPerm(perm)`，域内权限位必须包含该位 |
| 函数 | 作为自定义校验函数直接调用（`this` 为 handler） |

**隐式域权限**：`packages/hydrooj/src/service/server.ts` 的 `handler/create/http` 钩子中：

```ts
if ((!('noCheckPermView' in h) || !h.noCheckPermView) && !h.user.hasPriv(PRIV.PRIV_VIEW_ALL_DOMAIN)) {
    h.checkPerm(PERM.PERM_VIEW);
}
```

即：处理器若**未**声明 `noCheckPermView = true`，且当前用户没有 `PRIV_VIEW_ALL_DOMAIN`，则额外要求域权限 `PERM_VIEW`。本文档表格中「域权限」列的 `PERM_VIEW（隐式）` 即指这一条。

**常用权限常量**（`packages/common/permission.ts`；`PRIV` 是全局的 number 位掩码，`PERM` 是域内的 bigint 位掩码）：

| 常量 | 值 | 含义 |
|---|---|---|
| `PRIV.PRIV_EDIT_SYSTEM` | `1 << 0` | 系统超级管理员 |
| `PRIV.PRIV_USER_PROFILE` | `1 << 2` | 已登录用户 |
| `PRIV.PRIV_REGISTER_USER` | `1 << 3` | 允许注册新账号 |
| `PRIV.PRIV_JUDGE` | `1 << 9` | 评测机账号 |
| `PRIV.PRIV_CREATE_DOMAIN` | `1 << 10` | 允许创建域 |
| `PRIV.PRIV_VIEW_ALL_DOMAIN` | `1 << 11` | 可访问所有域（跳过隐式 `PERM_VIEW`） |
| `PRIV.PRIV_MANAGE_ALL_DOMAIN` | `1 << 12` | 管理所有域 |
| `PRIV.PRIV_UNLIMITED_ACCESS` | `1 << 22` | 不受频率限制 |
| `PRIV.PRIV_SEND_MESSAGE` | `1 << 24` | 允许发送站内私信 |
| `PRIV.PRIV_DEFAULT` | 组合值 | 注册用户的默认权限：`PRIV_USER_PROFILE + PRIV_CREATE_FILE + PRIV_SEND_MESSAGE`（即 `default.priv` 设置的默认值） |

**失败时的异常与 HTTP 状态码**：

| 校验 | 抛出的错误 | 状态码 |
|---|---|---|
| `checkPriv` 失败 | `PrivilegeError` | 403（消息：未登录时为 `You're not logged in.`） |
| `checkPerm` 失败（已登录） | `PermissionError` | 403 |
| `checkPerm` 失败（未登录） | `PrivilegeError(PRIV_USER_PROFILE)` | 403 |
| `@param` 取值/校验失败 | `ValidationError` | 403 |
| 密码错误 | `LoginError` | 403 |
| 被封禁 | `BlacklistedError` | 403 |
| 频率超限 | `OpcountExceededError` | 403 |
| 资源不存在 | `UserNotFoundError` / `NotFoundError` | 404 |

**关键差异：未登录时的重定向**（`packages/hydrooj/src/service/server.ts` 的 `onerror` 混入）：

```ts
if (this.user?._id === 0 && (error instanceof PermissionError || error instanceof PrivilegeError)) {
    this.response.redirect = this.url('user_login', {
        query: { redirect: (this.context.originalPath || this.request.path) + this.context.search },
    });
} else if (!this.user._dudoc.join && error instanceof PermissionError) {
    this.response.redirect = this.url('domain_join', { domainId: 'system', query: { redirect, target } });
} else { /* 正常错误响应 */ }
```

- **未登录**访问需要登录/权限的接口 → 重定向到 `/login?redirect=<原路径+查询串>`（不会抛出 403 错误体）。
- **已登录但未加入该域**且权限不足 → 重定向到 `/d/system/domain/join?...`。
- 由于框架规定「JSON 请求不做 302 跳转」（见下），**`Accept: application/json` 时以上重定向表现为 HTTP 200 + `{"url": "..."}`**，客户端需自行识别 `url` 字段。

#### JSON 模式与错误响应

`framework/framework/base.ts`（`request.json = (Accept 头包含 'application/json')`）：

响应形态按以下顺序判定，命中即停止：

1. `response.type` 已由处理器设置 → 原样输出；
2. `response.pjax` 存在且请求带 `pjax` → `application/json`，响应体为 `{ fragments: [{ html }] }`（`response.pjax` 中每个模板渲染结果一项）；
3. `request.json || response.redirect || request.query.noTemplate || !response.template` → `JSON.stringify(response.body, serializer(false, handler))`，`Content-Type: application/json`（`Accept` 是**子串匹配**且大小写敏感；`?noTemplate=1` 也能强制原始 JSON）；
4. 否则渲染 `response.template` 指定的模板（`text/html`）。

其它响应约定：

- 只要有 handler，响应都会带 `Vary: Accept`。
- `X-Hydro-Inject` 请求头可注入额外字段：`pageName`（写响应头 `x-hydro-page` / `x-hydro-template`）、`UiContext`（`body.UiContext`）、`UserContext`（`body.UserContext`）、`routeMap`（`body.routeMap`）；请求头值会被转小写后比对 `pagename` / `uicontext` / `usercontext` / `routemap`。
- JSON 序列化使用 `serializer`：丢弃除 `_id` 外所有 `_` 开头的键；`bigint` → 字符串 `"BigInt::<十进制值>"`；对象若带 `serialize()` 方法则调用它（如 `User#serialize`）。

**业务错误**的 JSON 结构：

```json
{ "error": { "params": ["alice"], "name": "LoginError", "code": 403 } }
```

- `errorMessage()` 返回 Error 对象本身，`JSON.stringify` 只序列化**自有可枚举属性**，因此只有 `params`（构造参数数组）、`name`、`code`；**没有 `message`、没有 `stack`**（`message` 是原型上的 getter，`stack` 是非枚举属性）。
- HTTP 状态码 = `error.code`（`UserFacingError` 子类为 400/403/404/405/500 等），非 `UserFacingError` 一律 500。
- 非 JSON 请求渲染 `error.html`（`UserFacingError`）或 `bsod.html`（其它异常）。

**重定向**：`this.response.redirect = url` 时

- HTML 模式：HTTP **302** + `Location`；
- JSON 模式：HTTP **200**，响应体为 `{"url": "<redirect>"}`（`response.body.url` 由框架写入）。

**ETag / 304**：设置 `this.response.etag` 会写入 `ETag` 响应头与 `Cache-Control: public`；若请求的 `If-None-Match` 与该值相同，则返回 **304**。

**二进制响应**：`this.binary(buffer, filename)` → `Content-Type: application/octet-stream`，并在传入 `filename` 时设置 `Content-Disposition: attachment; filename="..."`（RFC 5987 编码）。

**CSRF 防护**：`Handler#init` 对 POST 请求校验 `Referer` 的 host 必须等于请求 host（CORS 命中或 `allowCors = true` 除外），否则抛 `CsrfTokenError`（403）。

#### 参数来源约定

`framework/framework/decorators.ts` + `base.ts`：

- 非装饰器方法（如 `async post(args)`）收到的 `args` 是 `{ domainId, ...路径参数, ...查询参数, ...请求体, __start }` 的合并对象，**请求体覆盖查询参数**。
- `@param(name, Types.X[, isOptional])`：source = `all`，即从上述合并对象取值（query 或 body 均可）。
- `@query` / `@get`：仅从查询串取值；`@post`：仅从请求体取值；`@route`：从路径参数取值（并带上 `domainId`）。
- 装饰器书写顺序（自上而下）即方法形参顺序；若方法第一个形参名以 `domainId` 开头（忽略大小写），框架会自动把 `domainId` 作为第一个实参传入。

常用 `Types`（`framework/framework/validator.ts`）：

| 类型 | 校验/转换 |
|---|---|
| `Types.String` | 任意非空字符串 |
| `Types.ShortString` | 1–255 字符 |
| `Types.Content` | 去首尾空白，长度 < 65536 |
| `Types.Title` | 1–64 字符且去空白后非空 |
| `Types.Username` | 3–31 字符或 2 个汉字（经 SASLprep） |
| `Types.Password` | 6–255 字符 |
| `Types.Email` | 邮箱正则（经 SASLprep） |
| `Types.DomainId` | `^[a-zA-Z]\w{3,31}$` |
| `Types.Key` | `^[\w-]{1,255}$` |
| `Types.Int` | 有符号整数且为安全整数 |
| `Types.PositiveInt` | 正整数 |
| `Types.ObjectId` | 合法 ObjectId 字符串 |
| `Types.Boolean` | **自带 `isOptional = true`**（`[(v) => !!(v && !['false','off','no','0'].includes(v)), null, true]`），因此 `@param('x', Types.Boolean)` 天然可选；`'false'/'off'/'no'/'0'` 为 false，其它非空值为 true |
| `Types.Range([...])` | 取值必须在给定集合内 |
| `Types.Any` | 不做校验 |

#### POST 子操作（`operation`）派发

`framework/framework/server.ts#handleHttp`（约 548-560 行）对 POST 请求做子操作派发：

```ts
const operation = (method === 'post' && ctx.request.body?.operation)
    ? `_${ctx.request.body.operation}`.replace(/_([a-z])/g, (s) => s[1].toUpperCase())
    : '';
```

- 请求体中带 `operation` 时，框架会在其前面拼一个 `_`，再把每个 `_x` 转为大写 `X`，最后拼到 `post` 之后作为要调用的方法名。因此**下划线写法与驼峰写法都能命中同一方法**：`change_password` → `_change_password` → `ChangePassword` → `postChangePassword`；`delete_all_tokens` → `postDeleteAllTokens`。
- 目标方法不存在 → `InvalidOperationError`（405，`MethodNotAllowedError` 的子类）；未带 `operation` 且处理器没有普通 `post` 方法 → 同样 405。
- `operation` **只能从请求体读取**（`ctx.request.body.operation`），查询串中的 `operation` 不生效。
- 子操作在步骤链中的位置为 `handler/before-operation/...` → `post<Operation>` → `after` → `cleanup`；子方法与普通方法一样支持 `@param` 等装饰器（`domainId` 仍为第一个形参）。
- 本文档各接口的 operation 名称统一按源码方法名转下划线写法给出（例如 `postChangePassword` → `change_password`）。

#### 分页约定

`packages/hydrooj/src/service/db.ts#paginate` 与 `packages/hydrooj/src/service/server.ts` 的 `Handler#paginate` 混入：

```ts
paginate<T>(cursor: FindCursor<T>, page: number, key: string | number): Promise<[docs: T[], numPages: number, count: number]>
```

- `page` 从 1 开始；`page <= 0` 抛 `ValidationError('page')`（403）。
- 第三个参数为字符串时，页大小取设置项 `pagination.<key>`（未配置时默认 20）；为数字时直接作为页大小。
- 返回值：`[当页文档, 总页数, 总记录数]`，其中 `numPages = ceil(count / pageSize)`。

响应中**统一使用 `page`（当前页）+ `<prefix>pcount`（总页数）** 的命名，**没有** `records` / `total` / `size` / `current` / `pages` 这类结构；前缀随处理器而不同：

| 处理器示例 | 字段 |
|---|---|
| `problem_main` | `page`、`ppcount`（总页数）、`pcount`（总记录数） |
| `contest_main` / `training_main` | `page`、`tpcount` |
| `discussion_main` | `page`、`dpcount`、`dcount` |
| `record_main` | `page`、`pcount`、`rscount` |

#### 频率限制

每个 HTTP 处理器创建时都会执行 `h.limitRate('global', 5, 100)`（5 秒内最多 100 次，按 `{{ip}}@{{user}}` 计数；`limit.by_user` 决定 key 用 `{{ip}}` 还是 `{{ip}}@{{user}}`），超过则抛 `OpcountExceededError`。拥有 `PRIV_UNLIMITED_ACCESS` 的用户跳过限制。各接口另有自己的 `limitRate` 调用，见下文各接口说明。

---

### 用户与会话接口（`packages/hydrooj/src/handler/user.ts`）

路由注册（`user.ts:634-649`）：

```ts
ctx.Route('user_login', '/login', UserLoginHandler);
ctx.Route('user_oauth', '/oauth/:type/login', OauthHandler);
ctx.Route('user_sudo', '/user/sudo', UserSudoHandler, PRIV.PRIV_USER_PROFILE);
ctx.Route('user_tfa', '/user/tfa', UserTFAHandler);
ctx.Route('user_webauthn', '/user/webauthn', UserWebauthnHandler);
ctx.Route('user_oauth_callback', '/oauth/:type/callback', OauthCallbackHandler);
ctx.Route('user_register', '/register', UserRegisterHandler, PRIV.PRIV_REGISTER_USER);
ctx.Route('user_register_with_code', '/register/:code', UserRegisterWithCodeHandler, PRIV.PRIV_REGISTER_USER);
ctx.Route('user_logout', '/logout', UserLogoutHandler, PRIV.PRIV_USER_PROFILE);
ctx.Route('user_lostpass', '/lostpass', UserLostPassHandler);
ctx.Route('user_lostpass_with_code', '/lostpass/:code', UserLostPassWithCodeHandler);
ctx.Route('user_delete', '/user/delete', UserDeleteHandler, PRIV.PRIV_USER_PROFILE);
ctx.Route('user_detail', '/user/:uid', UserDetailHandler);
if (system.get('server.contestmode')) ctx.Route('contest_mode', '/contestmode', ContestModeHandler, PRIV.PRIV_EDIT_SYSTEM);
```

#### 登录 · `user_login`

```http
GET /login
POST /login
```

| 项 | 值 |
|---|---|
| 处理器 | `UserLoginHandler`（user.ts:52，`get` 56 行，`post` 72 行） |
| 认证 | 匿名 |
| 域权限 | —（`noCheckPermView = true`） |
| 响应 | GET：HTML 模板 `user_login.html`；POST：重定向（JSON 模式为 `{"url": ...}`） |

**GET 参数**（`@param`，query 或 body）

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `redirect` | string | 否 | 登录成功后回跳地址；模板会原样输出 |

**GET 响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `redirect` | string | 回跳地址 |
| `builtInLogin` | boolean | `server.login`，是否允许站内账号密码登录 |
| `loginMethods` | array | 可用第三方登录方式，元素为 `{ id, icon, text, name }`（来自 `ctx.oauth.providers` 中未标记 `hidden` 的提供方） |

**POST 请求参数**（`@param`，query 或 body 皆可；请求体优先）

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `uname` | `Types.Username` | 是 | **用户名或邮箱**（先按邮箱查，再按用户名查） |
| `password` | `Types.Password` | 是 | 明文密码（6–255 字符） |
| `rememberme` | `Types.Boolean` | 否 | 默认 `false`；为真时 `session.save = true`，会话有效期变为 `session.saved_expire_seconds`（默认 30 天） |
| `redirect` | string | 否 | 登录成功后的跳转地址 |
| `tfa` | string | 否 | 两步验证 TOTP 动态码（6 位，SHA1/30 秒窗口） |
| `authnChallenge` | string | 否 | WebAuthn 挑战 token（`token.TYPE_WEBAUTHN`，由 `GET /user/webauthn?login=1` 签发） |
| `judge` | `Types.Boolean` | 否 | 评测机登录模式；为真时允许在关闭站内登录的情况下以 `PRIV_JUDGE` 账号登录 |

**POST 处理流程**（精确顺序，`user.ts:72-110`）：

1. `judge` 为假且 `server.login` 为假 → `BuiltinLoginError`；`judge` 为真但 `server.login` 为假且用户无 `PRIV_JUDGE` → `BuiltinLoginError`。
2. 依次按邮箱（`user.getByEmail`）、用户名（`user.getByUname`）查用户；查不到 → `UserNotFoundError(uname)`。
3. 若 `system.contestmode` 为真且用户不是超级管理员：`_loginip` 与当前 IP 不一致 → `ValidationError('ip')`；`contestmode === 'strict'` 时若同一 IP 已有其它账号登录 → `ValidationError('ip')`。
4. 频率限制：`user_login`（60 秒 30 次）、`user_login_id`（60 秒 5 次，按 `uname` 计数）；同时写操作日志 `user.login`。
5. 两步验证：用户启用了 `tfa` 时必须提供正确 `tfa`；启用了 `authn` 时必须提供 `authnChallenge`（token 的 `uid` 必须匹配且 `verified` 必须为真，验证后立即删除）；两者都未提供 → `ValidationError('2FA', 'Authn')`。
6. `udoc.checkPassword(password)`：按 `hashType` 取 `global.Hydro.module.hash[...]` 计算并比对；不匹配 → `LoginError(uname)`。若 `hashType !== 'hydro'`（历史哈希），校验成功后会自动用新算法重写密码。
7. 用户缺少 `PRIV_USER_PROFILE`（被封禁）→ `BlacklistedError(uname, banReason)`。
8. `successfulAuth`：写 `loginat`/`loginip`，触发 `auth/before-login`、`auth/login` 钩子，记录 `user.loginSuccess` 操作日志，并重置 session：`uid`、`viewLang = ''`、`sudo = null`、`sudoUid = null`、`scope = PERM_ALL`、`oauthBind = null`、`recreate = true`（下次写回时重新生成 session id，防会话固定攻击）。
9. `session.save = rememberme`，随后设置重定向：`redirect` 参数 → `Referer`（若不以 `/login` 结尾）→ `url('homepage')`。

**密码哈希**（`packages/hydrooj/src/lib/hash.hydro.ts`，`hashType = 'hydro'`）：

```ts
crypto.pbkdf2(password, salt, 100000, 64, 'sha256', (err, key) => key.toString('hex').substring(0, 64))
```

即 **PBKDF2-SHA256，100000 轮，派生 64 字节，取十六进制前 64 个字符**；`salt` 为 `randomstring()` 生成的随机串，存于 `udoc.salt`。

**POST 响应**：无业务字段，仅重定向；JSON 模式下为：

```json
{ "url": "/" }
```

**错误**

| 错误 | 状态码 | 触发条件 |
|---|---|---|
| `BuiltinLoginError` | 403 | `server.login` 关闭（且非评测机账号） |
| `UserNotFoundError` | 404 | 用户名/邮箱不存在 |
| `ValidationError` | 403 | 比赛模式 IP 校验失败、2FA/WebAuthn 参数缺失或挑战未验证 |
| `InvalidTokenError` | 403 | 2FA 动态码错误或 WebAuthn 挑战 token 无效 |
| `LoginError` | 403 | 密码错误 |
| `BlacklistedError` | 403 | 账号被封禁 |
| `OpcountExceededError` | 403 | 触发登录频率限制 |

**示例**

```http
POST /login
Accept: application/json
Content-Type: application/json

{ "uname": "hydro", "password": "secret123", "rememberme": true }
```

```json
{ "url": "/" }
```

#### OAuth 登录入口 · `user_oauth`

```http
GET /oauth/:type/login
```

| 项 | 值 |
|---|---|
| 处理器 | `OauthHandler`（user.ts:469，`get` 474 行） |
| 认证 | 匿名 |
| 域权限 | —（`noCheckPermView = true`） |
| 响应 | 由提供方决定，通常是重定向（JSON 模式为 `{"url": ...}`） |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `type` | `Types.Key`（`^[\w-]{1,255}$`） | 提供方名称，如 `github`、`google`、`oidc`、`mail` |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `redirect` | string | 否 | 写入 `session.oauthRedirect`，OAuth 回调成功后回跳该地址 |

**行为**：设置 `session.oauthRedirect` 后调用 `this.ctx.oauth.providers[type]?.get.call(this)`。提供方来自插件（`login-with-github`、`login-with-google`、`login-with-oidc` 等）；内置注册的 `mail` 提供方 `get()` 直接抛 `NotFoundError`（404）。若 `type` 不存在，则不做任何事，响应为空对象 `{}`。

`OAuthProvider` 结构（`model/oauth.ts`）：`{ text, name, icon?, hidden?, get(), callback(), canRegister?, lockUsername? }`。

#### OAuth 回调 · `user_oauth_callback`

```http
GET /oauth/:type/callback
```

| 项 | 值 |
|---|---|
| 处理器 | `OauthCallbackHandler`（user.ts:480，`get` 483 行） |
| 认证 | 匿名 |
| 域权限 | —（`noCheckPermView = true`） |
| 响应 | 重定向（JSON 模式为 `{"url": ...}`） |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `type` | string（**未做类型校验**，方法内直接读 `args.type`） | 提供方名称 |

**行为**（按顺序判断）：

1. 提供方不存在 → `UserFacingError('Oauth type')`（400）；频率限制 `oauth_callback`（60 秒 5 次）。
2. `provider.callback.call(this, args)` 返回 `OAuthUserResponse`：`{ _id, email, avatar?, bio?, uname?, viewLang?, set?, setInDomain? }`（`_id` 可以是数组，表示同一平台多个 ID）。
3. 若 `session.oauthBind === type`（即从「账号安全」页发起的绑定）：若该 ID 已绑定到其它账号 → `BadRequestError('Already bound to another account')`（400）；否则写入绑定关系，跳转 `session.oauthRedirect || url('home_security')`。
4. 否则若 ID 已有绑定 → 以该 uid 登录，跳转 `oauthRedirect || url('homepage')`。
5. 否则若邮箱已存在对应账号 → 绑定该 ID 并登录。
6. 否则若 `provider.canRegister` 为假 → `ForbiddenError('No bound account found')`（403）。
7. 否则校验 `PRIV_REGISTER_USER`，检查邮箱域名黑名单（`BlacklistedError`），从 `uname` 候选中挑选未被占用的用户名，创建 `TYPE_REGISTRATION` token（有效期 `session.unsaved_expire_seconds`），重定向到 `url('user_register_with_code', { code })`。

#### Sudo 提权 · `user_sudo`

```http
GET /user/sudo
POST /user/sudo
```

| 项 | 值 |
|---|---|
| 处理器 | `UserSudoHandler`（user.ts:112，`get` 113 行，`post` 121 行） |
| 认证 | 需登录（`PRIV_USER_PROFILE`） |
| 域权限 | `PERM_VIEW`（隐式） |
| 响应 | GET：HTML 模板 `user_sudo.html`；POST：模板 `user_sudo_redirect.html` 或重定向 |

**GET 行为**：若 `session.sudoArgs.method` 不存在 → `ForbiddenError`（403）；否则渲染 `user_sudo.html`（JSON 模式下响应体为 `{}`）。

**POST 参数**（`@param`，query 或 body）

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `password` | string | 否 | 当前账号密码 |
| `tfa` | string | 否 | 两步验证 TOTP 动态码 |
| `authnChallenge` | string | 否 | WebAuthn 挑战 token |

**POST 行为**：`session.sudoArgs.method` 不存在 → `ForbiddenError`；频率限制 `user_sudo`（60 秒 5 次，按 `{{user}}` 计数）并记录 `user.sudo` 日志。校验顺序：若用户启用了 `authn` 且提供 `authnChallenge` → 校验 WebAuthn 挑战；否则若启用了 `tfa` 且提供 `tfa` → 校验 TOTP；否则 `checkPassword(password)`。成功后 `session.sudo = Date.now()`，有效期 **1 小时**（`requireSudo` 装饰器判定 `Date.now() - session.sudo < Time.hour`）。

- 若原请求方法不是 `GET`：渲染 `user_sudo_redirect.html`，`response.body = session.sudoArgs`（字段 `method`、`referer`、`args`、`redirect`），由前端表单重新提交原请求。
- 若原请求是 `GET`：`response.redirect = session.sudoArgs.redirect`。
- 最后清空 `session.sudoArgs.method`。

**`@requireSudo` 装饰器**（`service/server.ts#requireSudo`）：被装饰的方法在 sudo 有效期内会先恢复 `session.sudoArgs.referer` 到请求头、清空 `sudoArgs` 再执行；否则记录 `sudoArgs = { method, referer, args, redirect: request.originalPath }`，设置 `response.redirect = url('user_sudo')` 并返回 `'cleanup'`（请求流程直接跳到 `cleanup` 步骤，原方法体不会执行）。`home_security` 的 GET 与部分 POST 子操作带此装饰器。

#### 两步验证状态查询 · `user_tfa`

```http
GET /user/tfa
```

| 项 | 值 |
|---|---|
| 处理器 | `UserTFAHandler`（user.ts:144，`get` 148 行） |
| 认证 | 匿名 |
| 域权限 | —（`noCheckPermView = true`） |
| 响应 | JSON 对象（无模板） |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `q` | string | 是 | 用户名或邮箱（**固定按 `system` 域查询**） |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `tfa` | boolean | 是否启用 TOTP 两步验证；用户不存在时为 `false` |
| `authn` | boolean | 是否已注册 WebAuthn 凭据；用户不存在时为 `false` |

**示例**

```http
GET /user/tfa?q=hydro
Accept: application/json
```

```json
{ "tfa": true, "authn": false }
```

#### WebAuthn · `user_webauthn`

```http
GET /user/webauthn
POST /user/webauthn
```

| 项 | 值 |
|---|---|
| 处理器 | `UserWebauthnHandler`（user.ts:156，`get` 166 行，`post` 188 行） |
| 认证 | 匿名 |
| 域权限 | —（`noCheckPermView = true`） |
| 响应 | GET：`{"authOptions": {...}}`；POST：重定向（登录）或 `back()` |

**GET 查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `uname` | `Types.Username` | 否 | 目标用户名；已登录时忽略（默认用当前用户） |
| `login` | `Types.Boolean` | 否 | 是否为「登录」场景；缺省/为假表示「验证已有凭据（如 sudo 二次确认）」 |

**GET 行为**：`login` 为假时，先确定 `udoc`（已登录用当前用户，否则按邮箱/用户名查），无此用户 → `UserNotFoundError`，未启用 WebAuthn → `AuthOperationError('authn', 'disabled')`（400）；随后 `generateAuthenticationOptions` 生成挑战，以 `token.add(TYPE_WEBAUTHN, 60, { uid: login ? 'login' : uid }, challenge)` 存入 60 秒有效的 token，并写 `session.challenge`。

**GET 响应字段**

| 字段 | 类型 | 说明 |
|---|---|---|
| `authOptions` | object | `@simplewebauthn/server` 的 `PublicKeyCredentialRequestOptions`（含 `challenge`、`rpId`、`allowCredentials`、`userVerification`） |

**POST 参数**（无装饰器，取合并后的 args：路径/查询/请求体皆可，实际由请求体传入）

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `result` | object | 是 | WebAuthn 断言结果（`AuthenticationResponseJSON`，含 `id`、`response` 等） |
| `redirect` | string | 否 | 登录成功后的回跳地址 |

**POST 行为**：`session.challenge` 缺失 → `ForbiddenError('no-challenge')`；token 不存在 → `InvalidTokenError('WebAuthn')`；`tdoc.uid === 'login'` 时按凭据 ID 反查用户（否则用 `tdoc.uid`）；校验断言（`verifyAuthenticationResponse`，`expectedOrigin` 取请求 `Origin` 头）失败 → `ValidationError('authenticator')`；成功后更新计数器。

- 登录场景：`successfulAuth` + 删除挑战 token + 重定向（`redirect` → 非 `/login` 的 `Referer` → `url('homepage')`）。
- 验证场景：把挑战 token 标记为 `verified: true`（60 秒），随后 `this.back()`。

#### 注册（发送验证邮件） · `user_register`

```http
GET /register
POST /register
```

| 项 | 值 |
|---|---|
| 处理器 | `UserRegisterHandler`（user.ts:243，`prepare` 245 行，`get` 249 行，`post` 254 行） |
| 认证 | 匿名 |
| 域权限 | —（`noCheckPermView = true`） |
| 路由权限 | `PRIV_REGISTER_USER`（`prepare()` 中 `server.login` 为假时抛 `BuiltinLoginError`） |
| 响应 | GET：HTML 模板 `user_register.html`；POST：模板 `user_register_mail_sent.html` 或重定向到 `/register/:code` |

**POST 请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `mail` | `Types.Email` | 是 | 注册邮箱；**仅从请求体取值**（`@post`） |

**POST 行为**：

1. 邮箱已存在 → `UserAlreadyExistError(mail)`（403）。
2. 邮箱域名在黑名单（`mail::<domain>`）→ `BlacklistedError(domain)`（403）。
3. 频率限制：`send_mail`（60 秒 1 次，按邮箱）与 `send_mail`（3600 秒 30 次）。
4. 创建 `TYPE_REGISTRATION` token，有效期 `session.unsaved_expire_seconds`（默认 3 小时），载荷 `{ mail, redirect: this.domain.registerRedirect, identity: { provider: 'mail', platform: 'mail', id: mail } }`。
5. 若 `smtp.verify` 与 `smtp.user` 均已配置：渲染 `user_register_mail.html` 并发送邮件，模板 `user_register_mail_sent.html`，响应体 `{ mail }`。
6. 否则直接重定向到 `url('user_register_with_code', { code })`（JSON 模式为 `{"url": "/register/<code>"}`）。

**响应字段（JSON，仅在发送邮件分支）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `mail` | string | 已发送验证邮件的邮箱地址 |

#### 注册（凭 token 完成） · `user_register_with_code`

```http
GET /register/:code
POST /register/:code
```

| 项 | 值 |
|---|---|
| 处理器 | `UserRegisterWithCodeHandler`（user.ts:291，`prepare` 296 行，`get` 305 行，`post` 318 行） |
| 认证 | 匿名 |
| 域权限 | —（`noCheckPermView = true`） |
| 路由权限 | `PRIV_REGISTER_USER` |
| 响应 | GET：HTML 模板 `user_register_with_code.html`；POST：重定向 |

**路径/参数**

| 参数 | 类型 | 必填 | 来源 | 说明 |
|---|---|---|---|---|
| `code` | string | 是 | 路径（`prepare` 用 `@param`，query/body 亦可） | 注册 token（32 位随机串） |
| `password` | `Types.Password` | 是（POST） | query/body | 新密码 |
| `verifyPassword` | `Types.Password` | 是（POST） | query/body | 重复密码 |
| `uname` | `Types.Username` | 否（POST） | query/body | 用户名；提供方 `lockUsername` 时强制使用 token 中的用户名 |

**`prepare`**：`token.get(code, TYPE_REGISTRATION)` 不存在或缺少 `identity` → 先执行 `limitRate('user_register_with_code', 60, 5)` 再抛 `InvalidTokenError('Registration', code)`。

**GET 响应字段**

| 字段 | 类型 | 说明 |
|---|---|---|
| `_id` | string | 注册 token |
| `mail` | string | 邮箱（OAuth 场景可能为空，则用随机邮箱占位） |
| `username` | string | 建议用户名 |
| `redirect` | string | 完成注册后的跳转地址 |
| `identity` | object | `{ provider, platform, id }` |
| `set` / `setInDomain` | object | OAuth 提供方要求写入用户/域内用户的字段 |
| `tokenType` / `createAt` / `updateAt` / `expireAt` | — | token 元数据 |
| `lockUsername` | boolean | 提供方是否锁定用户名（`provider.lockUsername`） |

**POST 行为**：提供方不存在 → `SystemError('OAuth provider ... not found')`（500）；`lockUsername` 时用 token 中的用户名；用户名不合法 → `ValidationError('uname')`；两次密码不一致 → `VerifyPasswordError`（403）；调用 `user.create(mail, uname, password, undefined, ip)` 创建账号；删除注册 token；若邮箱是 `qq.com` 且本地部分为纯数字，自动设置 `avatar = qq:<id>` 与 `qq` 字段；写入 `viewLang`、`setInDomain`；调用 `ctx.oauth.set(platform, id, uid)` 绑定第三方账号；`successfulAuth` 登录；重定向到 `tdoc.redirect || url('home_settings', { category: 'preference' })`。

#### 退出登录 · `user_logout`

```http
GET /logout
POST /logout
```

| 项 | 值 |
|---|---|
| 处理器 | `UserLogoutHandler`（user.ts:229，`get` 232 行，`post` 236 行） |
| 认证 | 需登录（`PRIV_USER_PROFILE`） |
| 域权限 | —（`noCheckPermView = true`） |
| 响应 | GET：HTML 模板 `user_logout.html`；POST：重定向到 `/` |

**GET**：仅渲染确认页（JSON 模式下响应体为 `{}`）。

**POST**：调用 `successfulAuth(uid = 0)`，即把当前 session 重置为匿名（`uid = 0`、`scope = PERM_ALL`、`sudo = null`、`recreate = true`），并重定向到 `/`（JSON 模式为 `{"url": "/"}`）。uid 为 0 时不会写 `loginat`/`loginip`，也不写操作日志、不触发 `auth/login` 钩子。

#### 找回密码（发送邮件） · `user_lostpass`

```http
GET /lostpass
POST /lostpass
```

| 项 | 值 |
|---|---|
| 处理器 | `UserLostPassHandler`（user.ts:345，`get` 348 行，`post` 353 行） |
| 认证 | 匿名 |
| 域权限 | —（`noCheckPermView = true`） |
| 响应 | GET：HTML 模板 `user_lostpass.html`；POST：HTML 模板 `user_lostpass_mail_sent.html`（无 JSON 业务字段） |

**POST 参数**

| 参数 | 类型 | 必填 | 来源 | 说明 |
|---|---|---|---|---|
| `mail` | `Types.Email` | 是 | query/body | 账号邮箱（固定按 `system` 域查询） |

**行为**：`smtp.user` 未配置 → `SystemError('Cannot send mail')`（500）；用户不存在 → `UserNotFoundError(mail)`（404）；频率限制 `send_mail`（3600 秒 30 次）与（60 秒 1 次，按邮箱）；创建 `TYPE_LOSTPASS` token（有效期 `session.unsaved_expire_seconds`），渲染 `user_lostpass_mail.html` 并发送邮件，模板 `user_lostpass_mail_sent.html`。

#### 找回密码（凭 token 重置） · `user_lostpass_with_code`

```http
GET /lostpass/:code
POST /lostpass/:code
```

| 项 | 值 |
|---|---|
| 处理器 | `UserLostPassWithCodeHandler`（user.ts:380，`get` 383 行，`post` 394 行） |
| 认证 | 匿名 |
| 域权限 | —（`noCheckPermView = true`） |
| 响应 | GET：HTML 模板 `user_lostpass_with_code.html`；POST：重定向到首页 |

**GET**：`code` 从合并 args 取（**无装饰器校验**）；token 不存在或类型不符 → `InvalidTokenError('Lost Password', code)`（403）。

**GET 响应字段**

| 字段 | 类型 | 说明 |
|---|---|---|
| `uname` | string | token 对应用户的用户名 |

**POST 参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `code` | string | 是 | 找回密码 token |
| `password` | `Types.Password` | 是 | 新密码 |
| `verifyPassword` | `Types.Password` | 是 | 重复密码 |

**POST 行为**：token 无效 → `InvalidTokenError`；两次密码不一致 → `VerifyPasswordError`（403）；随后清空 `authenticators` 与 `tfa`（`user.setById(uid, { authenticators: [], tfa: false })`）、`user.setPassword(uid, password)`、删除 token，重定向到 `url('homepage')`（JSON 模式为 `{"url": "/"}`）。

#### 注销账号 · `user_delete`

```http
POST /user/delete
```

| 项 | 值 |
|---|---|
| 处理器 | `UserDeleteHandler`（user.ts:455，`post` 456 行） |
| 认证 | 需登录（`PRIV_USER_PROFILE`） |
| 域权限 | `PERM_VIEW`（隐式） |
| 响应 | HTML 模板 `user_delete_pending.html`（JSON 模式下为 `{}`） |

**POST 参数**（无装饰器，取合并 args）

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `password` | string | 是 | 当前账号密码，用于二次确认 |

**行为**：`this.user.checkPassword(password)` 校验失败抛 `LoginError`（403）；随后创建一个 7 天后执行的定时任务（`ScheduleModel.add({ executeAfter: now + 7 天, type: 'script', id: 'deleteUser', args: { uid } })`），并把任务 id 写入 `udoc.del`；响应模板 `user_delete_pending.html`。

#### 用户资料 · `user_detail`

```http
GET /user/:uid
```

| 项 | 值 |
|---|---|
| 处理器 | `UserDetailHandler`（user.ts:405，`get` 407 行） |
| 认证 | 匿名 |
| 域权限 | `PERM_VIEW`（隐式） |
| 响应 | HTML 模板 `user_detail.html`，同时支持 JSON |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `uid` | `Types.Int`（有符号安全整数） | 用户 ID；`0` 直接抛 `UserNotFoundError`；`< -999` 表示虚拟用户（`vuser` 集合） |

> 装饰器为 `@param('uid', Types.Int)`，source 为 `all`，因此 `uid` 也可以从查询串或请求体传入；路由中的 `:uid` 属于路径参数。

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `isSelfProfile` | boolean | 是否查看自己的主页（`this.user._id === uid`） |
| `udoc` | object | 目标用户（`user.getById`，公开字段序列化，见「UserContext 序列化字段」） |
| `sdoc` | object \| null | 最近一次会话的 `{ createAt, updateAt }`（`token.getMostRecentSessionByUid`，`_id` 不返回） |
| `pdocs` | array | 该用户在本域 AC 过的题目列表（需 `PERM_VIEW_PROBLEM`；否则为空数组） |
| `tags` | array | 由 AC 题目标签统计出的前 20 个标签，元素为 `[标签, 次数]`，按次数降序 |
| `tdocs` | array | 该用户参加过（`attend` 存在）的比赛，元素为 `{ docId, title, rule }`，按 `_id` 降序 |
| `psdocs` | array | 该用户最近 10 条题解（仅当有 `PERM_VIEW_PROBLEM_SOLUTION` 时出现） |
| `pdict` | object | `psdocs` 对应题目的字典（需 `PERM_VIEW_PROBLEM`） |

**权限细节**：

- `canViewHidden = this.user.hasPerm(PERM_VIEW_PROBLEM_HIDDEN) || this.user._id`（登录即可看到自己的隐藏题目记录）。
- `pdocs` 仅在调用者拥有 `PERM_VIEW_PROBLEM` 时计算；`psdocs`/`pdict` 还要求 `PERM_VIEW_PROBLEM_SOLUTION`。
- 响应会设置 `UiContext.extraTitleContent = udoc.uname`，使页面标题带上用户名。

**错误**：`uid === 0` 或用户不存在 → `UserNotFoundError`（404）。

**示例**

```http
GET /d/system/user/2
Accept: application/json
```

```json
{
  "isSelfProfile": false,
  "udoc": {
    "_id": 2,
    "uname": "hydro",
    "mail": "hydro@hydro.local",
    "perm": "BigInt::9223372036854775807",
    "role": "root",
    "priv": 1023,
    "regat": "2024-01-01T00:00:00.000Z",
    "loginat": "2024-06-01T00:00:00.000Z",
    "avatar": "gravatar:hydro@hydro.local"
  },
  "sdoc": { "createAt": "2024-06-01T00:00:00.000Z", "updateAt": "2024-06-01T01:00:00.000Z" },
  "pdocs": [],
  "tags": [],
  "tdocs": []
}
```

#### 比赛模式 · `contest_mode`

```http
GET /contestmode
POST /contestmode   # 仅子操作（operation=reset），无普通 post
```

| 项 | 值 |
|---|---|
| 处理器 | `ContestModeHandler`（user.ts:553，`get` 554 行，`postReset` 562 行） |
| 认证 | 需要 `PRIV_EDIT_SYSTEM`（超级管理员） |
| 域权限 | `PERM_VIEW`（隐式） |
| 注册条件 | 仅当 `system.get('server.contestmode')` 为真时才注册该路由 |
| 响应 | GET：HTML 模板 `contest_mode.html`；POST：无业务字段 |

**GET 响应字段**

| 字段 | 类型 | 说明 |
|---|---|---|
| `bindings` | array | 所有 `loginip` 存在的用户，元素为 `{ _id, loginip }` |

**POST 子操作（`operation`）**

| operation | 对应方法 | 说明 | 额外参数 |
|---|---|---|---|
| `reset` | `postReset`（562） | 清除 IP 绑定（比赛结束后恢复） | `uid`（`Types.Int`，可选）：指定则只清该用户的 `loginip`；不传则清空所有用户的 `loginip` 并刷新用户缓存 |

> `ContestModeHandler` 只有 `get@554` 与 `postReset@562`，**没有普通的 `post` 方法**；`operation` 必须为 `reset`。

**示例**

```http
POST /contestmode
Accept: application/json
Content-Type: application/json

{ "operation": "reset", "uid": 2 }
```

```json
{}
```

#### 用户数据 API（`UserApi`）

注册方式（`user.ts:662-664`）：

```ts
await ctx.inject(['api'], ({ api }) => { api.provide(UserApi); });
```

由 `applyApiHandler(childContext, 'api', '/api/:op')` 暴露：

- HTTP：`Route('api', '/api/:op', ApiHandler)` → 形如 `GET/POST /api/user.user`、`POST /api/user.users`；
- WebSocket/SSE：`Connection('api_conn', '/api/:op/conn', ApiConnectionHandler)`（仅 `Subscription` 类型可用，本 API 均为 `Query`）。

**请求约定**（`framework/framework/api.ts`）：

- 入参从 `{ domainId, ...args, ...(args.args || {}) }` 组装；`args` 可以是对象，也可以是 JSON 字符串（自动 `JSON.parse`）；`projection` 可以是逗号分隔的字段名列表，或 JSON 投影对象。
- 操作名不合法 → `BadRequestError('Invalid API operation: ...')`（400）；`Mutation` 不允许 GET；`Subscription` 不允许在 HTTP 调用。
- 出参同样经过序列化器（`_` 前缀字段被丢弃、`bigint` → `"BigInt::<n>"`、`User` 走 `serialize()`）。

##### 用户查询 · `user.user`

```http
GET /api/user.user?id=2
POST /api/user.user
```

| 项 | 值 |
|---|---|
| 定义 | `UserApi.user`（user.ts:574） |
| 类型 | `Query` |
| 认证 | 匿名（`ApiHandler` 无 `noCheckPermView`，因此隐式要求 `PERM_VIEW`） |
| 响应 | 单个用户对象或 `null` |

**参数**（`Schema.object`）

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `id` | number（整数） | 否 | 用户 uid |
| `uname` | string | 否 | 用户名 |
| `mail` | string | 否 | 邮箱 |
| `domainId` | string | 是 | 域 ID；框架会自动注入当前请求的域，通常无需显式传 |

**取值优先级**：`id` → `mail` → `uname` → 当前登录用户（`c.user._id`）。都取不到时返回当前用户（匿名时即 uid 0 的默认用户）。

**响应**：`user.getById(domainId, id)` 等方法的返回值，即 `User` 对象序列化后的公开字段；未找到时返回 `null`。

```http
GET /api/user.user?uname=hydro
Accept: application/json
```

```json
{
  "_id": 2,
  "uname": "hydro",
  "mail": "hydro@hydro.local",
  "perm": "BigInt::9223372036854775807",
  "role": "root",
  "priv": 1023,
  "regat": "2024-01-01T00:00:00.000Z",
  "loginat": "2024-06-01T00:00:00.000Z",
  "avatar": "gravatar:hydro@hydro.local"
}
```

##### 批量/搜索用户 · `user.users`

```http
GET /api/user.users?search=hyd
POST /api/user.users
```

| 项 | 值 |
|---|---|
| 定义 | `UserApi.users`（user.ts:583） |
| 类型 | `Query` |
| 认证 | 匿名（隐式 `PERM_VIEW`） |
| 响应 | 用户对象数组 |

**参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `ids` | number[] | 否 | 按 uid 批量查询 |
| `auto` | string[] | 否 | 批量查询：纯数字按 uid，其余按用户名/邮箱 |
| `search` | string | 否 | 搜索关键字（uid、用户名或邮箱） |
| `limit` | number | 否 | 前缀搜索结果上限，默认 10，且被限制为最大 10 |
| `exact` | boolean | 否 | 为真时只返回精确匹配，不做前缀搜索 |

**行为**：

- 传了 `ids` 或 `auto` 时走批量分支：先按数字 ID 用 `user.getList` 查，并为结果补充 `avatarUrl`（`avatar(udoc.avatar)`）；剩余非数字项再按用户名/邮箱查（若未命中的项超过 50 个则直接返回已找到的结果）；不做 `limit` 限制。
- 否则走搜索分支：`search` 为空返回 `[]`；先用 `user.getById(+search) || user.getByUname || user.getByEmail` 精确匹配，`exact` 为假时再追加 `user.getPrefixList(domainId, search, Math.min(limit || 10, 10))` 的前缀匹配结果（精确命中会被放到首位），同样为结果补充 `avatarUrl`。

```http
GET /api/user.users?search=hyd&limit=5
Accept: application/json
```

```json
[
  {
    "_id": 2,
    "uname": "hydro",
    "mail": "hydro@hydro.local",
    "priv": 1023,
    "role": "root",
    "avatar": "gravatar:hydro@hydro.local",
    "avatarUrl": "//cn.gravatar.com/avatar/<md5>?d=mm&s=128"
  }
]
```

---

### 首页与个人中心接口（`packages/hydrooj/src/handler/home.ts`）

路由注册（`home.ts:622-630`）：

```ts
ctx.Route('homepage', '/', HomeHandler);
ctx.Route('home_security', '/home/security', HomeSecurityHandler, PRIV.PRIV_USER_PROFILE);
ctx.Route('user_changemail_with_code', '/home/changeMail/:code', UserChangemailWithCodeHandler, PRIV.PRIV_USER_PROFILE);
ctx.Route('home_settings', '/home/settings/:category', HomeSettingsHandler, PRIV.PRIV_USER_PROFILE);
ctx.Route('home_avatar', '/home/avatar', HomeAvatarHandler, PRIV.PRIV_USER_PROFILE);
ctx.Route('home_domain', '/home/domain', HomeDomainHandler, PRIV.PRIV_USER_PROFILE);
ctx.Route('home_domain_create', '/home/domain/create', HomeDomainCreateHandler, PRIV.PRIV_CREATE_DOMAIN);
ctx.Route('home_messages', '/home/messages', HomeMessagesHandler, PRIV.PRIV_USER_PROFILE);
```

#### 首页 · `homepage`

```http
GET /
```

| 项 | 值 |
|---|---|
| 处理器 | `HomeHandler`（home.ts:36，`get` 147 行） |
| 认证 | 匿名 |
| 域权限 | `PERM_VIEW`（隐式） |
| 响应 | HTML 模板 `main.html`，同时支持 JSON |
| 说明 | **没有 `post` 方法**，`POST /` 会返回 405 `MethodNotAllowedError` |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `contents` | array | 首页栏目，元素为 `{ width, sections }`；`sections` 是 `[区块名, 数据]` 数组，区块名与数据取决于 `hydrooj.homepage` 配置（YAML）与下方 `get*` 方法 |
| `udict` | object | 首页出现过的用户字典 `{ [uid]: udoc }`（由各区块收集的 `uids`） |
| `domain` | object | 当前域文档 `DomainDoc` |

**内置区块方法**（`hydrooj.homepage` 中的键名会映射到 `get<CamelCase>` 方法，参数为该键对应的配置值）：

| 方法 | 需要的权限 | 返回 |
|---|---|---|
| `getHomework(domainId, limit = 5)` | `PERM_VIEW_HOMEWORK` | `[tdocs, tsdict]`；无权限时 `[[], {}]` |
| `getContest(domainId, limit = 10)` | `PERM_VIEW_CONTEST` | `[tdocs, tsdict]`；无权限时 `[[], {}]` |
| `getTraining(domainId, limit = 10)` | `PERM_VIEW_TRAINING` | `[tdocs, tsdict]`；无权限时 `[[], {}]` |
| `getDiscussion(domainId, limit = 20)` | `PERM_VIEW_DISCUSSION` | `[ddocs, vndict]`；无权限时 `[[], {}]` |
| `getRanking(domainId, limit = 50)` | `PERM_VIEW_RANKING` | uid 数组；无权限时 `[]` |
| `getStarredProblems(domainId, limit = 50)` | `PERM_VIEW_PROBLEM` | `[pdocs]`；无权限时 `[[], {}]` |
| `getRecentProblems(domainId, limit = 10)` | `PERM_VIEW_PROBLEM` | `[pdocs, psdict]`；无权限时 `[[], {}]` |
| `getDiscussionNodes(domainId)` | — | 讨论节点列表 |

未识别的键会作为静态数据直接放入 `sections`；区块方法抛错时会替换为 `['error', err.message]`。

#### 账号安全 · `home_security`

```http
GET /home/security
POST /home/security   # 仅子操作（operation），无普通 post
```

| 项 | 值 |
|---|---|
| 处理器 | `HomeSecurityHandler`（home.ts:181，`get` 183 行，11 个 `postXxx` 子操作） |
| 认证 | 需登录（`PRIV_USER_PROFILE`） |
| 域权限 | `PERM_VIEW`（隐式） |
| 响应 | GET：HTML 模板 `home_security.html`；POST：`back()` 重定向，个别子操作返回 `authOptions` |
| 特殊 | GET 与多数 POST 子操作带 `@requireSudo`（1 小时内提权过） |

**GET 响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `sudoUid` | number \| null | 当前 sudo 代理的 uid（`session.sudoUid`），无则为 `null` |
| `sessions` | array | 当前用户的会话列表（`token.getSessionListByUid`，按 `updateAt` 降序，最多 100 条） |
| `authenticators` | array | 已注册的 WebAuthn 凭据摘要，字段取自 `{ credentialID, name, credentialType, credentialDeviceType, authenticatorAttachment, regat, fmt }` |
| `geoipProvider` | string \| undefined | GeoIP 提供方名称（`ctx.get('geoip')?.provider`） |
| `relations` | array | 已绑定的第三方账号，元素为 `{ platform, id, uid }` |
| `loginMethods` | array | 可用第三方登录方式 `{ id, icon, text, name }` |

`sessions` 元素字段：token 文档原始字段（`uid`、`tokenType`、`createAt`、`updateAt`、`expireAt`、`createIp`、`createUa`、`createHost`、`updateIp`、`updateUa` 等）外加：

| 字段 | 说明 |
|---|---|
| `_id` | **被替换为 `md5(真实 session id)`**，客户端只能用该摘要做操作 |
| `isCurrent` | 是否为当前会话 |
| `updateUaInfo` | `UAParser(updateUa \|\| createUa)` 的解析结果 |
| `updateGeoip` | `geoip.lookup(updateIp \|\| createIp, locale)` 的结果（需 geoip 服务） |

**POST 子操作（`operation`）**

| operation | 对应方法 | 说明 | 额外参数 | 需 sudo |
|---|---|---|---|---|
| `change_password` | `postChangePassword`（215） | 修改密码；成功后删除该用户全部会话并跳转登录页 | `current`（string）、`password`（`Types.Password`）、`verifyPassword`（`Types.Password`） | 是 |
| `change_mail` | `postChangeMail`（230） | 修改邮箱：校验新邮箱未被占用、域名不在黑名单，发送确认邮件（模板 `user_changemail_mail_sent.html`） | `password`（`Types.Password`，当前密码）、`mail`（`Types.Email`，新邮箱） | 是 |
| `link_account` | `postLinkAccount`（257） | 绑定第三方账号：设置 `session.oauthBind = platform` 后跳转到该提供方的授权页 | `platform`（string，必须是已注册的提供方） | 否 |
| `unlink_account` | `postUnlinkAccount`（264） | 解绑第三方账号（`ctx.oauth.unbind`） | `platform`（string） | 否 |
| `delete_token` | `postDeleteToken`（271） | 按摘要删除某个会话 | `tokenDigest`（string，`md5(sessionId)`） | 否 |
| `delete_all_tokens` | `postDeleteAllTokens`（283） | 删除当前用户的全部会话并跳转登录页 | — | 否 |
| `enable_tfa` | `postEnableTfa`（291） | 启用 TOTP：校验动态码后写入 `tfa` 密钥 | `code`（string，6 位动态码）、`secret`（string，Base32 密钥） | 是 |
| `register` | `postRegister`（305） | 生成 WebAuthn 注册选项（**不重定向**，直接返回 `authOptions`） | `type`（`Types.Range(['cross-platform', 'platform'])`） | 是 |
| `enable_authn` | `postEnableAuthn`（329） | 完成 WebAuthn 注册：使用 `session.webauthnVerify` 与请求体中的 `result` 校验并保存凭据 | `name`（string，凭据名称）、`result`（WebAuthn 注册响应，从 args 读取） | 是 |
| `disable_authn` | `postDisableAuthn`（357） | 删除指定 WebAuthn 凭据 | `id`（string，`credentialID` 的 base64 字符串） | 是 |
| `disable_tfa` | `postDisableTfa`（365） | 关闭 TOTP（`$unset tfa`） | — | 是 |

> 该处理器**没有普通的 `post` 方法**，因此不带 `operation`（或 `operation` 无对应方法）的 POST 请求会抛 `InvalidOperationError`（405）。

**注意**：

- `change_password` / `change_mail` 会优先使用 `session.sudoUid`（sudo 代理用户）校验当前密码，否则用当前登录用户。
- `register` 子操作的响应为 `{ "authOptions": { ... } }`（因为该子操作设置了 `response.body.authOptions` 且未设置 `redirect`）。
- 其余子操作调用 `this.back()`，JSON 模式下返回 `{"url": "<Referer>"}`。
- `operation` 与方法的对应关系见「认证、会话与通用响应约定」中的「POST 子操作（`operation`）派发」。

**示例**

```http
POST /home/security
Accept: application/json
Content-Type: application/json

{
  "operation": "change_password",
  "current": "oldpass",
  "password": "newpass123",
  "verifyPassword": "newpass123"
}
```

```json
{ "url": "/login" }
```

（`change_password` 成功后删除该用户全部会话并跳转登录页。若当前会话未通过 sudo 校验，`requireSudo` 会先返回 `{"url": "/user/sudo"}`。）

#### 修改邮箱确认 · `user_changemail_with_code`

```http
GET /home/changeMail/:code
```

| 项 | 值 |
|---|---|
| 处理器 | `UserChangemailWithCodeHandler`（home.ts:475，`get` 477 行） |
| 认证 | 需登录（`PRIV_USER_PROFILE`） |
| 域权限 | `PERM_VIEW`（隐式） |
| 响应 | 重定向到 `home_security` |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `code` | string | `TYPE_CHANGEMAIL` token |

**行为**：token 不存在或 `tdoc.uid !== this.user._id` → `InvalidTokenError('Change Email', code)`（403）；新邮箱已被占用 → `UserAlreadyExistError`（403）；否则执行 `user.setEmail(uid, tdoc.email)` 并删除 token，重定向到 `url('home_security')`（JSON 模式为 `{"url": ...}`）。

#### 个人设置 · `home_settings`

```http
GET /home/settings/:category
POST /home/settings/:category
```

| 项 | 值 |
|---|---|
| 处理器 | `HomeSettingsHandler`（home.ts:417，`get` 419 行，`post` 435 行） |
| 认证 | 需登录（`PRIV_USER_PROFILE`） |
| 域权限 | `PERM_VIEW`（隐式） |
| 响应 | GET：HTML 模板 `home_settings.html`；POST：`back()` 重定向 |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `category` | `Types.Range(['preference', 'account', 'domain'])` | 设置分类；其它值抛 `NotFoundError`（404） |

**GET 响应字段**

| 字段 | 类型 | 说明 |
|---|---|---|
| `category` | string | 当前分类 |
| `page_name` | string | UI 字段，固定为 `home_<category>` |
| `current` | object | 当前用户（`User#private()` 序列化，包含私有偏好字段） |
| `settings` | array | 该分类下的设置项定义：`preference` → `PREFERENCE_SETTINGS`；`account` → `ACCOUNT_SETTINGS`；`domain` → `DOMAIN_USER_SETTINGS` |

**POST 参数**（无装饰器，取合并 args；**请求体覆盖查询参数**）

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `category` | string | 是 | 决定写入位置：`domain` → `domain.setUserInDomain(domainId, uid, $set)`；其它 → `user.setById(uid, $set)` |
| `booleanKeys` | object | 否 | 布尔开关清单：其中值为假的键会被显式写成 `false`（用于处理未勾选的复选框） |
| 其余键 | 任意 | 否 | 逐项按设置定义校验/转换（`set()`），校验失败抛 `ValidationError(key)`（403）；`setting_storage` 家族与 `FLAG_DISABLED` 项被忽略 |

**行为**：`set()` 会按设置类型转换（`boolean`：`'on'` → `true`，其它 → `false`；`number`/`float` 校验安全整数/有限数；`json`/`yaml`/`markdown`/`textarea`/`text` 分别做长度与语法校验）。写入完成后若 `viewLang` 发生变化会清空 `session.viewLang`，随后 `this.back()`。

#### 头像设置 · `home_avatar`

```http
POST /home/avatar
```

| 项 | 值 |
|---|---|
| 处理器 | `HomeAvatarHandler`（home.ts:454，`post` 458 行） |
| 认证 | 需登录（`PRIV_USER_PROFILE`） |
| 域权限 | —（`noCheckPermView = true`） |
| 响应 | `back()` 重定向 |

**参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `avatar` | string | 否 | 头像标识，`lib/avatar.ts#validate` 校验：允许 `url:<链接>`、`github:<用户名>`（`^[a-zA-Z0-9-]+$`）、`qq:<数字>`（`^[1-9]\d{4,}$`）、`gravatar:<邮箱>` |
| `file`（multipart 文件字段） | file | 否 | 当未传 `avatar` 时使用；**上限 8 MiB**，扩展名仅允许 `.jpg` / `.jpeg` / `.png` |

**行为**：

- 传了 `avatar`：校验通过后 `user.setById(uid, { avatar })`。
- 否则读取 `this.request.files.file`（multipart）：大小/扩展名不合法 → `ValidationError('file')`（403）；合法则 `storage.put('user/<uid>/.avatar<ext>', filepath, uid)` 并写入 `avatar = url:/file/<uid>/.avatar<ext>`。
- 两者都没有 → `ValidationError('avatar')`（403）。
- 最后 `this.back()`。

#### 我的域 · `home_domain`

```http
GET /home/domain
POST /home/domain   # 仅子操作（operation=star|leave），无普通 post
```

| 项 | 值 |
|---|---|
| 处理器 | `HomeDomainHandler`（home.ts:492，`get` 494 行，`postStar` 525 行，`postLeave` 535 行） |
| 认证 | 需登录（`PRIV_USER_PROFILE`） |
| 域权限 | `PERM_VIEW`（隐式） |
| 响应 | GET：HTML 模板 `home_domain.html`；POST：`back()` 重定向 |

**GET 查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `all` | `Types.Boolean`（仅从 query 取） | 否 | 为真时列出全部域，需要 `PRIV_VIEW_ALL_DOMAIN`（否则 `PrivilegeError`，403） |

**GET 响应字段**

| 字段 | 类型 | 说明 |
|---|---|---|
| `ddocs` | array | 域文档数组：`all` 为真时是全部域，否则是当前用户加入过的域（`domain.getDictUserByDomainId`） |
| `canManage` | object | `{ [domainId]: boolean }`；拥有 `PRIV_MANAGE_ALL_DOMAIN` 时全部为 `true`，否则取决于该域内权限 `PERM_EDIT_DOMAIN` |
| `role` | object | `{ [domainId]: 角色名 }`；拥有 `PRIV_MANAGE_ALL_DOMAIN` 时固定为 `'root'`，否则为该域内 `udoc.role` |

**POST 子操作（`operation`）**

| operation | 对应方法 | 说明 | 额外参数 |
|---|---|---|---|
| `star` | `postStar`（525） | 置顶/取消置顶域（写入用户 `pinnedDomains`）；置顶时域不存在 → `NotFoundError`（404） | `id`（`Types.DomainId`）、`star`（`Types.Boolean`，默认 false） |
| `leave` | `postLeave`（535） | 退出域（`domain.setJoin(id, uid, false)`）；`id === 'system'` → `BadRequestError`（400）；域不存在 → `NotFoundError`（404） | `id`（`Types.DomainId`） |

> 该处理器**没有普通的 `post` 方法**（仅有 `get@494`、`postStar@525`、`postLeave@535`），不带 `operation` 的 POST 会抛 `InvalidOperationError`（405）。

`star` 子操作的响应为 `{"star": <bool>, "url": "<Referer>"}`（`back({ star })`）；`leave` 为 `{"url": "<Referer>"}`。

#### 创建域 · `home_domain_create`

```http
GET /home/domain/create
POST /home/domain/create
```

| 项 | 值 |
|---|---|
| 处理器 | `HomeDomainCreateHandler`（home.ts:544，`get` 545 行，`post` 554 行） |
| 认证 | 需要 `PRIV_CREATE_DOMAIN` |
| 域权限 | `PERM_VIEW`（隐式） |
| 响应 | GET：HTML 模板 `domain_create.html`；POST：重定向到域控制台 |

**POST 参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `id` | `Types.DomainId`（`^[a-zA-Z]\w{3,31}$`） | 是 | 新域 ID |
| `name` | `Types.Title`（1–64 字符） | 是 | 域名称 |
| `bulletin` | `Types.Content` | 是 | 域公告 |
| `avatar` | `Types.Content` | 否 | 域头像；缺省时用创建者头像，再退化为 `gravatar:<创建者邮箱>` |

**行为**：域 ID 已存在 → `DomainAlreadyExistsError`（403）；`domain.add(id, uid, name, bulletin)` 创建域；并行执行 `domain.edit(domainId, { avatar })`、`domain.setUserRole(domainId, uid, 'root')`，若创建者尚未置顶该域则 `$push pinnedDomains`。

**响应**：`response.body = { domainId }` 且 `response.redirect = url('domain_dashboard', { domainId })`；JSON 模式为：

```json
{ "domainId": "mydomain", "url": "/d/mydomain/dashboard" }
```

#### 站内消息 · `home_messages`

```http
GET /home/messages
POST /home/messages   # 仅子操作（operation=send|delete_message），无普通 post
```

| 项 | 值 |
|---|---|
| 处理器 | `HomeMessagesHandler`（home.ts:573，`get` 574 行，`postSend` 603 行，`postDeleteMessage` 613 行） |
| 认证 | 需登录（`PRIV_USER_PROFILE`） |
| 域权限 | `PERM_VIEW`（隐式） |
| 响应 | GET：HTML 模板 `home_messages.html`；POST：`back()` 重定向 |

**GET 响应字段**

| 字段 | 类型 | 说明 |
|---|---|---|
| `messages` | object | `{ [对方uid]: { _id, udoc, messages } }`；`udoc` 含 `avatarUrl`，`messages` 为该会话的消息数组（按 `_id` 降序，最多 1000 条原始记录） |

**副作用**：读取时会把当前用户 `unreadMsg` 清零。

**消息文档字段**（`model/message.ts`）：`_id`、`from`、`to`（数组）、`content`、`flag`。`flag` 常量：`FLAG_UNREAD = 1`、`FLAG_ALERT = 2`、`FLAG_RICHTEXT = 4`、`FLAG_INFO = 8`、`FLAG_I18N = 16`。

**POST 子操作（`operation`）**

| operation | 对应方法 | 说明 | 额外参数 |
|---|---|---|---|
| `send` | `postSend`（603） | 发送私信；需要 `PRIV_SEND_MESSAGE`（否则 `PrivilegeError`，403）；收件人不存在 → `UserNotFoundError`（404） | `uid`（`Types.Int`，收件人）、`content`（`Types.Content`，内容） |
| `delete_message` | `postDeleteMessage`（613） | 删除消息；仅发送者可删，否则 `PermissionError`（403） | `messageId`（`Types.ObjectId`） |

> 该处理器**没有普通的 `post` 方法**（仅有 `get@574`、`postSend@603`、`postDeleteMessage@613`），不带 `operation` 的 POST 会抛 `InvalidOperationError`（405）。

`send` 子操作的响应为 `back({ mdoc, udoc })`，JSON 模式：

```json
{
  "mdoc": { "_id": "<ObjectId>", "from": 2, "to": [3], "content": "hi", "flag": 1 },
  "udoc": { "_id": 3, "uname": "someone", "avatarUrl": "//..." },
  "url": "/home/messages"
}
```

---

### 实时事件接口（`packages/hydrooj/src/handler/connection.ts`）

#### 事件总线 WebSocket 网关 · `websocket_gateway`

```http
WS /websocket
```

| 项 | 值 |
|---|---|
| 处理器 | `WebsocketEventsConnectionManagerHandler`（connection.ts:11，`prepare` 19 行） |
| 注册 | `ctx.Connection('websocket_gateway', '/websocket', ...)`（connection.ts:111） |
| 认证 | 匿名（频道级鉴权）或网关特权模式（`x-hydro-websocket-gateway` 头） |
| 域权限 | —（`noCheckPermView = true`） |
| 响应 | WebSocket 帧（JSON 文本，可选择 deflate 压缩） |

**连接方式**

- 地址：`UiContext.ws_prefix + 'websocket'`，默认即 `/websocket`（**不带 `/d/:domainId` 前缀**，客户端直接用 `new WebSocket(`${UiContext.ws_prefix}websocket`)`）。
- 无法携带 Cookie 的跨域场景可用查询参数传会话：`/websocket?sid=<sessionId>`；若提供 `?shorty=on`，服务端会先发送字符串 `shorty` 并随后使用 deflate 压缩负载（客户端用 `shorty.js` 的 `Shorty` 解压）。
- 心跳：服务端每 40 秒检查一次，空闲超过 30 秒发送文本 `ping`，空闲超过 80 秒直接断开；客户端可回 `pong`（也可主动发 `ping`，服务端回 `pong`）。

**两种鉴权/权限模式**

| 模式 | 触发条件 | 能力 |
|---|---|---|
| 普通（未授权） | 未提供 `x-hydro-websocket-gateway` 头 | 只能订阅经 `subscription/subscribe` 校验通过的频道 |
| 网关特权（privileged） | 请求头 `x-hydro-websocket-gateway` 的值等于 `system.get('websocket.secret')`（未配置该设置项时永远不通过） | 可使用 `resume` 操作、可接收全量 `user/message` 推送 |

头部校验失败 → `ForbiddenError('Invalid token')`（403），随后按 WebSocket 错误流程发送错误帧并关闭。

**客户端 → 服务端消息格式**

```json
{ "operation": "subscribe", "request_id": "abc123", "credential": "<sessionId>", "channels": ["message"], "metadata": {} }
```

| 字段 | 类型 | 说明 |
|---|---|---|
| `operation` | string | `subscribe` / `resume` / `unsubscribe`；其它值会被忽略 |
| `channels` | string[] | 要订阅/退订的频道名 |
| `credential` | string | 可选，session id（`token.TYPE_SESSION`）；用于确定订阅者身份 |
| `request_id` | string | 可选，原样回显在 `verify` 响应中 |
| `subscription_id` | string | 可选，原样回显在 `verify` 响应中 |
| `metadata` | object | 可选，仅在网关特权模式下会传给 `subscription/subscribe` |

**服务端 → 客户端消息格式**

| 消息 | 结构 | 说明 |
|---|---|---|
| 订阅结果 | `{ "operation": "verify", "accept": [...], "reject": [...], "request_id": ..., "subscription_id": ... }` | `accept` 为成功订阅的实际频道名（可能与请求名不同，例如 `message` → `message:<uid>`），`reject` 为失败项 |
| 事件推送 | `{ "operation": "event", "channels": ["message:3"], "payload": { ... } }` | 由各插件通过 `h.send()` 推送 |
| 网关恢复失败 | `{ "operation": "resume_failed" }` | 非特权连接发起 `resume` 时返回 |
| 错误 | `{ "error": { "name": "PrivilegeError", "params": [4] } }` | 发送后连接以 `close(4000, err.toString())` 关闭（HTTP 升级阶段或 `prepare` 阶段抛错时） |

**内置可订阅频道**

核心包仅实现了一个频道（`handler/home.ts:641-662`）：

| 频道名 | 订阅条件 | 实际频道 | 推送内容 |
|---|---|---|---|
| `message` | 已登录（`udoc.hasPriv(PRIV.PRIV_USER_PROFILE)`） | `message:<uid>` | 收到新私信时推送 `{ udoc: {...序列化用户, avatarUrl}, mdoc }`（事件源为 `user/message`，参数为收件人 uid 数组与消息文档） |

- 网关特权连接在 `subscription/init` 阶段会直接监听全量 `user/message`，把每条消息推给所有相关 uid（频道名仍为 `message:<uid>`）。
- 普通连接在 `subscription/enable` 阶段只监听自己订阅的 `message:<uid>`。

**插件扩展点**（事件总线，`packages/hydrooj/src/service/bus.ts`）：

| 事件 | 签名 | 用途 |
|---|---|---|
| `subscription/init` | `(h: ConnectionHandler, privileged: boolean)` | 连接建立后调用，插件可在此注册全局监听 |
| `subscription/subscribe` | `(channel: string, user: User, metadata: Record<string, string>)` | 校验频道订阅请求；返回 `{ ok: true, channel: '实际频道名' }` 表示通过（框架用 `ctx.bail` 调用，取第一个有返回值的处理器） |
| `subscription/enable` | `(channel, h, privileged, onDispose)` | 频道被接受后调用，插件在此注册真正的推送监听，并通过 `onDispose` 注册清理函数 |

**客户端示例**（`packages/ui-default/pages/home_messages.page.tsx`）：

```js
const sock = new WebSocket(`${UiContext.ws_prefix}websocket`);
sock.onopen = () => {
  sock.send(JSON.stringify({
    operation: 'subscribe',
    request_id: Math.random().toString(16).substring(2),
    credential: document.cookie.split('sid=')[1].split(';')[0],
    channels: ['message'],
  }));
};
sock.onmessage = (message) => {
  const msg = JSON.parse(message.data);
  if (msg.operation !== 'event') return;
  // msg.payload.udoc / msg.payload.mdoc
};
```

---

---

## 3. 题目、提交与评测

### 题目接口（problem）与通用补充约定

> 本部分覆盖 `packages/hydrooj/src/handler/problem.ts`、`record.ts`、`judge.ts`、`status.ts`、`import.ts`、`compat.ts` 的**全部路由**，以及评测状态码、评测链路、`config.yaml` 字段、语言与代码模板。
> 路由注册代码分别在 `problem.ts:1069-1087`、`record.ts:491-496`、`judge.ts:356-359`、`status.ts:84-88`、`import.ts:40-43`、`compat.ts:9-11`。
> 所有路由均挂载在域前缀下：系统域为 `/p/...`，其它域为 `/d/:domainId/p/...`（见第 1 章）。

**本部分通用规则（源码依据）**

- 参数装饰器来源：`@param` = query + body（`source:'all'`，`decorators.ts:66-73`）；`@query`/`@get` = 仅 query；`@post` = 仅 body；`@route` = URL 路径参数（内部会把 `domainId` 一起注入）。
- `Types.Boolean` 的第三项为 `true`（`validator.ts:120`），即**凡 `Types.Boolean` 的参数隐含可选**。
- 若响应中设置了 `response.redirect`，JSON 模式下 body 会自动附加 `url` 字段（`framework/framework/base.ts:64-67`）。
- HTTP 错误在 `Accept: application/json` 下返回 `{ "error": { "params": [], "name": "<错误类名>", "code": <HTTP状态码> } }`（详见下节「错误 JSON 体」）。
- WebSocket 错误推送 `{ "error": { "name": "...", "params": [...] } }` 后以 close code `4000` 关闭（`packages/hydrooj/src/service/server.ts:240-253`）。
- 除声明 `noCheckPermView` 的处理器外，所有 HTTP 路由隐式要求 `PERM_VIEW`（`service/server.ts:275`）。

#### 通用补充约定（序列化 / 分页 / 错误体 / 响应分流）

**序列化规则**（`framework/framework/serializer.ts`，由 `JSON.stringify(body, serializer(false, handler))` 应用于所有 JSON 响应）

| 规则 | 说明 |
|---|---|
| 下划线键 | 键名以 `_` 开头的字段在 JSON 响应中被**丢弃**（`_id` 例外，会被保留） |
| `bigint` | 序列化为字符串 `"BigInt::<值>"`（`PERM.*` 权限位是 bigint，故若接口返回权限值会是该形式） |
| 自定义序列化 | 值若为对象且带 `serialize()` 方法，则调用 `value.serialize(handler)` 并用返回值替换自身 |

**分页**（`this.paginate`，底层 `packages/hydrooj/src/service/db.ts:153-166`）

| 项 | 值 |
|---|---|
| 签名 | `paginate(cursor, page, key: string \| number) => Promise<[docs: T[], numPages: number, count: number]>` |
| 页码校验 | `page <= 0` 抛 `ValidationError('page')` |
| 每页大小 | `key` 为字符串时取 `setting.pagination.<key>`（缺省 **20**）；为数字时直接使用该数字 |
| 响应约定 | 各接口返回 `page` 与 `<prefix>pcount`（如 `ppcount` / `pscount` / `pcount`），**不存在** HOJ 风格的 `records` / `total` / `size` / `current` / `pages` 结构 |

**错误 JSON 体**（`Accept: application/json`）

```json
{ "error": { "params": [], "name": "LoginError", "code": 403 } }
```

实测响应体**没有** `message` 与 `stack`，只有 `params`（错误构造参数）、`name`（错误类名）与 `code`（HTTP 状态码）。
> 注：`packages/hydrooj/src/service/server.ts:229-236` 的 mixin 还会尝试写入 `message`（`error.msg()`）与 `stack`，当取值为 `undefined` 时会被 `JSON.stringify` 丢弃。

**响应分流**（`framework/framework/base.ts:64-110`）

| 条件（按判断顺序） | 输出 |
|---|---|
| 已设置 `response.type`（如 `problem_solution_raw` 的 `text/markdown`） | 跳过以下所有分支，按该类型直接返回 `response.body` |
| `response.pjax` 存在且请求带 `pjax` 参数 | `{ fragments: [{ html }, ...] }`，`Content-Type: application/json` |
| `request.json`（`Accept: application/json`）／设置了 `response.redirect`／query 带 `noTemplate`／无 `response.template` | 直接返回 `this.response.body` 的 JSON |
| 以上都不满足且存在 `response.template` | 渲染该模板并返回 `text/html` |
| 设置了 `response.redirect` 且非 JSON 请求 | HTTP 302 重定向到该地址 |

#### 本部分路由与 HTTP 方法总览

`operation` 一律使用**下划线写法**，框架按 `_${operation}` 转驼峰后派发（`operation=get_links` → `postGetLinks`，`operation=generate_testdata` → `postGenerateTestdata`）。

| 路由名 | 路径 | HTTP 方法 | 处理器方法（行号） |
|---|---|---|---|
| `problem_main` | `/p` | `GET`；`POST`（仅 operation：`copy`/`delete`/`hide`/`unhide`） | `get@110`、`postCopy@203`、`postDelete@238`、`postHide@254`、`postUnhide@267`（**无普通 `post`**） |
| `problem_random` | `/problem/random` | `GET` | `get@282` |
| `problem_detail` | `/p/:pid` | `GET`；`POST`（仅 operation：`rejudge`/`delete`/`star`） | `get@381`、`postRejudge@428`、`postDelete@448`、`postStar@457`（**无普通 `post`**） |
| `problem_submit` | `/p/:pid/submit` | `GET`、`POST` | `prepare@465`、`get@471`、`post@489`（有普通 `post`） |
| `problem_hack` | `/p/:pid/hack/:rid` | `GET`、`POST` | `prepare@555`、`get@574`、`post@588` |
| `problem_edit` | `/p/:pid/edit` | `GET`、`POST` | `get@622`、`post@635` |
| `problem_config` | `/p/:pid/config` | `GET` | `get@650`（**无 POST**） |
| `problem_files` | `/p/:pid/files` | `GET`；`POST`（operation：`get_links`/`upload_file`/`rename_files`/`delete_files`/`generate_testdata`） | `get@671`、`post@681`、`postGetLinks@689`、`postUploadFile@718`、`postRenameFiles@779`、`postDeleteFiles@791`、`postGenerateTestdata@799` |
| `problem_file_download` | `/p/:pid/file/:filename` | `GET` | `get@814` |
| `problem_solution` | `/p/:pid/solution` | `GET`；`POST`（operation：`submit`/`edit_solution`/`delete_solution`/`reply`/`edit_reply`/`delete_reply`/`upvote`/`downvote`） | `get@841`、`postSubmit@875`、`postEditSolution@883`、`postDeleteSolution@892`、`postReply@902`、`postEditReply@912`、`postDeleteReply@924`、`postUpvote@935`、`postDownvote@942`（**无普通 `post`**） |
| `problem_solution_detail` | `/p/:pid/solution/:sid` | 同 `problem_solution` | 同一处理器 |
| `problem_solution_raw` | `/p/:pid/solution/:psid/raw` | `GET` | `get@953` |
| `problem_solution_reply_raw` | `/p/:pid/solution/:psid/:psrid/raw` | `GET` | 同一 `get@953` |
| `problem_statistics` | `/p/:pid/stat` | `GET` | `get@976` |
| `problem_create` | `/problem/create` | `GET`、`POST` | `get@998`、`post@1013` |
| `problem_import_hydro` | `/problem/import/hydro` | `GET`、`POST` | `get@10`、`post@17` |
| `problem_category_compat` | `/p/category/:category` | `GET` | `get@4`（**无 POST**） |
| `record_main` | `/record` | `GET` | `get@38`（**无 POST**） |
| `record_detail` | `/record/:rid` | `GET`、`POST`（普通 `post@224` 做权限校验；operation：`rejudge`/`cancel`） | `prepare@144`、`get@168`、`post@224`、`postRejudge@231`、`postCancel@242` |
| `record_conn` | `/record-conn` | `WebSocket` | `prepare@288` |
| `record_detail_conn` | `/record-detail-conn` | `WebSocket` | `prepare@419` |
| `judge_files_download` | `/judge/files` | `GET`、`POST` | `get@209`、`post@216` |
| `judge_files_upload` | `/judge/upload` | `POST` | `post@265`（**无 GET**） |
| `judge_conn` | `/judge/conn` | `WebSocket` | `prepare@279` |
| `status` | `/status` | `GET` | `get@23`（**无 POST**） |
| `status_update` | `/status/update` | `POST` | `post@71`（**无 GET**） |

---

#### 题目列表 · `problem_main`

```http
GET /p
POST /p
```

| 项 | 值 |
|---|---|
| 处理器 | `ProblemMainHandler`（problem.ts:91）：`get@110`、`postCopy@203`、`postDelete@238`、`postHide@254`、`postUnhide@267`（**无普通 `post`**，POST 仅支持 `operation` 派发） |
| 注册 | `ctx.Route('problem_main', '/p', ProblemMainHandler, PERM.PERM_VIEW_PROBLEM)`（problem.ts:1070） |
| 认证 | 匿名可访问；未登录时 `psdict` 为空对象 |
| 域权限 | `PERM_VIEW_PROBLEM`（注册时强制）；隐藏题需 `PERM_VIEW_PROBLEM_HIDDEN`（否则查询条件被限制为 `hidden:false` 或本人所有/维护） |
| 响应 | JSON 对象 / HTML 模板 `problem_main.html`；`pjax=true` 时响应被替换为 `{ title, fragments: [{html}, ...] }` |

**查询参数**（`@param` → query 或 body）

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `page` | `Types.PositiveInt` | 否 | 默认 `1`；当 `page > 1` 时 `limit` 被强制重置为 `pagination.problem`（problem.ts:113） |
| `q` | `Types.Content` | 否 | 搜索串。经 `parser.parse(q, { keywords:['category','difficulty','namespace'], offsets:false, alwaysArray:true, tokenize:true })` 解析 |
| `limit` | `Types.PositiveInt` | 否 | 每页数量；上限 `pagination.problem`（默认 100，setting.ts:359） |
| `pjax` | `Types.Boolean` | 否 | 为真时只返回 HTML 片段数组 |
| `quick` | `Types.Boolean` | 否 | 为真时只投影 `title/pid/domainId/docId` 四个字段 |
| `sort` | `Types.Range(['default','recent'])` | 否 | 默认 `default`（`{sort:1, docId:1}`）；`recent` 为 `{docId:-1}` 并把 `hint` 改为 `basic` |

`q` 的关键字语义（problem.ts:120-140）：
- `category:xxx` / 纯文本 → 追加 `query.$and = [{ tag }]`（多值用逗号或中文逗号分隔，见 `parseCategory`）
- `difficulty:0` → `{ $in: [0, undefined] }`；`difficulty:n` → `{ $in: [n] }`
- `namespace:xxx` → 若 `domain.namespaces[xxx]` 存在则 `{ sort: /^prefix-/ }`，否则 `{ tag: xxx }`
- 文本部分走 `global.Hydro.module.problemSearch` 的第一个实现，缺省为 `defaultSearch`（problem.ts:61-83），用正则匹配 `pid`/`title`/`tag` 并把命中顺序写入 `queryContext.sort`

**请求体（POST 子操作，`operation`）**

| operation | 说明 | 额外参数（全部 `@param`，即 query 或 body） |
|---|---|---|
| `copy` | 复制题目到目标域（`problem.copy`），要求源域 `share` 允许目标域且目标域用户有 `PERM_CREATE_PROBLEM` | `pids`: `Types.NumericArray`（必填）、`target`: `Types.String`（必填）、`hidden`: `Types.Boolean`、`redirect`: `Types.Boolean`（默认 `false`；为真时 `response.redirect = url('problem_detail', {domainId: target, pid: ids[0]})`） |
| `delete` | 逐题删除（`problem.del`）；非本人题目需 `PERM_EDIT_PROBLEM` | `pids`: `Types.NumericArray`（必填） |
| `hide` | 批量置 `hidden: true` | `pids`: `Types.NumericArray`（必填） |
| `unhide` | 批量置 `hidden: false` | `pids`: `Types.NumericArray`（必填） |

`copy` 的 JSON 响应为新建题目的 `docId` 数组；`delete`/`hide`/`unhide` 调用 `this.back()` → 响应 `{}` 且 `redirect` 为 `Referer`。

**响应字段（`Accept: application/json`，非 pjax）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `page` | number | 当前页码 |
| `pcount` | number | 命中题目总数 |
| `ppcount` | number | 总页数 |
| `pcountRelation` | string | 计数关系，`eq` 或搜索实现返回的 `countRelation` |
| `pdocs` | `ProblemDoc[]` | 题目文档列表（投影见 `problem.PROJECTION_LIST`；`quick=true` 时只有 4 个字段） |
| `psdict` | object | `docId → ProblemStatusDoc`（`score`/`status`/`star`/`rid`）；仅登录用户填充 |
| `qs` | string | 原样回显的 `q` |
| `sort` | string | 排序策略回显 |

**响应字段（`pjax=true`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `title` | string | `renderTitle(translate('problem_main'))` |
| `fragments` | `{html:string}[]` | 依次渲染 `partials/problem_list.html`、`partials/problem_stat.html`、`partials/problem_lucky.html` |

**示例**

```http
GET /d/system/p?page=1&limit=20&sort=recent
Accept: application/json
```
```json
{
  "page": 1,
  "pcount": 1,
  "ppcount": 1,
  "pcountRelation": "eq",
  "pdocs": [{ "_id": "652f...", "docId": 1000, "pid": "P1000", "title": "A+B Problem", "nSubmit": 12, "nAccept": 8, "tag": [], "hidden": false }],
  "psdict": {},
  "qs": "",
  "sort": "recent"
}
```

---

#### 随机题目 · `problem_random`

```http
GET /problem/random
```

| 项 | 值 |
|---|---|
| 处理器 | `ProblemRandomHandler`（problem.ts:280），唯一方法 `get@282`（**无 POST**） |
| 注册 | `ctx.Route('problem_random', '/problem/random', ProblemRandomHandler, PERM.PERM_VIEW_PROBLEM)`（problem.ts:1071） |
| 认证 | 匿名可访问 |
| 域权限 | `PERM_VIEW_PROBLEM` |
| 响应 | JSON 对象 `{ pid, url }`；无题目时抛 `NoProblemError` |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `q` | `Types.Content` | 否 | 只解析 `category:` 前缀（按空格切分），生成 `q.$and = [{tag}, ...]`；其余内容被忽略 |

**响应字段**

| 字段 | 类型 | 说明 |
|---|---|---|
| `pid` | `string \| number` | 随机命中的题目 `pid`（无 `pid` 时回退 `docId`，见 `problem.random` model/problem.ts:376） |
| `url` | string | 由 base 层自动附加的重定向地址（`problem_detail`） |

**示例**

```http
GET /d/system/problem/random?q=category:动态规划
Accept: application/json
```
```json
{ "pid": "P1001", "url": "/d/system/p/P1001" }
```

---

#### 题目详情 · `problem_detail`

```http
GET /p/:pid
POST /p/:pid
```

| 项 | 值 |
|---|---|
| 处理器 | `ProblemDetailHandler`（problem.ts:296）：`_prepare@303`、`get@381`、`postRejudge@428`、`postDelete@448`、`postStar@457`（**无普通 `post`**，POST 仅支持 `operation` 派发） |
| 注册 | `ctx.Route('problem_detail', '/p/:pid', ProblemDetailHandler)`（**未声明权限**，problem.ts:1072） |
| 认证 | 匿名可访问；可见性由 `_prepare` 中的 `problem.canViewBy(this.pdoc, this.user)` 决定（要求 `PERM_VIEW_PROBLEM`，隐藏题要求 `PERM_VIEW_PROBLEM_HIDDEN` 或本人所有） |
| 域权限 | 无注册级权限；`canViewBy` 内部要求 `PERM_VIEW_PROBLEM`；`postRejudge` 需 `PERM_REJUDGE_PROBLEM`；`postDelete` 需本人（`PERM_EDIT_PROBLEM_SELF`）或 `PERM_EDIT_PROBLEM` |
| 响应 | JSON 对象 / HTML 模板 `problem_detail.html` |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `pid` | `Types.ProblemId` | 题目 ID，可为数字 `docId` 或字符串 `pid`（`validator.ts:100`：`/^(?:[a-z0-9]{1,10}-)?[a-z0-9]+$/i`，纯数字自动转 number） |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `tid` | `Types.ObjectId` | 否 | 比赛/作业 ID（`@query`，仅 query）。传入后进入比赛模式：校验 `tdoc.pids` 包含该题、比赛已开始、已报名；并清空 `tag`，删除 `nAccept`/`nSubmit`/`difficulty`/`stats` |
| `pjax` | `Types.Boolean` | 否 | 为真时响应被整体替换为 `{ title, fragments:[{html}], raw:{pdoc,tdoc} }`（仅渲染 `partials/problem_description.html`） |

`_prepare` 还会做（problem.ts:303-377）：
1. 若 `pdoc.reference` 存在，则用源域的 `config` 与 `additional_file` 覆盖；
2. 若 `pdoc.config` 为对象（已解析），计算 `pdoc.config.langs`：
   - 依次收集 `config.langs`、`domain.langs`、`this.domain.langs`、`tdoc.langs` 四层白名单；
   - `remote_judge` 类型：取 `setting.langs` 中以 `subType.` 开头或 `validAs[subType]` 的语言，外加 `subType` 本身；
   - 其它类型：若存在白名单则取所有非 `remote` 语言，否则取所有非 `remote` 且非 `hidden` 的语言，再与白名单求交集；
   - `objective` / `submit_answer` 类型固定为 `['_']`；
3. 触发 `ctx.parallel('problem/get', pdoc, this)`；
4. 并行读取 `psdoc`（`problem.getStatus`）与 `udoc`（题主 `user.getById`）；
5. 统计 `solutionCount`（`parentId = docId`）与 `discussionCount`。

`get` 额外行为（problem.ts:381-425）：
- 非 JSON 请求（或 `pjax`）时，把正文里的 `file://xxx` 重写为 `./<docId>/file/xxx`（存在 `additional_file` 命中时），比赛模式附加 `tid` 查询串；
- 设置 `page_name`：`problem_detail` / `contest_detail_problem` / `homework_detail_problem`；
- 无 `tdoc` 时：若 `psdoc.rid` 存在则取 `rdoc`；并填充 `ctdocs`（`contest.getRelated`）与 `htdocs`（`rule='homework'`），按 `PERM_VIEW_HIDDEN_CONTEST` 或 `assign` 组过滤。

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `pdoc` | `ProblemDoc` | 题目文档；含 `_id`、`docId`、`pid`、`domainId`、`docType`、`owner`、`title`、`content`、`html`、`nSubmit`、`nAccept`、`tag`、`data`、`additional_file`、`stats`、`difficulty`、`hidden`、`reference`、`maintainer`、`config`（解析后的对象或错误字符串） |
| `udoc` | `User` | 题主用户对象 |
| `psdoc` | `ProblemStatusDoc \| null` | 当前用户在该题的状态（`score`/`status`/`star`/`rid`）；比赛模式（有 `tid`）时为 `null` |
| `title` | string | 题目标题 |
| `solutionCount` | number | 题解数 |
| `discussionCount` | number | 讨论数 |
| `tdoc` | `Tdoc \| undefined` | 比赛/作业文档 |
| `owner_udoc` | `User \| null` | 当比赛题主与题主不同时给出的比赛创建者 |
| `mode` | string | `normal`（无 `tid`）/ `view`（未报名）/ `contest`（进行中）/ `correction`（已结束且可见）/ `none` |
| `tsdoc` | object | 仅当 `tdoc && tsdoc` 存在；经 `pick` 裁剪为 `attend`、`startAt`、`endAt`，以及 `canShowSelfRecord` 为真时的 `detail` |
| `page_name` | string | `problem_detail` / `contest_detail_problem` / `homework_detail_problem` |
| `rdoc` | `RecordDoc` | 仅当无 `tdoc` 且 `psdoc.rid` 存在 |
| `ctdocs` | `Tdoc[]` | 仅当无 `tdoc`；关联比赛列表 |
| `htdocs` | `Tdoc[]` | 仅当无 `tdoc`；关联作业列表（`rule='homework'`） |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| `rejudge` | 重测该题全部评测记录（排除 `generate`/`pretest`、`STATUS_CANCELED`、有 `files.hack` 的记录），先 `record.reset` 再按是否比赛分别入队；需 `PERM_REJUDGE_PROBLEM` | `pid`: `Types.UnsignedInt`（`@param`，必填；注意会覆盖路径参数） |
| `delete` | 删除题目；若被比赛引用抛 `ProblemAlreadyUsedByContestError`；需本人（`PERM_EDIT_PROBLEM_SELF`）或 `PERM_EDIT_PROBLEM` | — |
| `star` | 收藏/取消收藏（`problem.setStar`），`back({ star })` | `star`: `Types.Boolean`（隐含可选） |

**示例**

```http
GET /d/system/p/P1000
Accept: application/json
```
```json
{
  "pdoc": { "docId": 1000, "pid": "P1000", "title": "A+B Problem", "config": { "type": "default", "count": 10, "memoryMin": 256, "memoryMax": 256, "timeMin": 1000, "timeMax": 1000, "langs": ["cc", "py"], "hackable": false } },
  "udoc": { "_id": 2, "uname": "admin" },
  "psdoc": null,
  "title": "A+B Problem",
  "solutionCount": 0,
  "discussionCount": 0,
  "tdoc": null,
  "owner_udoc": null,
  "mode": "normal",
  "page_name": "problem_detail",
  "ctdocs": [],
  "htdocs": []
}
```

---

#### 提交代码 · `problem_submit`

```http
GET /p/:pid/submit
POST /p/:pid/submit
```

| 项 | 值 |
|---|---|
| 处理器 | `ProblemSubmitHandler extends ProblemDetailHandler`（problem.ts:463）：`prepare@465`、`get@471`、`post@489`（**有普通 `post`**，即真正的提交入口） |
| 注册 | `ctx.Route('problem_submit', '/p/:pid/submit', ProblemSubmitHandler, PERM.PERM_SUBMIT_PROBLEM)`（problem.ts:1073） |
| 认证 | 需登录（`checkPerm` 在未登录时抛 `PrivilegeError(PRIV_USER_PROFILE)`） |
| 域权限 | `PERM_SUBMIT_PROBLEM` |
| 响应 | GET：JSON 对象 / HTML 模板 `problem_submit.html`；POST：JSON `{ rid }`（或比赛模式 `{ tid }`）+ 重定向 |

继承链：`ProblemDetailHandler._prepare` 先执行（填充 `pdoc`/`udoc`/`psdoc`/`tdoc`/`tsdoc` 等），随后执行本类 `prepare`。

`prepare(domainId, tid?)`（`@param('tid', Types.ObjectId, true)`）校验：
- `tid` 存在且比赛非进行中 → `ContestNotLiveError`；
- `pdoc.config` 是字符串（解析失败）→ `ProblemConfigError`；
- `pdoc.config.langs` 存在但为空数组 → `ProblemConfigError`。

**查询参数（GET）**：无（`get()` 无装饰器，使用 `_prepare` 的结果）

**请求体（POST）**（全部 `@param` → query 或 body）

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `lang` | `Types.Name` | 是 | 语言 key（`/^.{1,255}$/`）。`objective`/`submit_answer` 类型会被强制改为 `_` |
| `code` | `Types.String` | 否 | 源代码文本；为空时回落到上传文件 `file` |
| `pretest` | `Types.Boolean` | 否 | 默认 `false`。为真时走自测流程 |
| `input` | `Types.ArrayOf(Types.String, true)` | 否 | 自测输入数组（`pretest=true` 时必填非空，否则 `ValidationError('input')`） |
| `tid` | `Types.ObjectId` | 否 | 比赛/作业 ID |
| `file` | 文件（multipart `request.files.file`） | 否 | 当 `code` 为空时使用；`submit_answer` 类型上限 128MiB，其它类型上限 `limit.codelength`（默认 128KiB） |

语言校验（problem.ts:491-497）：
- `config.type ∈ {submit_answer, objective}` → `lang = '_'`；
- 否则 `config.langs` 不含该语言、或 `setting.langs[lang]` 不存在、或 `setting.langs[lang].disabled` → `ProblemNotAllowLanguageError`。

自测校验（problem.ts:498-505）：
- `setting.langs[lang].pretest` 存在时替换 `lang`；
- `pdoc.config.type` 必须是 `default` 或 `remote_judge`，否则 `ProblemNotAllowPretestError`。

限流（problem.ts:506-507）：`limitRate('add_record', 60, system.get('limit.submission_user'), '{{user}}')` 与 `limitRate('add_record', 60, pretest ? limit.pretest : limit.submission)`。

大文件处理：当文件为二进制（`lang === '_'`、`.zip` 或 `setting.langs[lang].isBinary`）或超过代码长度上限时，文件被写入存储 `submission/${uid}/${nanoid}`，记录 `files.code = "<uid>/<id>#<originalFilename>"`，不入库源码。

**响应字段（GET，`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `pdoc` `udoc` `psdoc` `title` `solutionCount` `discussionCount` `tdoc` `owner_udoc` `mode` | — | 同 `problem_detail` |
| `page_name` | string | `problem_submit` / `contest_detail_problem_submit` / `homework_detail_problem_submit` |
| `langRange` | object | `{ langKey: displayName }`。来源：`pdoc.config.langs` 映射 `setting.langs[i].display`；若 `config` 无 `langs` 则用 `setting.SETTINGS_BY_KEY.codeLang.range` |

**响应字段（POST，`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `rid` | string | 新建评测记录的 `_id`；随后重定向到 `record_detail` |
| `tid` | string | 仅当 `tid && !pretest && !contest.canShowSelfRecord(...)` 时返回；重定向到 `contest_problemlist`（作业为 `homework_detail`） |
| `url` | string | 重定向地址（base 层附加） |

提交副作用：非 `pretest` 时并行执行 `problem.inc(domainId, docId, 'nSubmit', 1)`、`domain.incUserInDomain(domainId, uid, 'nSubmit')`、以及比赛时的 `contest.updateStatus`。

**示例**

```http
POST /d/system/p/P1000/submit
Accept: application/json
Content-Type: application/json

{ "lang": "cc", "code": "#include <iostream>\nint main(){int a,b;std::cin>>a>>b;std::cout<<a+b;}" }
```
```json
{ "rid": "6530f0c1a1b2c3d4e5f60718", "url": "/d/system/record/6530f0c1a1b2c3d4e5f60718" }
```

---

#### 题目 Hack · `problem_hack`

```http
GET /p/:pid/hack/:rid
POST /p/:pid/hack/:rid
```

| 项 | 值 |
|---|---|
| 处理器 | `ProblemHackHandler extends ProblemDetailHandler`（problem.ts:550）：`prepare@555`、`get@574`、`post@588`（有普通 `post`） |
| 注册 | `ctx.Route('problem_hack', '/p/:pid/hack/:rid', ProblemHackHandler, PERM.PERM_SUBMIT_PROBLEM)`（problem.ts:1074） |
| 认证 | 需登录 |
| 域权限 | `PERM_SUBMIT_PROBLEM`；此外要求本人已 AC 该题 |
| 响应 | GET：JSON 对象 / HTML 模板 `problem_hack.html`；POST：JSON `{ rid, url }` |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `pid` | `Types.ProblemId` | 题目 ID（继承自 `_prepare`） |
| `rid` | `Types.ObjectId` | 被 hack 的目标记录 ID（`@param`，即 query 或 body） |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `tid` | `Types.ObjectId` | 否 | 比赛 ID（`@param`）。传入后要求比赛 `rule === 'codeforces'` 且正在进行 |

`prepare` 校验（problem.ts:556-573），任一不满足抛 `HackFailedError`/`RecordNotFoundError`：
1. `pdoc.config.hackable` 必须为真（`hackable = validator && checker && checker_type ∉ {default, strict}`，见 `lib/testdataConfig.ts:18`）；
2. 记录存在、`rdoc.pid === pdoc.docId`、且 `rdoc.contest` 与 `tid` 一致；
3. `tid` 时比赛规则为 `codeforces` 且 `contest.isOngoing`；
4. 不能 hack 自己（或比赛队友）的提交；
5. 必须已通过该题（`psdoc.status === STATUS_ACCEPTED` 或比赛详情中该题 AC）；
6. 目标记录本身必须是 `STATUS_ACCEPTED`。

**请求体（POST）**（`@param`）

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `input` | `Types.String` | 否 | Hack 输入数据（纯文本） |
| `autoOrganizeInput` | `Types.Boolean` | 否 | 默认 `false`；为真时压缩空白：`\s+\n` → `\n`、`\s+ ` → ` ` |
| `tid` | `Types.ObjectId` | 否 | 比赛 ID |
| `file` | 文件（multipart `request.files.file`） | 否 | 优先于 `input`；大小上限 2MiB，超限抛 `ValidationError('input')` |

行为：输入写入 `submission/${uid}/${nanoid}`，然后 `record.add(..., type:'hack', hackTarget: rdoc._id, files: { hack: '<uid>/<id>#input.txt' })`，语言与代码复用目标记录（`this.rdoc.lang` / `this.rdoc.code`）。

**响应字段（GET，`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `pdoc` | `ProblemDoc` | 题目文档 |
| `udoc` | `User` | 题主 |
| `rid` | string | 被 hack 的目标记录 ID |
| `title` | string | 题目标题 |
| `page_name` | string | `problem_hack` / `contest_detail_problem_hack` |

**响应字段（POST）**：`{ "rid": "<新记录ID>", "url": "<record_detail 地址>" }`

**示例**

```http
POST /d/system/p/P1000/hack/6530f0c1a1b2c3d4e5f60718
Accept: application/json
Content-Type: application/json

{ "input": "1 2\n", "autoOrganizeInput": true }
```
```json
{ "rid": "6530f1a2b3c4d5e6f7081920", "url": "/d/system/record/6530f1a2b3c4d5e6f7081920" }
```

---

#### 编辑题目 · `problem_edit`

```http
GET /p/:pid/edit
POST /p/:pid/edit
```

| 项 | 值 |
|---|---|
| 处理器 | `ProblemEditHandler extends ProblemManageHandler`（problem.ts:621）：`get@622`、`post@635`（有普通 `post`） |
| 父类 | `ProblemManageHandler`（problem.ts:615，唯一方法 `prepare@616`）：本人（`PERM_EDIT_PROBLEM_SELF`）否则 `checkPerm(PERM_EDIT_PROBLEM)` |
| 注册 | `ctx.Route('problem_edit', '/p/:pid/edit', ProblemEditHandler)`（problem.ts:1075） |
| 认证 | 需登录 |
| 域权限 | `PERM_EDIT_PROBLEM_SELF`（本人题目）或 `PERM_EDIT_PROBLEM` |
| 响应 | GET：JSON 对象 / HTML 模板 `problem_edit.html`；POST：重定向到 `problem_detail` |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `pid` | `Types.ProblemId` | 题目 ID（`_prepare` 用）；`post` 上另有 `@route('pid', Types.ProblemId)` 取原 `pid` |

**请求体（POST，全部 `@post`，仅 body）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `title` | `Types.Title` | 是 | 标题（1–64 字符） |
| `content` | `Types.Content` | 是 | 题面（Markdown/HTML，<65536 字符） |
| `pid` | `Types.ProblemId` + 自定义校验 `/^(?:[a-z0-9]{1,10}-)?[a-z][a-z0-9]*$/i` | 否 | 新 `pid`；纯数字会补前缀 `P`（`P${newPid}`）；与已存在 `pid` 冲突抛 `ProblemAlreadyExistError` |
| `hidden` | `Types.Boolean` | 否 | 默认 `false` |
| `tag` | `Types.Content` + `parseCategory` 转换 | 否 | 分类，逗号（含中文逗号）分隔字符串或数组 |
| `difficulty` | `Types.PositiveInt` + 校验 `+i <= 10` | 否 | 难度 0–10，默认 `0` |

更新字段：`{ title, content, pid, hidden, tag, difficulty, html: false }`（`html:false` 表示题面为 Markdown 需渲染）。

**响应字段（GET，`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `pdoc` `udoc` `psdoc` `title` `solutionCount` `discussionCount` `tdoc` `owner_udoc` `mode` | — | 同 `problem_detail` |
| `additional_file` | `FileInfo[]` | 附加文件（经 `sortFiles` 排序） |
| `statementLangs` | string[] | `ctx.i18n.langs(false)`，可用题面语言列表 |

**响应字段（POST）**：`{}` + `url`（`problem_detail`，使用新 `pid` 或 `docId`）

**示例**

```http
POST /d/system/p/P1000/edit
Accept: application/json
Content-Type: application/json

{ "title": "A+B Problem", "content": "给定 a,b，输出 a+b。", "tag": "模拟,入门", "difficulty": 1 }
```
```json
{ "url": "/d/system/p/P1000" }
```

---

#### 题目配置页 · `problem_config`

```http
GET /p/:pid/config
```

| 项 | 值 |
|---|---|
| 处理器 | `ProblemConfigHandler extends ProblemManageHandler`（problem.ts:649），唯一方法 `get@650` |
| 注册 | `ctx.Route('problem_config', '/p/:pid/config', ProblemConfigHandler)`（problem.ts:1076） |
| 认证 | 需登录 |
| 域权限 | `PERM_EDIT_PROBLEM_SELF` 或 `PERM_EDIT_PROBLEM` |
| 响应 | JSON 对象 / HTML 模板 `problem_config.html`；**仅支持 GET**，POST 无对应方法 → `MethodNotAllowedError`（405） |

`get` 行为：若 `pdoc.reference` 存在抛 `ProblemIsReferencedError('edit config')`；否则读取测试数据目录下的 `config.yaml` 原文（读取失败时静默返回空串）。

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `pdoc` `udoc` `psdoc` `title` `solutionCount` `discussionCount` `tdoc` `owner_udoc` `mode` | — | 同 `problem_detail` |
| `testdata` | `FileInfo[]` | 测试数据文件列表（`sortFiles`） |
| `config` | string | `config.yaml` 的原始文本内容；不存在或读取失败时为 `""` |

**示例**

```http
GET /d/system/p/P1000/config
Accept: application/json
```
```json
{ "pdoc": { "docId": 1000 }, "testdata": [{ "name": "1.in", "size": 3 }], "config": "type: default\ntime: 1s\nmemory: 256m\n" }
```

---

#### 题目文件管理 · `problem_files`

```http
GET /p/:pid/files
POST /p/:pid/files
```

| 项 | 值 |
|---|---|
| 处理器 | `ProblemFilesHandler extends ProblemDetailHandler`（problem.ts:666）：`get@671`、`post@681`（派发前置检查）、`postGetLinks@689`、`postUploadFile@718`、`postRenameFiles@779`、`postDeleteFiles@791`、`postGenerateTestdata@799` |
| 注册 | `ctx.Route('problem_files', '/p/:pid/files', ProblemFilesHandler, PERM.PERM_VIEW_PROBLEM)`（problem.ts:1077） |
| 认证 | 匿名可访问（查看）；写操作需登录 |
| 域权限 | `PERM_VIEW_PROBLEM`（注册级）；读测试数据需本人或 `PERM_READ_PROBLEM_DATA`（或特权 `PRIV_READ_PROBLEM_DATA`）；写操作需本人（`PERM_EDIT_PROBLEM_SELF`）或 `PERM_EDIT_PROBLEM` |
| 响应 | GET：JSON 对象 / HTML 模板 `problem_files.html`（`pjax` 片段见下）；POST：视子操作而定 |
| 备注 | 处理器声明 `notUsage = true`（不参与用量统计） |

**查询参数（GET，`@param`）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `d` | `Types.CommaSeperatedArray` | 否 | 需要渲染的 pjax 片段类型，默认 `['testdata','additional_file']` |
| `sidebar` | `Types.Boolean` | 否 | 默认 `false`；为假时额外追加 `partials/problem-sidebar-information.html` 片段 |

比赛模式下（`this.tdoc` 存在）抛 `ContestNotEndedError`。

**响应字段（GET，`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `pdoc` `udoc` `psdoc` `title` `solutionCount` `discussionCount` `tdoc` `owner_udoc` `mode` | — | 同 `problem_detail` |
| `testdata` | `FileInfo[]` | 测试数据列表 |
| `additional_file` | `FileInfo[]` | 附加文件列表 |
| `reference` | object | `pdoc.reference`（`{domainId, pid}`）或 `undefined` |

> 若请求带 `pjax` 且非 JSON，`response.pjax` 被设为 `[['partials/problem_files.html', {filetype, sidebar, can_edit:true}], ...]`，base 层会输出 `{ fragments: [{html}] }`。

**POST 派发前置检查（problem.ts:681-685）**

```js
async post() {
    if (this.args.operation === 'get_links') return;   // 唯一放行的 operation
    if (this.pdoc.reference) throw new ProblemIsReferencedError('edit files');
    if (!this.user.own(this.pdoc, PERM.PERM_EDIT_PROBLEM_SELF)) this.checkPerm(PERM.PERM_EDIT_PROBLEM);
}
```

即：除 `get_links` 外的所有子操作都要求题目编辑权限，且引用题（`reference`）不可编辑文件。

**POST 子操作（`operation`）**

| operation | 处理器 | 参数（装饰器来源见括号） | 说明与响应 |
|---|---|---|---|
| `get_links` | `postGetLinks`（problem.ts:689） | `files`: `Types.Set`（`@post`，必填）、`type`: `Types.Range(['testdata','additional_file'])`（`@post`，默认 `testdata`） | 生成带签名的下载链接。测试数据需本人或 `PERM_READ_PROBLEM_DATA`；比赛未结束时抛 `ContestNotEndedError`；记录 oplog `download.problem.bulk`。响应 `{ links: { filename: url } }` |
| `upload_file` | `postUploadFile`（problem.ts:718） | `filename`: `Types.Filename`（`@post`，可选，缺省用上传文件名或随机 16 位串）、`type`: `Types.Range(['testdata','additional_file'])`（`@post`，默认 `testdata`）、文件 `file`（multipart，必填，否则 `ValidationError('file')`） | `testdata` 且文件名为 `.zip` 时用 `@zip.js/zip.js` 解包逐个写入（跳过目录项，文件名经 `sanitize`）；非特权用户受 `limit.problem_files_max`（默认 100）与 `limit.problem_files_max_size`（默认 256MiB）限制，超限抛 `FileLimitExceededError('count'\|'size')`。调用 `problem.addTestdata` 或 `problem.addAdditionalFile`。响应 `this.back()` → `{}` + `url` |
| `rename_files` | `postRenameFiles`（problem.ts:779） | `files`: `Types.ArrayOf(Types.Filename)`（`@post`，必填）、`newNames`: `Types.ArrayOf(Types.Filename)`（`@post`，必填）、`type`: 同上（默认 `testdata`） | 数组长度不一致抛 `ValidationError('files','newNames')`；逐项 `problem.renameTestdata` / `problem.renameAdditionalFile`。响应 `this.back()` |
| `delete_files` | `postDeleteFiles`（problem.ts:791） | `files`: `Types.ArrayOf(Types.Filename)`（`@post`，必填）、`type`: 同上（默认 `testdata`） | `problem.delTestdata` / `problem.delAdditionalFile`。响应 `this.back()` |
| `generate_testdata` | `postGenerateTestdata`（problem.ts:799） | `std`: `Types.Filename`（`@post`，必填）、`gen`: `Types.Filename`（`@post`，必填） | 二者必须都存在于 `pdoc.data` 中，否则 `BadRequestError`；创建 `type:'generate'` 的评测记录（语言 `_`，代码为 `"<gen>\n<std>"`）。响应 `{ url }` → 重定向到 `record_detail` |

**示例**

```http
POST /d/system/p/P1000/files
Accept: application/json
Content-Type: application/json

{ "operation": "get_links", "files": ["1.in", "1.out"], "type": "testdata" }
```
```json
{ "links": { "1.in": "https://.../1.in?sig=...", "1.out": "https://.../1.out?sig=..." } }
```

---

#### 题目文件下载 · `problem_file_download`

```http
GET /p/:pid/file/:filename
```

| 项 | 值 |
|---|---|
| 处理器 | `ProblemFileDownloadHandler extends ProblemDetailHandler`（problem.ts:809），唯一方法 `get@814`（**无 POST**） |
| 注册 | `ctx.Route('problem_file_download', '/p/:pid/file/:filename', ProblemFileDownloadHandler)`（problem.ts:1078） |
| 认证 | 匿名可访问；无 `tid` 时要求 `PERM_VIEW_PROBLEM` |
| 域权限 | `PERM_VIEW_PROBLEM`；`type=testdata` 且非本人时需 `PERM_READ_PROBLEM_DATA`（或特权 `PRIV_READ_PROBLEM_DATA`） |
| 响应 | 302 重定向到签名下载地址；JSON 模式下为 `{ url }` |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `pid` | `Types.ProblemId` | 题目 ID |
| `filename` | `Types.Filename`（`@param`） | 文件名，`/^[^\\/?#~!\|*]{1,255}$/` 且 `sanitize(i) === i` |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `type` | `Types.Range(['additional_file','testdata'])` | 否 | 默认 `additional_file`（`@query`，仅 query） |
| `tid` | `Types.ObjectId` | 否 | 有 `tid` 时跳过 `PERM_VIEW_PROBLEM` 校验（比赛场景） |
| `noDisposition` | `Types.Boolean` | 否 | 为真时不设置下载文件名（内联浏览） |

行为：引用题下载测试数据抛 `ProblemIsReferencedError('download testdata')`，下载附加文件时改用源题目；记录 oplog `download.problem.single`；最终 `this.response.redirect = await storage.signDownloadLink(...)`。

**示例**

```http
GET /d/system/p/P1000/file/statement.pdf?type=additional_file
Accept: application/json
```
```json
{ "url": "https://cdn.example.com/problem/system/1000/additional_file/statement.pdf?..." }
```

---

#### 题解列表 · `problem_solution`

```http
GET /p/:pid/solution
POST /p/:pid/solution
```

| 项 | 值 |
|---|---|
| 处理器 | `ProblemSolutionHandler extends ProblemDetailHandler`（problem.ts:837）：`get@841`、`postSubmit@875`、`postEditSolution@883`、`postDeleteSolution@892`、`postReply@902`、`postEditReply@912`、`postDeleteReply@924`、`postUpvote@935`、`postDownvote@942`（**无普通 `post`**，POST 仅支持 `operation` 派发） |
| 注册 | `ctx.Route('problem_solution', '/p/:pid/solution', ProblemSolutionHandler, PERM.PERM_VIEW_PROBLEM)`（problem.ts:1079） |
| 认证 | 匿名可访问；未通过题目时要求 `PERM_VIEW_PROBLEM_SOLUTION` |
| 域权限 | `PERM_VIEW_PROBLEM`（注册级）；阅读权限：本人已 AC 或 `PERM_VIEW_PROBLEM_SOLUTION_ACCEPT`，否则 `PERM_VIEW_PROBLEM_SOLUTION` |
| 响应 | JSON 对象 / HTML 模板 `problem_solution.html` |

**查询参数（GET，`@param`）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `page` | `Types.PositiveInt` | 否 | 默认 `1`，分页键 `solution`（`pagination.solution` 默认 20） |
| `tid` | `Types.ObjectId` | 否 | 只要传了就抛 `PermissionError(PERM_VIEW_PROBLEM_SOLUTION)`（比赛内禁止看题解） |
| `sid` | `Types.ObjectId` | 否 | 只取单条题解（用于 `problem_solution_detail` 路由） |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `pdoc` `udoc` `psdoc` `title` `solutionCount` `discussionCount` `tdoc` `owner_udoc` `mode` | — | 同 `problem_detail` |
| `psdocs` | `SolutionDoc[]` | 题解文档数组（含 `docId`、`owner`、`content`、`vote`、`reply` 等）；有 `sid` 时为单元素数组 |
| `page` | number | 当前页 |
| `pcount` | number | 总页数 |
| `pscount` | number | 题解总数 |
| `udict` | object | `uid → User`（含题主、题解作者、回复作者） |
| `pssdict` | object | `psid → 状态`（`solution.getListStatus`，含用户投票） |
| `sid` | string \| undefined | 回显的 `sid` |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数（`@param`） | 权限 |
|---|---|---|---|
| `submit` | 发表题解（`solution.add`），`back({ psid })` | `content`: `Types.Content`（必填） | `PERM_CREATE_PROBLEM_SOLUTION` |
| `edit_solution` | 编辑题解（`solution.edit`），`back({ psdoc })` | `content`: `Types.Content`（必填）、`psid`: `Types.ObjectId`（必填） | 本人 `PERM_EDIT_PROBLEM_SOLUTION_SELF`，否则 `PERM_EDIT_PROBLEM_SOLUTION` |
| `delete_solution` | 删除题解（`solution.del`） | `psid`: `Types.ObjectId`（必填） | 本人 `PERM_DELETE_PROBLEM_SOLUTION_SELF`，否则 `PERM_DELETE_PROBLEM_SOLUTION` |
| `reply` | 回复题解（`solution.reply`） | `psid`、`content`（均必填） | `PERM_REPLY_PROBLEM_SOLUTION` |
| `edit_reply` | 编辑回复（`solution.editReply`） | `psid`、`psrid`、`content`（均必填） | 必须本人且持有 `PERM_EDIT_PROBLEM_SOLUTION_REPLY_SELF` |
| `delete_reply` | 删除回复（`solution.delReply`） | `psid`、`psrid`（均必填） | 本人且 `PERM_DELETE_PROBLEM_SOLUTION_REPLY_SELF`，否则 `PERM_DELETE_PROBLEM_SOLUTION_REPLY` |
| `upvote` | 点赞（`solution.vote(..., 1)`），`back({ vote, user_vote: 1 })` | `psid`（必填） | `PERM_VOTE_PROBLEM_SOLUTION` |
| `downvote` | 点踩（`solution.vote(..., -1)`），`back({ vote, user_vote: -1 })` | `psid`（必填） | `PERM_VOTE_PROBLEM_SOLUTION` |

`edit_reply` / `delete_reply` 会校验 `psdoc.parentId === pdoc.docId`，否则抛 `SolutionNotFoundError`。

**示例**

```http
POST /d/system/p/P1000/solution
Accept: application/json
Content-Type: application/json

{ "operation": "submit", "content": "直接用 `cin >> a >> b` 即可。" }
```
```json
{ "psid": "6530f2b3c4d5e6f708192a3b", "url": "/d/system/p/P1000/solution" }
```

---

#### 题解详情 · `problem_solution_detail`

```http
GET /p/:pid/solution/:sid
POST /p/:pid/solution/:sid
```

| 项 | 值 |
|---|---|
| 处理器 | 与 `problem_solution` **同一个处理器** `ProblemSolutionHandler`（problem.ts:837），方法集相同（`get@841` + 8 个 `postXxx`） |
| 注册 | `ctx.Route('problem_solution_detail', '/p/:pid/solution/:sid', ProblemSolutionHandler, PERM.PERM_VIEW_PROBLEM)`（problem.ts:1080） |
| 认证 | 同 `problem_solution` |
| 域权限 | `PERM_VIEW_PROBLEM`；阅读权限规则同 `problem_solution` |
| 响应 | JSON 对象 / HTML 模板 `problem_solution.html` |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `sid` | — | URL 中的 `:sid`。注意 `get` 上的 `@param('sid', Types.ObjectId, true)` 会覆盖路径参数，故 `sid` 亦可由 query/body 提供 |

**响应字段**：与 `problem_solution` 相同，但 `psdocs` 只含该 `sid` 对应的题解（不存在则 `SolutionNotFoundError`），`page` 恒为 `1`。

**POST 子操作**：与 `problem_solution` 完全一致（同一处理器）。

**示例**

```http
GET /d/system/p/P1000/solution/6530f2b3c4d5e6f708192a3b
Accept: application/json
```
```json
{ "psdocs": [{ "_id": "6530f2b3c4d5e6f708192a3b", "docId": "6530f2b3c4d5e6f708192a3b", "owner": 2, "content": "..." }], "page": 1, "pcount": 1, "pscount": 1, "sid": "6530f2b3c4d5e6f708192a3b" }
```

---

#### 题解原文 · `problem_solution_raw`

```http
GET /p/:pid/solution/:psid/raw
```

| 项 | 值 |
|---|---|
| 处理器 | `ProblemSolutionRawHandler extends ProblemDetailHandler`（problem.ts:949），唯一方法 `get@953`（**无 POST**） |
| 注册 | `ctx.Route('problem_solution_raw', '/p/:pid/solution/:psid/raw', ProblemSolutionRawHandler, PERM.PERM_VIEW_PROBLEM)`（problem.ts:1081） |
| 认证 | 匿名可访问；未 AC 时要求 `PERM_VIEW_PROBLEM_SOLUTION` |
| 域权限 | `PERM_VIEW_PROBLEM`；阅读权限规则同 `problem_solution` |
| 响应 | **`text/markdown` 原文**（设置 `response.type`，因此即使 `Accept: application/json` 也不走 JSON 序列化，直接返回题解 Markdown 文本） |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `psid` | `Types.ObjectId`（`@param`） | 题解 ID |
| `psrid` | `Types.ObjectId`（`@route`，可选） | 该路由中恒为空（由下一条路由使用） |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `tid` | `Types.ObjectId` | 否 | 只要传了就抛 `PermissionError(PERM_VIEW_PROBLEM_SOLUTION)` |

**响应**：`psdoc.content` 原始 Markdown；若 `psdoc` 不存在（`solution.get` 抛 `SolutionNotFoundError`）则报错。

**示例**

```http
GET /d/system/p/P1000/solution/6530f2b3c4d5e6f708192a3b/raw
```
```text
直接用 `cin >> a >> b` 即可。
```

---

#### 题解回复原文 · `problem_solution_reply_raw`

```http
GET /p/:pid/solution/:psid/:psrid/raw
```

| 项 | 值 |
|---|---|
| 处理器 | `ProblemSolutionRawHandler`（problem.ts:949），同一 `get@953` 方法（**无 POST**） |
| 注册 | `ctx.Route('problem_solution_reply_raw', '/p/:pid/solution/:psid/:psrid/raw', ProblemSolutionRawHandler, PERM.PERM_VIEW_PROBLEM)`（problem.ts:1082） |
| 认证 / 域权限 | 同 `problem_solution_raw` |
| 响应 | **`text/markdown` 原文**（回复正文），不返回 JSON |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `psid` | `Types.ObjectId`（`@param`） | 题解 ID |
| `psrid` | `Types.ObjectId`（`@route`，可选） | 回复 ID |

校验：`solution.getReply(domainId, psid, psrid)`；`psdoc` 不存在或 `psdoc.parentId !== pdoc.docId` 时抛 `SolutionNotFoundError(psid, psrid)`。

**示例**

```http
GET /d/system/p/P1000/solution/6530f2b3c4d5e6f708192a3b/6530f3c4d5e6f708192a3b4c/raw
```
```text
+1，学到了。
```

---

#### 题目统计 · `problem_statistics`

```http
GET /p/:pid/stat
```

| 项 | 值 |
|---|---|
| 处理器 | `ProblemStatisticsHandler extends ProblemDetailHandler`（problem.ts:971），唯一方法 `get@976`（**无 POST**） |
| 注册 | `ctx.Route('problem_statistics', '/p/:pid/stat', ProblemStatisticsHandler, PERM.PERM_VIEW_PROBLEM)`（problem.ts:1083） |
| 认证 | 匿名可访问 |
| 域权限 | `PERM_VIEW_PROBLEM` |
| 响应 | JSON 对象 / HTML 模板 `problem_statistics.html`；比赛模式（`tdoc` 存在）抛 `ContestNotEndedError` |

**查询参数（`@param`）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `sort` | `Types.Range(Object.keys(record.STAT_QUERY))` | 否 | 默认 `time`；可选 `time`、`memory`、`length`、`date`（model/record.ts:32-37） |
| `direction` | `Types.Range([-1, 1])` | 否 | 默认 `1`；`-1` 为升序，`1` 为降序（`STAT_QUERY[sort][Math.max(direction, 0)]`） |
| `lang` | `Types.String` | 否 | 按语言过滤 |
| `page` | `Types.PositiveInt` | 否 | 默认 `1`，分页键 `record`（`pagination.record` 默认 100） |

数据来源：`record.getMultiStat(domainId, { pid, ...(lang ? {lang} : {}) }, STAT_QUERY[sort][...])`，即 `record.stat` 集合（只收录 `STATUS_ACCEPTED` 且按 `record.statMode` 去重的记录）。

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `pdoc` `udoc` `psdoc` `title` `solutionCount` `discussionCount` `tdoc` `owner_udoc` `mode` | — | 同 `problem_detail` |
| `rsdocs` | `RecordStatDoc[]` | 统计记录：`_id`(rid)、`domainId`、`pid`、`uid`、`time`、`memory`、`length`、`lang` |
| `page` | number | 当前页 |
| `pcount` | number | 总页数 |
| `rscount` | number | 记录总数 |
| `sort` | string | 排序字段 |
| `direction` | number | 排序方向 |
| `types` | string[] | `Object.keys(record.STAT_QUERY)` = `["time","memory","length","date"]` |
| `udict` | object | `uid → User`（`user.getListForRender`，是否含私有信息取决于 `PERM_VIEW_USER_PRIVATE_INFO`） |
| `udoc` | `User` | 题主（覆盖上面的 `udoc` 字段，值相同） |

**示例**

```http
GET /d/system/p/P1000/stat?sort=time&direction=-1&page=1
Accept: application/json
```
```json
{ "rsdocs": [{ "_id": "6530f0c1a1b2c3d4e5f60718", "pid": 1000, "uid": 2, "time": 12, "memory": 1024, "length": 120, "lang": "cc" }], "page": 1, "pcount": 1, "rscount": 1, "sort": "time", "direction": -1, "types": ["time", "memory", "length", "date"] }
```

---

#### 创建题目 · `problem_create`

```http
GET /problem/create
POST /problem/create
```

| 项 | 值 |
|---|---|
| 处理器 | `ProblemCreateHandler`（problem.ts:997）：`get@998`、`post@1013`（有普通 `post`） |
| 注册 | `ctx.Route('problem_create', '/problem/create', ProblemCreateHandler, PERM.PERM_CREATE_PROBLEM)`（problem.ts:1084） |
| 认证 | 需登录 |
| 域权限 | `PERM_CREATE_PROBLEM` |
| 响应 | GET：JSON 对象 / HTML 模板 `problem_edit.html`；POST：JSON `{ pid, url }` |

**请求体（POST，全部 `@post`）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `title` | `Types.Title` | 是 | 标题 |
| `content` | `Types.Content` | 是 | 题面 |
| `pid` | `Types.ProblemId` + 校验 `/^(?:[a-z0-9]{1,10}-)?[a-z][a-z0-9]*$/i` | 否 | 指定 `pid`；纯数字补前缀 `P`；已存在则抛 `ProblemAlreadyExistError` |
| `hidden` | `Types.Boolean` | 否 | 默认 `false` |
| `difficulty` | `Types.PositiveInt` + 校验 `+i <= 10` | 否 | 默认 `0` |
| `tag` | `Types.Content` + `parseCategory` | 否 | 分类 |

额外行为：扫描题面中 `file://<name>.<ext>`（正则 `/file:\/\/([\w-]+\.[a-zA-Z0-9]+)/g`），把命中的、属于当前用户 `_files` 的文件从 `user/<uid>/<file>` 重命名到 `problem/<domainId>/<docId>/additional_file/<file>` 并注册为附加文件。

**响应字段（GET，`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `page_name` | string | 固定 `'problem_create'` |
| `additional_file` | array | 固定 `[]` |

> 注意：`get` 中先写入了 `statementLangs`，但紧接着用对象字面量整体覆盖了 `this.response.body`（problem.ts:999-1003），因此**响应中不含 `statementLangs`**。

**响应字段（POST）**：`{ "pid": "<pid 或 docId>", "url": "<problem_files 地址>" }`

**示例**

```http
POST /d/system/problem/create
Accept: application/json
Content-Type: application/json

{ "title": "新题", "content": "题面", "pid": "P2000", "tag": "模拟" }
```
```json
{ "pid": "P2000", "url": "/d/system/p/P2000/files" }
```

---

#### 导入题目（Hydro 格式） · `problem_import_hydro`

```http
GET /problem/import/hydro
POST /problem/import/hydro
```

| 项 | 值 |
|---|---|
| 处理器 | `ProblemImportHydroHandler`（import.ts:8）：`get@10`、`post@17`（有普通 `post`） |
| 注册 | `ctx.Route('problem_import_hydro', '/problem/import/hydro', ProblemImportHydroHandler, PERM.PERM_CREATE_PROBLEM)`（import.ts:41） |
| 认证 | 需登录；`keepUser=true` 时额外要求特权 `PRIV_EDIT_SYSTEM` |
| 域权限 | `PERM_CREATE_PROBLEM` |
| 响应 | GET：HTML 模板 `problem_import.html`（无 JSON 结构，`get` 未设置 `response.body`，JSON 模式返回 `{}`）；POST：重定向到 `problem_main` |
| 附加 | `ctx.injectUI('ProblemAdd', 'problem_import_hydro', { icon: 'copy', text: 'Import From Hydro' })`（import.ts:42） |

**请求体（POST）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `keepUser` | `Types.Boolean`（`@param`） | 否 | 保留压缩包内的作者信息；为真时要求 `PRIV_EDIT_SYSTEM` |
| `preferredPrefix` | `Types.String`（`@param`，可选） | 否 | `pid` 前缀，必须匹配 `/^[a-zA-Z]+$/`，否则 `ValidationError('preferredPrefix')` |
| `hidden` | `Types.Boolean`（`@param`） | 否 | 导入的题目是否隐藏 |
| `file` | 文件（multipart `request.files.file`） | 是 | 缺少时抛 `ValidationError('file')` |

行为：调用 `problem.import(domainId, filepath, { preferredPrefix, progress, operator, delSource: true, hidden })`；最多等待 5 秒（`Promise.race` + `sleep(5000)`），超时则重定向到 `problem_main?showImport=1`；导入失败通过站内信（`MessageModel.send`）通知用户。

**响应**：`this.response.redirect = url('problem_main', resolved ? {} : { query: { showImport: 1 } })`；JSON 模式下为 `{ "url": "/d/system/p" }`。

**示例**

```http
POST /d/system/problem/import/hydro
Accept: application/json
Content-Type: multipart/form-data; boundary=...

preferredPrefix=P&hidden=false&file=<zip>
```
```json
{ "url": "/d/system/p" }
```

---

#### 题目分类兼容跳转 · `problem_category_compat`

```http
GET /p/category/:category
```

| 项 | 值 |
|---|---|
| 处理器 | `ProblemCategoryCompatHandler`（compat.ts:3），唯一方法 `get@4`（**无 POST**） |
| 注册 | `ctx.Route('problem_category_compat', '/p/category/:category', ProblemCategoryCompatHandler)`（compat.ts:10） |
| 认证 | 匿名可访问 |
| 域权限 | 无注册级权限（仅隐式 `PERM_VIEW`） |
| 响应 | 302 重定向到 `problem_main?q=category:<category>`；JSON 模式下为 `{ "url": ... }` |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `category` | string | 直接取自 URL，未做类型校验（`async get({ category })`） |

**示例**

```http
GET /d/system/p/category/%E5%8A%A8%E6%80%81%E8%A7%84%E5%88%92
Accept: application/json
```
```json
{ "url": "/d/system/p?q=category%3A%E5%8A%A8%E6%80%81%E8%A7%84%E5%88%92" }
```

---

#### 题目 API（`ProblemApi`）

`packages/hydrooj/src/handler/problem.ts:1037-1061` 定义了 `ProblemApi` 并通过 `api.provide(ProblemApi)` 注册（problem.ts:1085-1087）。命名空间为 `problem`，因此实际调用名为 `problem.problem` 与 `problem.problems`。

| 项 | 值 |
|---|---|
| 注册 | `await ctx.inject(['api'], ({ api }) => { api.provide(ProblemApi); })`（problem.ts:1085-1087） |
| HTTP 入口 | `GET\|POST /api/:op`（`applyApiHandler(childContext, 'api', '/api/:op')`，service/server.ts:104） |
| WS 入口 | `/api/:op/conn`（同上，`Connection(name+'_conn', path+'/conn', ApiConnectionHandler)`） |
| 调用方式 | URL 中 `op = problem.problem`；参数以 query 或 JSON body 传递；也可用 `args` 字段传 JSON，或用 `projection` 裁剪返回字段（`api.ts:126-137`） |
| 类型 | 二者均为 `Query`（GET 与 POST 均可调用） |

**`problem.problem` 输入 Schema**（`api.ts` 的 `Query(Schema.object({...}))`）

| 参数 | Schema | 必填 | 说明 |
|---|---|---|---|
| `id` | `Schema.union([Schema.number().step(1), Schema.string()])` | 是 | 题目 ID（数字 `docId` 或字符串 `pid`） |
| `domainId` | `Schema.string()` | 是 | 域 ID |

行为（problem.ts:1043-1048）：`problem.get(args.domainId, args.id)`；不存在返回 `null`；若 `pdoc.hidden` 则 `ctx.checkPerm(PERM_VIEW_PROBLEM_HIDDEN)`（即隐藏题需要该权限，否则抛 `PermissionError`）。

**返回**：`ProblemDoc`（含解析后的 `config`）或 `null`。

**`problem.problems` 输入 Schema**

| 参数 | Schema | 必填 | 说明 |
|---|---|---|---|
| `ids` | `Schema.array(Schema.number().step(1))` | 是 | `docId` 数组 |
| `domainId` | `Schema.string()` | 是 | 域 ID |

行为（problem.ts:1055-1059）：`problem.getList(args.domainId, args.ids, ctx.user.hasPerm(PERM_VIEW_PROBLEM_HIDDEN) || ctx.user._id, undefined, undefined, true)`，即不可见（隐藏且非本人）的题目会被过滤；返回 `args.ids.map((id) => pdocs[+id]).filter((i) => i)`。

**返回**：`ProblemDoc[]`（按请求顺序，缺失项被剔除）。

**示例**

```http
GET /api/problem.problems?domainId=system&args=%7B%22ids%22%3A%5B1000%2C1001%5D%7D
Accept: application/json
```
```json
[{ "docId": 1000, "pid": "P1000", "title": "A+B Problem" }, { "docId": 1001, "pid": "P1001", "title": "A-B Problem" }]
```

---

#### 语言列表来源与 `applyProjection` 裁剪逻辑

**提交页语言列表（`Types.Language` 的实际情况）**

源码中**不存在 `Types.Language`**。`ProblemSubmitHandler.post` 的语言参数使用的是 `Types.Name`（problem.ts:484）：

```ts
@param('lang', Types.Name)
@param('code', Types.String, true)
@param('pretest', Types.Boolean)
@param('input', Types.ArrayOf(Types.String, true), true)
@param('tid', Types.ObjectId, true)
```

语言白名单来自三处：

1. **题目级**：`pdoc.config.langs`（`config.yaml` 的 `langs` 字段，经 `parseConfig` → `result.langs`，`lib/testdataConfig.ts:36`）。`_prepare` 会把它与域、比赛白名单求交集（problem.ts:340-362）。
2. **系统级**：`setting.langs`（`packages/hydrooj/src/model/setting.ts:379`），内容由系统设置 `hydrooj.langs`（YAML，默认值见 `packages/hydrooj/setting.yaml`）经 `parseLang`（`packages/common/lang.ts:28`）解析而来，类型为 `Record<string, LangConfig>`。
3. **提交页下拉框**：`langRange`（problem.ts:473-476）
   - 若 `pdoc.config` 为对象且含 `langs` → `Object.fromEntries(config.langs.map((i) => [i, setting.langs[i]?.display || i]))`；
   - 否则 → `setting.SETTINGS_BY_KEY.codeLang.range`（用户偏好 `setting_usage.codeLang` 的 `range`，由 `setting.ts:399-401` 在设置加载/变更时填充）。

`LangConfig` 字段（`packages/common/lang.ts:3-27`）：`disabled`、`compile`、`execute`、`code_file`、`highlight`、`monaco`、`time_limit_rate`、`memory_limit_rate`、`address_space_limit`、`process_limit`、`display`、`target`、`key`、`hidden`、`isBinary`、`analysis`、`remote`、`validAs`、`pretest`（已废弃）、`comment`、`compile_time_limit`、`compile_memory_limit`、`version`。

**`applyProjection` 裁剪逻辑**

`contest.applyProjection(tdoc, rdoc, udoc)`（`model/contest.ts:1124-1127`）按比赛规则分派到 `RULES[tdoc.rule].applyProjection`，用于在**比赛未结束**时抹掉评测细节：

| 规则 | 定义位置 | 裁剪行为（`isDone(tdoc)` 为真时**不裁剪**，直接返回原记录） |
|---|---|---|
| `acm`（XCPC） | contest.ts:293-301 | `delete rdoc.time`、`delete rdoc.memory`、`rdoc.testCases = []`、`rdoc.judgeTexts = []`、`delete rdoc.progress`、`delete rdoc.subtasks`、`delete rdoc.score` |
| `oi` | contest.ts:471-481 | `delete rdoc.status`、`rdoc.compilerTexts = []`、`rdoc.judgeTexts = []`、`delete rdoc.memory`、`delete rdoc.time`、`delete rdoc.score`、`rdoc.testCases = []`、`delete rdoc.subtasks` |
| `homework`（Assignment） | 继承自 `oi`（contest.ts:656） | 同 `oi` |
| `ioi` | contest.ts:492-494 | 恒等（`return rdoc`） |
| `strictioi` | 继承自 `ioi`（contest.ts:497） | 恒等 |
| `ledo` | contest.ts:651-653 | 恒等 |

调用点（本 agent 负责的文件内）：`handler/record.ts:117`（列表）、`handler/record.ts:188`（详情，仅当 `!user.own(tdoc) && !user.hasPerm(PERM_EDIT_CONTEST)`）、`handler/record.ts:381` 与 `:474`（WebSocket 推送）。

---

### 评测记录（record）

#### 记录列表 · `record_main`

```http
GET /record
```

| 项 | 值 |
|---|---|
| 处理器 | `RecordListHandler extends ContestDetailBaseHandler`（record.ts:27），唯一方法 `get@38` |
| 注册 | `ctx.Route('record_main', '/record', RecordListHandler)`（record.ts:491，**未声明权限**） |
| 认证 | 匿名可访问；查询他人记录需 `PERM_VIEW_RECORD` |
| 域权限 | 无注册级权限（隐式 `PERM_VIEW`）；`fullStatus`/他人记录 → `PERM_VIEW_RECORD`；`all` → `PERM_VIEW_CONTEST_HIDDEN_SCOREBOARD` + `PERM_VIEW_HOMEWORK_HIDDEN_SCOREBOARD`；`allDomain` → 特权 `PRIV_MANAGE_ALL_DOMAIN`；`stat` → 特权 `PRIV_VIEW_JUDGE_STATISTICS` |
| 响应 | JSON 对象 / HTML 模板 `record_main.html`；**仅支持 GET**（无 `post`/`postXxx`），POST 返回 `MethodNotAllowedError`（405） |

**查询参数（全部 `@param`，query 或 body）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `page` | `Types.PositiveInt` | 否 | 默认 `1` |
| `pid` | `Types.ProblemId` | 否 | 题目过滤；比赛上下文中单字母（`/^[A-Z]$/`）会被解析为 `tdoc.pids[parseInt(pid,36)-10]` |
| `tid` | `Types.ObjectId` | 否 | 比赛/作业 ID；决定是否套用 `applyProjection` 与队伍视图 |
| `uidOrName` | `Types.UidOrName` | 否 | 用户过滤，依次尝试 `getById(+v)`、`getByUname`、`getByEmail`；查不到时 `invalid = true`（返回空列表） |
| `lang` | `Types.String` | 否 | 语言过滤 |
| `status` | `Types.Int` | 否 | 状态码过滤（数值） |
| `fullStatus` | `Types.Boolean` | 否 | 映射到形参 `full`。为真时**强制** `uidOrName = 当前用户`，并只取最近 10 条、不投影字段（含 `code`/`input`） |
| `all` | `Types.Boolean` | 否 | 忽略比赛维度（`delete q.contest`），需两个隐藏成绩表权限 |
| `allDomain` | `Types.Boolean` | 否 | 跨域查询：`delete q.contest`、`q._id = { $gt: Time.getObjectID(now - 10周) }`，`record.getMulti('')`，需 `PRIV_MANAGE_ALL_DOMAIN` |
| `stat` | `Types.Boolean` | 否 | 为真且用户有 `PRIV_VIEW_JUDGE_STATISTICS` 时附加 `statistics` |

其它逻辑：
- **参数名以源码为准**：本接口**没有** `limit` 与 `uid` 参数——每页数量由 `full ? 10 : system.get('pagination.record')` 决定，用户过滤使用 `uidOrName`（同时接受数字 ID、用户名与邮箱）。
- `tid` 存在时校验 `contest.canShowScoreboard`、`canShowRecord`/`canShowSelfRecord`，未报名时追加 `notification`；
- 队伍（`tdoc.allowTeam`）成员查看比赛列表时 `q.uid = { $in: teamMembers }`（`tsdoc.members`）；
- 排序恒为 `_id: -1`；
- 分页大小：`full ? 10 : system.get('pagination.record')`（默认 100）；
- 非 `full` 时投影 `record.PROJECTION_LIST`（`_id, score, time, memory, lang, uid, pid, rejudged, progress, domainId, contest, judger, judgeAt, status, source, files, hackTarget`）；
- `pdict` 按上下文投影：比赛内用 `problem.PROJECTION_CONTEST_LIST`，普通场景用 `problem.PROJECTION_LIST`，无 `PERM_VIEW_PROBLEM` 时用 `problem.default` 占位；
- 非比赛所有者/无 `PERM_EDIT_CONTEST` 时对每条记录套用 `contest.applyProjection`。

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `page` | number | 当前页 |
| `rdocs` | `RecordDoc[]` | 记录列表（已按上下文投影/裁剪） |
| `tdoc` | `Tdoc \| null` | 比赛文档 |
| `pdict` | object | `pid → ProblemDoc` |
| `udict` | object | `uid → User` |
| `all` | boolean | 回显 |
| `allDomain` | boolean | 回显 |
| `filterPid` | string \| number | 回显 `pid` |
| `filterTid` | string | 回显 `tid` |
| `filterUidOrName` | string | 回显 `uidOrName`（`full` 时被替换为当前用户 ID） |
| `filterLang` | string | 回显 `lang` |
| `filterStatus` | number | 回显 `status` |
| `notification` | array | 提示项（如未报名），元素形如 `{ name, args, checker }` |
| `statistics` | object | 仅当 `stat && PRIV_VIEW_JUDGE_STATISTICS`：`{ d5min, d1h, day, week, month, year, total }`（model/record.ts:78-99） |

**示例**

```http
GET /d/system/record?pid=P1000&status=1&page=1
Accept: application/json
```
```json
{
  "page": 1,
  "rdocs": [{ "_id": "6530f0c1a1b2c3d4e5f60718", "pid": 1000, "uid": 2, "lang": "cc", "status": 1, "score": 100, "time": 12, "memory": 1024, "progress": null }],
  "tdoc": null,
  "pdict": { "1000": { "docId": 1000, "pid": "P1000", "title": "A+B Problem" } },
  "udict": { "2": { "_id": 2, "uname": "admin" } },
  "all": false,
  "allDomain": false,
  "filterPid": "P1000",
  "filterStatus": 1,
  "notification": []
}
```

---

#### 记录详情 · `record_detail`

```http
GET /record/:rid
POST /record/:rid
```

| 项 | 值 |
|---|---|
| 处理器 | `RecordDetailHandler extends ContestDetailBaseHandler`（record.ts:140）：`prepare@144`、`get@168`、`post@224`、`postRejudge@231`、`postCancel@242`，另有 `download@150` |
| 注册 | `ctx.Route('record_detail', '/record/:rid', RecordDetailHandler)`（record.ts:492，**未声明权限**） |
| 认证 | 匿名可访问（前提是记录可见） |
| 域权限 | 查看：本人/队友记录，或 `PERM_VIEW_RECORD`；比赛内另有 `canShowRecord`/`canShowSelfRecord` 与 `PERM_VIEW_CONTEST_HIDDEN_SCOREBOARD`；代码可见性见下 |
| 响应 | JSON 对象 / HTML 模板 `record_detail.html`；`download=true` 时返回文件流或纯文本源码 |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `rid` | `Types.ObjectId` | 记录 ID |

**查询参数（`@param`）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `rid` | `Types.ObjectId` | 是 | 与路径一致（`prepare` 与 `get` 均声明） |
| `download` | `Types.Boolean` | 否 | 默认 `false`；为真且可见代码时直接下载源码 |
| `rev` | `Types.ObjectId` | 否 | 历史版本 ID（`record.collHistory`），命中时用历史快照覆盖 `rdoc`（`progress` 置 `null`） |

**代码可见性（`canViewCode`，record.ts:200-204）**

依次满足任一即为真：本人/队友记录；`PRIV_READ_RECORD_CODE`；`PERM_READ_RECORD_CODE`；`PERM_READ_RECORD_CODE_ACCEPT` 且本人该题已 AC；比赛所有者；比赛 `allowViewCode && isDone(tdoc)` 且本人已报名。
不可见时代码被清空：`rdoc.code = ''`、`rdoc.files = {}`、`rdoc.compilerTexts = []`。

**`download()` 行为（record.ts:150-163）**

1. 若 `rdoc.files.code` 或 `rdoc.files.hack` 存在 → 302 到 `storage.signDownloadLink('submission/<id>', filename, true, 'user')`；
2. 否则 `this.response.body = rdoc.code`、`this.response.type = 'text/plain'`、`Content-Disposition: attachment; filename="<langs[lang].code_file 或 foo.<lang>>"`。

**响应字段（GET，`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `udoc` | `User` | 提交者 |
| `rdoc` | `RecordDoc` | 记录；无详情权限时被裁剪为 `pick(rdoc, ['_id','lang','code'])`；代码不可见时 `code`/`files`/`compilerTexts` 被清空；比赛场景下可能已套用 `applyProjection` |
| `pdoc` | `ProblemDoc` | 题目（投影 `problem.PROJECTION_LIST + ['config']`） |
| `tdoc` | `Tdoc` | 比赛文档（若 `rdoc.contest` 存在） |
| `rev` | string \| undefined | 回显的历史版本 ID |
| `allRevs` | object | `{ [historyId]: judgeAt }`，该记录的全部历史版本及评测时间（按 `_id` 倒序） |
| `url` | string | 当 `download=true` 触发重定向时由 base 层附加 |

`RecordDoc` 主要字段（`interface.ts` 的 `RecordDoc` = `RecordPayload` + `_id`）：`_id`、`domainId`、`pid`、`uid`、`lang`、`code`、`score`、`time`、`memory`、`status`、`judgeTexts`、`compilerTexts`、`testCases`、`subtasks`、`judger`、`judgeAt`、`rejudged`、`progress`、`contest`、`files`、`hackTarget`、`input`（自测）、`notify`。

**POST 子操作（`operation`）**

| operation | 说明 | 参数（`@param`） | 权限 |
|---|---|---|---|
| （无 `operation`，直接 POST） | `post()` 仅做权限校验：需 `PERM_REJUDGE`；若 `rdoc.files.hack` 存在抛 `HackRejudgeFailedError`；若 `contest` 前 23 位为 `0`（自测）抛 `PretestRejudgeFailedError` | `rid`: `Types.ObjectId` | `PERM_REJUDGE` |
| `rejudge` | 重测：校验题目配置可解析，`record.submissionPriority(uid, -20)` → `record.reset(domainId, rid, true)` → 广播 `record/change` → `record.judge(..., contest ? { detail:false } : {})`，最后 `back()` | `rid`: `Types.ObjectId` | `PERM_REJUDGE`（由无 operation 的 `post()` 前置校验） |
| `cancel` | 取消评测：写入 `status: STATUS_CANCELED(9)`、`score/time/memory = 0`、单条 `testCases` 占位（`message: 'score canceled'`、`status: 9`）、`subtasks: {}`；同时 `TaskModel.deleteMany({ rid })`；随后广播 `record/change` 并调用 `postJudge(latest)` 回写题目/比赛统计 | `rid`: `Types.ObjectId` | 同上 |

**示例**

```http
GET /d/system/record/6530f0c1a1b2c3d4e5f60718
Accept: application/json
```
```json
{
  "udoc": { "_id": 2, "uname": "admin" },
  "rdoc": { "_id": "6530f0c1a1b2c3d4e5f60718", "pid": 1000, "uid": 2, "lang": "cc", "status": 1, "score": 100, "time": 12, "memory": 1024, "code": "...", "testCases": [{ "id": 1, "status": 1, "time": 12, "memory": 1024, "score": 10, "message": "" }] },
  "pdoc": { "docId": 1000, "pid": "P1000", "title": "A+B Problem" },
  "tdoc": null,
  "allRevs": {}
}
```

---

#### 记录列表推送 · `record_conn`

```http
WS /record-conn
```

| 项 | 值 |
|---|---|
| 处理器 | `RecordMainConnectionHandler extends ConnectionHandler`（record.ts:265）：`prepare@288`、`message@344`、`onRecordChange@353`（`@subscribe('record/change')`）；**无 HTTP 方法，仅 WebSocket** |
| 注册 | `ctx.Connection('record_conn', '/record-conn', RecordMainConnectionHandler)`（record.ts:493） |
| 认证 | 匿名可连接；`uidOrName` 指向他人时需 `PERM_VIEW_RECORD` |
| 域权限 | 同 `record_main`：`all` → `PERM_VIEW_CONTEST_HIDDEN_SCOREBOARD` + `PERM_VIEW_HOMEWORK_HIDDEN_SCOREBOARD`；`allDomain` → `PRIV_MANAGE_ALL_DOMAIN`；`tid` 未结束比赛 → `PERM_VIEW_CONTEST_HIDDEN_SCOREBOARD` |
| 响应 | WebSocket 文本帧（JSON），支持 `shorty` 压缩（连接参数 `shorty`） |

**连接查询参数（`@param`，query 或 body）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `tid` | `Types.ObjectId` | 否 | 比赛 ID；不存在抛 `ContestNotFoundError`；`pretest` 或成绩表可见时记录 `this.tid`，否则抛 `PermissionError`；非比赛所有者且无 `PERM_EDIT_CONTEST` 时启用 `applyProjection` |
| `pid` | `Types.ProblemId` | 否 | 题目过滤；题目不存在抛 `ProblemNotFoundError` |
| `uidOrName` | `Types.UidOrName` | 否 | 用户过滤；找不到用户抛 `UserNotFoundError`；`pretest=true` 时被强制为当前用户 |
| `status` | `Types.Int` | 否 | 状态过滤 |
| `pretest` | `Types.Boolean` | 否 | 默认 `false`；为真时只推送当前用户的自测记录，且推送内容省略 `code`/`input` |
| `all` | `Types.Boolean` | 否 | 忽略比赛维度 |
| `allDomain` | `Types.Boolean` | 否 | 跨域推送 |
| `noTemplate` | `Types.Boolean` | 否 | 默认 `false`；为真时推送原始 `rdoc` 对象而非渲染后的 HTML |

**客户端 → 服务端消息**

| 消息 | 格式 | 说明 |
|---|---|---|
| 拉取记录 | `{ "rids": ["<hex>", ...] }` | 非数组直接忽略；服务端按 `record.PROJECTION_LIST` 投影查询后逐条触发 `onRecordChange`（record.ts:344-350） |
| 心跳 | 文本 `"ping"` / `"pong"` | 由框架处理（`server.ts:700-720`）：服务端 30 秒无消息发 `ping`，80 秒无响应直接终止 |

**服务端 → 客户端推送**

| 字段 | 类型 | 说明 |
|---|---|---|
| `html` | string | 默认模式：渲染 `record_main_tr.html`（参数 `{ rdoc, udoc, pdoc, tdoc, allDomain }`） |
| `rdoc` | `RecordDoc` | `pretest=true` 时为 `omit(rdoc, ['code','input'])`；`noTemplate=true` 时为完整 `rdoc` |

推送触发与过滤（`onRecordChange`，record.ts:353-401）：
- 非 `allDomain` 时要求 `rdoc.domainId` 相同；非 `pretest` 时跳过自测记录（`typeof rdoc.input === 'string'`）；
- 非 `all` 时：无 `tid` 则跳过比赛记录；有 `tid` 则要求 `rdoc.contest ∈ { tid, '000000000000000000000000' }`，并按 `isOwnOrTeammateRecord` + `canShowRecord`/`canShowSelfRecord` 过滤；
- `pid`/`uid`/`teamMembers` 过滤；
- `pdoc` 不可见或无 `PERM_VIEW_PROBLEM` 时置 `null`；
- 应用 `applyProjection` 后入队；
- 推送经 100ms 节流的队列合并（`throttle(this.queueClear, 100, { trailing: true })`），同 `rid` 只保留最后一次。

**示例**

```text
# 客户端连接
wss://hydro.ac/d/system/record-conn?pid=1000&noTemplate=1
# 客户端请求指定记录
{ "rids": ["6530f0c1a1b2c3d4e5f60718"] }
# 服务端推送
{ "rdoc": { "_id": "6530f0c1a1b2c3d4e5f60718", "status": 1, "score": 100 } }
```

---

#### 记录详情推送 · `record_detail_conn`

```http
WS /record-detail-conn
```

| 项 | 值 |
|---|---|
| 处理器 | `RecordDetailConnectionHandler extends ConnectionHandler`（record.ts:406）：`prepare@419`、`sendUpdate@453`、`onRecordChange@467`（`@subscribe('record/change')`）；**无 HTTP 方法，仅 WebSocket** |
| 注册 | `ctx.Connection('record_detail_conn', '/record-detail-conn', RecordDetailConnectionHandler)`（record.ts:494） |
| 认证 | 匿名可连接；记录不存在时 `prepare` 直接返回（不推送） |
| 域权限 | 比赛内：`canShowRecord` / `canShowSelfRecord`（否则 `PermissionError(PERM_VIEW_CONTEST_HIDDEN_SCOREBOARD)`）；题目不可见且未报名 → `PermissionError(PERM_VIEW_PROBLEM_HIDDEN)` |
| 响应 | WebSocket 文本帧（JSON）；评测结束后 30 秒以 close code `4001`、原因 `Ended` 关闭 |

**连接查询参数（`@param`）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `rid` | `Types.ObjectId` | 是 | 订阅的记录 ID |
| `noTemplate` | `Types.Boolean` | 否 | 默认 `false`；为真时推送原始 `rdoc`，否则推送渲染后的 HTML |

**服务端 → 客户端推送**

| 字段 | 类型 | 说明 |
|---|---|---|
| `status` | number | 记录状态码（默认模式） |
| `status_html` | string | `record_detail_status.html` 渲染结果（默认模式） |
| `summary_html` | string | `record_detail_summary.html` 渲染结果（默认模式） |
| `rdoc` | `RecordDoc` | `noTemplate=true` 时推送；代码不可见时 `code` 置空、`compilerTexts` 置空；比赛场景下可能已套用 `applyProjection` |

行为要点：
- 仅推送 `rdoc._id === rid` 的变更；
- 发送经 1 秒节流（`throttle(this.sendUpdate, 1000, { trailing: true })`）；
- 当状态不再是 `WAITING(0)`/`JUDGING(20)`/`COMPILING(21)`/`FETCHED(22)` 时，启动 30 秒定时器后关闭（若期间又有更新则取消定时器）（record.ts:484-485）；
- `prepare` 中先调用一次 `onRecordChange(rdoc)`，因此连接建立即收到当前状态。

**示例**

```text
wss://hydro.ac/d/system/record-detail-conn?rid=6530f0c1a1b2c3d4e5f60718
# 服务端推送
{ "status": 1, "status_html": "<span class=\"record-status--text pass\">Accepted</span>", "summary_html": "<div>...</div>" }
```

---

### 评测机（judge）

#### 评测文件下载 · `judge_files_download`

```http
GET /judge/files
POST /judge/files
```

| 项 | 值 |
|---|---|
| 处理器 | `JudgeFilesDownloadHandler`（judge.ts:205）：`get@209`、`post@216` |
| 注册 | `ctx.Route('judge_files_download', '/judge/files', JudgeFilesDownloadHandler, builtin.PRIV.PRIV_JUDGE)`（judge.ts:357） |
| 认证 | 需特权 `PRIV_JUDGE`（`1 << 9`） |
| 域权限 | 无（处理器声明 `noCheckPermView = true`，跳过隐式 `PERM_VIEW` 校验） |
| 响应 | JSON；处理器声明 `notUsage = true`（不参与用量统计） |

**GET**：无参数，返回 `"ok"`（字符串）。评测机用它做登录态探活（`hydrojudge/src/hosts/hydro.ts:ensureLogin`）。

**POST 请求体（`@post`，仅 body）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `id` | `Types.String` | 否 | 提交文件 ID（形如 `<uid>/<nanoid>`）。非空时直接返回该提交代码的签名链接 |
| `files` | `Types.Set` | 否 | 需要下载的测试数据文件名集合（`id` 为空时使用） |
| `pid` | `Types.UnsignedInt` | 否 | 题目 `docId`（`id` 为空时使用） |

**响应字段**

| 字段 | 类型 | 说明 |
|---|---|---|
| `url` | string | 当 `id` 非空：`storage.signDownloadLink('submission/<id>', 'code', true, 'judge')` |
| `links` | object \| null | 当 `id` 为空：`{ [filename]: signedUrl }`；题目不存在时为 `null` |

链接目标为 `problem/<domainId>/<docId>/testdata/<file>`，使用 `judge` 身份签名（内部直链，不设 disposition）。

**示例**

```http
POST /d/system/judge/files
Accept: application/json
Content-Type: application/json

{ "pid": 1000, "files": ["1.in", "1.out"] }
```
```json
{ "links": { "1.in": "https://.../1.in?sig=...", "1.out": "https://.../1.out?sig=..." } }
```

---

#### 评测文件上传 · `judge_files_upload`

```http
POST /judge/upload
```

| 项 | 值 |
|---|---|
| 处理器 | `JudgeFileUpdateHandler`（judge.ts:260），唯一方法 `post@265`（**无 GET**） |
| 注册 | `ctx.Route('judge_files_upload', '/judge/upload', JudgeFileUpdateHandler, builtin.PRIV.PRIV_JUDGE)`（judge.ts:358） |
| 认证 | 需特权 `PRIV_JUDGE` |
| 域权限 | 无（隐式 `PERM_VIEW` 仍生效，因为未声明 `noCheckPermView`）；处理器声明 `notUsage = true` |
| 响应 | JSON `{ ok: 1 }` |

**请求体（multipart，`@post`）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `rid` | `Types.ObjectId` | 是 | 触发上传的记录 ID（用于定位题目与鉴权） |
| `name` | `Types.Filename` | 是 | 目标测试数据文件名 |
| `file` | 文件（`request.files.file`） | 是 | 缺少时抛 `ValidationError('file')` |

回调 `processJudgeFileCallback(rid, filename, filePath)`（judge.ts:235-258）：
1. 记录与题目/用户必须存在；
2. 用户需为题目所有者（`PERM_EDIT_PROBLEM_SELF`）或持有 `PERM_EDIT_PROBLEM`，否则 `ForbiddenError`；
3. 引用题（`pdoc.reference`）抛 `ProblemIsReferencedError('edit files')`；
4. 文件数量上限 `limit.problem_files_max`、总大小上限 `limit.problem_files_max_size`，超限抛 `FileLimitExceededError('count'|'size')`；
5. 文件名经 `sanitize` 后写入 `problem.addTestdata`。

**示例**

```http
POST /d/system/judge/upload
Content-Type: multipart/form-data; boundary=...

rid=6530f0c1a1b2c3d4e5f60718&name=2.in&file=<binary>
```
```json
{ "ok": 1 }
```

---

#### 评测机连接 · `judge_conn`

```http
WS /judge/conn
```

| 项 | 值 |
|---|---|
| 处理器 | `JudgeConnectionHandler extends ConnectionHandler`（judge.ts:272）：`prepare@279`、`sendLanguageConfig@285`、`newTask@289`、`message@309`、`cleanup@349`；**无 HTTP 方法，仅 WebSocket** |
| 注册 | `ctx.Connection('judge_conn', '/judge/conn', JudgeConnectionHandler, builtin.PRIV.PRIV_JUDGE)`（judge.ts:359） |
| 认证 | 需特权 `PRIV_JUDGE`。评测机在 WebSocket 握手请求头中携带会话凭证（`hydrojudge/src/hosts/hydro.ts` 中该行在本仓库被脱敏为 `Authorization: ******'sid=')[1].split(';')[0]}`，即形如 `sid=<sessionId>`；HTTP 侧的 `authorization` 亦被 `layers/base.ts:32-35` 解析为 bearer token） |
| 域权限 | 无（WebSocket 层不注入域前缀，路径恒为 `/judge/conn`） |
| 状态字段 | `category = '#judge'`；默认消费过滤 `query = { type: { $in: ['judge','generate'] } }`；`concurrency = 1`（judge.ts:273-275） |

**握手与初始化流程**

1. 连接建立 → `prepare()`：打印日志并 `sendLanguageConfig()` → 服务端立即下发 `{ "language": setting.langs }`（完整 `LangConfig` 表）。
2. `@subscribe('system/setting')`（judge.ts:284）使得系统设置变更时再次推送 `{ language: ... }`。
3. 评测机发送配置/心跳（`hydrojudge/src/hosts/hydro.ts:consume`）：
   - 无额外配置时发送 `{"key":"ping"}`；
   - 有配置时发送 `{"key":"config","prio":<n>,"concurrency":<n>,"lang":[...]}`；
   - 随后发送 `{"key":"start"}` 才开始接任务；
   - 若未禁用状态上报，则发送 `{"key":"status","info":{...}}` 并每 20 分钟（`1200000ms`）续报。
4. 服务端收到 `start` 才创建任务消费者；重复 `start` 抛 `BadRequestError('Judge daemon already started')`。

**评测机 → 服务端消息（`message(msg)`，judge.ts:309-347）**

| `key` | 附加字段 | 服务端行为 |
|---|---|---|
| `ping` | — | 不记录日志（心跳，由框架层处理 `ping`/`pong`） |
| `prio` | — | 仅跳过详细日志 |
| `config` | `prio`(number)、`concurrency`(number>0)、`lang`(string[])、`type`(string[]) | 逐项更新 `this.query` 并 `consumer.setQuery(...)`；`concurrency` 同时 `consumer.setConcurrency(...)`。未传 `type` 时保持默认 `{ $in: ['judge','generate'] }` |
| `start` | — | `this.consumer = task.consume(this.query, this.newTask.bind(this), true, this.concurrency)`（`destroyOnError = true`） |
| `status` | `info` | `await updateJudge(msg.info)`（写入 `status` 集合，`type:'judge'`，附 `updateAt`） |
| `next` | `rid` + `JudgeResultBody` 字段 | 找到 `this.tasks[rid]` 后调用 `context.next(msg)`；未知 `rid` 只记警告 |
| `end` | `rid` + `JudgeResultBody` 字段（含可选 `nop`） | `context.end(msg.nop ? undefined : { judger: this.user._id, ...msg })`；`nop` 表示丢弃（仅结束等待） |

日志策略（judge.ts:310-315）：除 `ping`/`prio`/`config`/`start` 外都记录；`status`/`next` 用 debug（省略 `key` 字段），其余用 info（省略 `key`、`subtasks`、`cases`）。

**任务下发（`newTask(t)`，judge.ts:289-308）**

1. `rid = t.rid.toHexString()`；创建 `JudgeResultCallbackContext(this.ctx, t)`；
2. 若同 `rid` 已有在途任务，最多等待 300 秒（每秒轮询）；超时则以 `{ message: 'Wait for previous judge timeout', status: STATUS_SYSTEM_ERROR }` 结束；
3. 记录 `this.tasks[rid] = context`，下发 `{ "task": <Task> }`；
4. 立即调用 `context.next({ status: STATUS_FETCHED })`，把记录状态置为 `FETCHED(22)` 并广播 `record/change`；
5. `await this.tasks[rid]`（等待 `end` 或 `nop` 触发 resolve），然后 `delete this.tasks[rid]`。

`Task` 对象字段（`model/record.ts:106-128`）：记录自身的全部字段（`_id`/`uid`/`pid`/`lang`/`code`/`input`/`files`/`hackTarget`/`contest`…）＋ `priority`、`type`（`judge` / `remotejudge` / `generate`）、`rid`、`domainId`、`config`（解析后的 `ProblemConfigFile`，可被调用方覆盖）、`data`（题目 `FileInfo[]`）、`source`（`<domainId>/<docId>`）、`trusted`（域是否可信）、`meta`（`JudgeMeta`：`problemOwner`、`rejudge`、`hackRejudge`、`type`）。

**断连处理（`cleanup()`，judge.ts:349-353）**

`consumer.destroy()`，并对所有在途任务执行 `context.reset()`：`record.reset(domainId, rid, false)` → 广播 `record/change` → `task.add(this.task)` 重新入队。

**心跳**：由框架统一处理（`framework/framework/server.ts:700-720`）——30 秒无消息发文本 `ping`，80 秒无响应 `cleanup()` + `terminate()`；评测机收到 `ping` 回 `pong`。

**示例**

```text
# 服务端 → 评测机（连接建立时）
{ "language": { "cc": { "display": "C++", "key": "cc", "execute": "/w/foo", ... } } }
# 评测机 → 服务端
{ "key": "config", "concurrency": 4, "lang": ["cc", "py"] }
{ "key": "start" }
# 服务端 → 评测机
{ "task": { "rid": "6530f0c1a1b2c3d4e5f60718", "type": "judge", "domainId": "system", "pid": 1000, "uid": 2, "lang": "cc", "code": "...", "config": { "type": "default", "count": 10 }, "priority": 0 } }
# 评测机 → 服务端
{ "key": "next", "rid": "6530f0c1a1b2c3d4e5f60718", "status": 20, "progress": 0.1 }
{ "key": "next", "rid": "6530f0c1a1b2c3d4e5f60718", "case": { "id": 1, "status": 1, "time": 12, "memory": 1024, "score": 10 } }
{ "key": "end", "rid": "6530f0c1a1b2c3d4e5f60718", "status": 1, "score": 100, "time": 12, "memory": 1024, "subtasks": { "1": { "type": "sum", "score": 100, "status": 1 } } }
```

---

### 服务状态（status）

#### 状态页 · `status`

```http
GET /status
```

| 项 | 值 |
|---|---|
| 处理器 | `StatusHandler`（status.ts:22），唯一方法 `get@23`（**无 POST**） |
| 注册 | `ctx.Route('status', '/status', StatusHandler)`（status.ts:85） |
| 认证 | 匿名可访问 |
| 域权限 | 无注册级权限（隐式 `PERM_VIEW`） |
| 响应 | JSON 对象 / HTML 模板 `status.html` |

数据来源：`status` 集合（`db.collection('status')`），按 `{ type: 1, updateAt: -1 }` 排序，并为每条补 `isOnline`（`updateAt` 距今 < 5 分钟）与 `status`（`Online`/`Offline`）。该集合带 TTL 索引 `{ updateAt: 1, expireAfterSeconds: 24*2600 }`（status.ts:87）。

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `stats` | object[] | 各节点原始状态文档，并附加：`isOnline`(boolean)、`status`(`'Online'`/`'Offline'`)、`battery`(字符串，如 `No battery` 或 `<type> <model> <percent>%[ Charging]`) |
| `languages` | object | `"<display>(<key>)" → <compile 或 execute 命令>`，跳过 `hidden` 语言 |
| `compilers` | object[] | 按编译器消息合并后的结果：`{ key: string[], message: string }`，`key` 为语言 key 列表，`message` 为出现次数最多的编译器版本信息 |

`stats` 元素字段来自 `updateJudge` 写入的 `info`（`packages/hydrooj/src/service/monitor.ts:85-93`），即 `mid`、`type`、`updateAt`、`cpu`、`memory`、`osinfo`、`load`、`CpuTemp`、`battery`、`compilers`、`stackSize` 等（见 `framework/utils/lib/sysinfo.ts` 的 `StatusFull`）。

**示例**

```http
GET /status
Accept: application/json
```
```json
{
  "stats": [{ "mid": "xxxx", "type": "judge", "isOnline": true, "status": "Online", "battery": "No battery", "compilers": { "cc": "g++ 12.2.0" } }],
  "languages": { "C++(cc)": "/usr/bin/g++ -Wall --std=c++14 -o foo foo.cc -lm -I/include" },
  "compilers": [{ "key": ["cc"], "message": "g++ 12.2.0" }]
}
```

---

#### 状态上报 · `status_update`

```http
POST /status/update
```

| 项 | 值 |
|---|---|
| 处理器 | `StatusUpdateHandler`（status.ts:70），唯一方法 `post@71`（**无 GET**） |
| 注册 | `ctx.Route('status_update', '/status/update', StatusUpdateHandler)`（status.ts:86） |
| 认证 | 需特权 `PRIV_JUDGE`（方法体内 `this.checkPriv(PRIV.PRIV_JUDGE)`） |
| 域权限 | 无注册级权限（隐式 `PERM_VIEW`） |
| 响应 | JSON `{ ok: 1 }` |

`post(args)` 无参数装饰器，直接使用原始 `args`（`{ domainId, ...params, ...query, ...body, __start }`），因此**请求体中的任意字段都会被写入状态文档**。

**请求体（POST）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `mid` | string | 是（用作 upsert 条件） | 机器标识 |
| `type` | string | — | **服务端强制覆盖为 `'judge'`**（status.ts:73），请求中传入无效 |
| `updateAt` | Date | — | **服务端强制覆盖为 `new Date()`**（status.ts:74） |
| 其它任意字段 | any | 否 | 原样 `$set` 写入（如 `cpu`、`memory`、`load`、`battery`、`compilers`、`stackSize` 等） |

写入方式：`coll.updateOne({ mid, type: 'judge' }, { $set: args }, { upsert: true })`。

**示例**

```http
POST /d/system/status/update
Accept: application/json
Content-Type: application/json

{ "mid": "xxxx", "stackSize": 8192, "compilers": { "cc": "g++ 12.2.0" } }
```
```json
{ "ok": 1 }
```

---

### 评测状态码

来源：`packages/common/status.ts`（**权威来源**；`packages/hydrooj/src/model/builtin.ts` 只是 re-export，并通过 `global.Hydro.model.builtin.STATUS` 等暴露）。

#### `STATUS` 完整枚举

| 常量 | 数值 | `STATUS_TEXTS`（源码原文） | `STATUS_SHORT_TEXTS` | `STATUS_CODES`（CSS 类） | 中文含义（说明性翻译，非源码文案） |
|---|---|---|---|---|---|
| `STATUS_WAITING` | 0 | `Waiting` | — | `pending` | 等待评测 |
| `STATUS_ACCEPTED` | 1 | `Accepted` | `AC` | `pass` | 通过 |
| `STATUS_WRONG_ANSWER` | 2 | `Wrong Answer` | `WA` | `fail` | 答案错误 |
| `STATUS_TIME_LIMIT_EXCEEDED` | 3 | `Time Exceeded` | `TLE` | `fail` | 超时 |
| `STATUS_MEMORY_LIMIT_EXCEEDED` | 4 | `Memory Exceeded` | `MLE` | `fail` | 超内存 |
| `STATUS_OUTPUT_LIMIT_EXCEEDED` | 5 | `Output Exceeded` | `OLE` | `fail` | 输出超限 |
| `STATUS_RUNTIME_ERROR` | 6 | `Runtime Error` | `RE` | `fail` | 运行时错误 |
| `STATUS_COMPILE_ERROR` | 7 | `Compile Error` | `CE` | `fail` | 编译错误 |
| `STATUS_SYSTEM_ERROR` | 8 | `System Error` | `SE` | `fail` | 系统错误 |
| `STATUS_CANCELED` | 9 | `Cancelled` | `IGN` | `ignored` | 已取消 |
| `STATUS_ETC` | 10 | `Unknown Error` | — | `fail` | 其它错误 |
| `STATUS_HACKED` | 11 | `Hacked` | `HK` | `fail` | 被 Hack |
| `STATUS_JUDGING` | 20 | `Running` | — | `progress` | 评测中 |
| `STATUS_COMPILING` | 21 | `Compiling` | — | `progress` | 编译中 |
| `STATUS_FETCHED` | 22 | `Fetched` | — | `progress` | 已被评测机取走 |
| `STATUS_IGNORED` | 30 | `Ignored` | `IGN` | `ignored` | 忽略 |
| `STATUS_FORMAT_ERROR` | 31 | `Format Error` | `FE` | `ignored` | 格式错误 |
| `STATUS_HACK_SUCCESSFUL` | 32 | `Hack Successful` | — | `pass` | Hack 成功 |
| `STATUS_HACK_UNSUCCESSFUL` | 33 | `Hack Unsuccessful` | — | `fail` | Hack 失败 |

#### 相关派生常量

| 常量 | 内容 | 说明 |
|---|---|---|
| `STATUS_TEXTS` | `Record<STATUS, string>` | 上表第 3 列的英文文案，**无中文版本**；模板 `record_main.html:58`、`components/record.html`、`record_detail_status.html` 直接输出该值，不经过 i18n（`ui-default/locales/zh.yaml` 仅对 `Accepted`、`Compile Error` 等个别键有中文翻译，但状态渲染未走 `_()`） |
| `STATUS_SHORT_TEXTS` | `Partial<Record<STATUS, string>>` | 仅含 AC/WA/TLE/MLE/OLE/RE/CE/SE/IGN/HK/FE；用于题目统计键 `stats.${STATUS_SHORT_TEXTS[status]}`（judge.ts:138）与比赛表头悬浮提示（contest.ts:565） |
| `STATUS_CODES` | `Record<STATUS, string>` | CSS 类名：`pending`/`pass`/`fail`/`progress`/`ignored` |
| `NORMAL_STATUS` | `STATUS[]` | `[1, 2, 3, 4, 5, 6, 7]`，即 `AC, WA, TLE, MLE, OLE, RE, CE`，共 7 项；用于统计「正式评测结果」 |
| `getScoreColor(score)` | `string` | 按 `Math.floor(score / 10)` 取 11 档颜色：`['#ff4f4f','#ff694f','#f8603a','#fc8354','#fa9231','#f7bb3b','#ecdb44','#e2ec52','#b0d628','#93b127','#25ad40']`；`score` 为 `null`/`undefined`/非有限数时返回 `#000000` |
| `USER_GENDER_MALE` | 0 | 性别常量，文案见 `USER_GENDER_RANGE[0] = 'Boy ♂'`，图标见 `USER_GENDER_ICONS[0] = '♂'` |
| `USER_GENDER_FEMALE` | 1 | 文案 `'Girl ♀'`，图标 `'♀'` |
| `USER_GENDER_OTHER` | 2 | 文案 `'Other'`，图标 `'?'` |
| `USER_GENDERS` | `number[]` | `[0, 1, 2]` |

评测机侧还有两个内部状态：`STATUS_JUDGING(20)`/`STATUS_COMPILING(21)`/`STATUS_FETCHED(22)` 属「进行中」，前端据此决定是否继续订阅（record.ts:478-480）。

---

### 评测流程（submit → 入队 → 下发 → 回调）

以下链路全部来自源码，参与文件：`handler/problem.ts`、`model/record.ts`、`model/task.ts`、`handler/judge.ts`、`handler/record.ts`、`service/monitor.ts`。

**① 提交（`ProblemSubmitHandler.post`，problem.ts:489-548）**

1. 校验语言白名单与自测条件；
2. 限流：`limitRate('add_record', 60, limit.submission_user, '{{user}}')` 与 `limitRate('add_record', 60, limit.submission / limit.pretest)`；
3. 处理源码（内联 `code` 或上传 `file`；二进制/超长文件落到 `submission/<uid>/<nanoid>`）；
4. `record.add(domainId, docId, uid, lang, code, true, pretest ? { input, type:'pretest' } : { contest: tid, files, type:'judge' })`；
5. 非自测时并行 `problem.inc(...,'nSubmit',1)`、`domain.incUserInDomain(...,'nSubmit')`、比赛时 `contest.updateStatus(...)`；
6. 响应 `{ rid }` 并重定向到 `record_detail`。

**② 记录入库与入队（`RecordModel.add` → `RecordModel.judge`，model/record.ts:129-213）**

- `add`：构造 `RecordDoc`（`status: STATUS_WAITING(0)`、`score/time/memory = 0`、`judgeTexts/compilerTexts/testCases = []`、`judger/judgeAt = null`、`rejudged = false`），`insertOne` 后立即 `bus.broadcast('record/change', data)`（记录列表/详情 WebSocket 会先收到一次 WAITING 状态）。
  - `type: 'pretest'` → `data.contest = RECORD_PRETEST('000000000000000000000000')`，并写入 `data.input`；
  - `type: 'generate'` → `data.contest = RECORD_GENERATE('000000000000000000000001')`；
  - `type: 'rejudge'` → 实际转成 `type: 'judge'` 且 `data.rejudged = true`。
- `addTask = true` 时计算优先级 `submissionPriority(uid, pretest ? -20 : (isContest ? 50 : 0))`（`model/record.ts:44-54`：基于近 30 分钟该用户提交数与耗时动态下调，下限 `base - 10000`），再调用 `RecordModel.judge`。
- `judge`：读取题目（引用题会切换到源题目并更新 `source`），`meta.problemOwner = pdoc.owner`；`task.deleteMany({ rid })` 清理旧任务；然后 `task.addMany(...)` 写入 `task` 集合，每个任务包含记录字段 + `priority` + `type` + `rid` + `domainId` + `config`（题目配置，可被调用参数覆盖）+ `data`（题目文件列表）+ `source` + `trusted`（`ddoc.isTrusted`）+ `meta`。
  - `type` 判定：`pdoc.config.type === 'remote_judge'` 且非自测 → `'remotejudge'`；`meta.type === 'generate'` → `'generate'`；否则 `'judge'`。

**③ 任务消费（`model/task.ts:36-96`）**

评测机连接后（`judge_conn` 的 `start`）创建 `Consumer`，其 `consume()` 循环调用 `coll.findOneAndDelete({ ...filter, type: { $in: ['judge','generate'] } }, { sort: { priority: -1 } })` 按优先级取任务，并把并发控制在 `concurrency` 之内；`getFirst` 在 `process.env.CI` 下直接返回 `null`。

**④ 任务下发（`JudgeConnectionHandler.newTask`，judge.ts:289-308）**

`{ task }` 通过 WebSocket 下发给评测机；同时把记录置为 `STATUS_FETCHED(22)`；随后 `await` 该任务的 `JudgeResultCallbackContext`。

**⑤ 结果回调（`JudgeResultCallbackContext`，judge.ts:76-201）**

| 方法 | 触发 | 行为 |
|---|---|---|
| `next(body)` | 收到 `key: 'next'` | 串行化（`operationPromise` 链）后调用 `_next` |
| `_next(body)` | — | `processPayload(body)` 生成 `$set/$push/$unset/$inc`；`meta.rejudge === 'controlled'` 时写入 `record.collHistory`（`relatedId`），否则 `record.update(...)` 并用 `ctx.broadcast('record/change', rdoc, $set, $push, body)` 推送增量 |
| `end(body)` | 收到 `key: 'end'` | `_end`：追加 `$set.judgeAt = new Date()`、`$set.judger = body.judger ?? 1`、`$unset.progress`；写库后 `bus.broadcast('record/change', rdoc, null, null, body)`（全量刷新）并调用 `postJudge(rdoc, this)`；`end()` 无参数（`nop`）时仅 resolve |
| `reset()` | 评测机断连 | `record.reset(domainId, rid, false)` + 广播 + `task.add(this.task)` 重新入队 |
| `then(...)` | `await context` | 等待 `end` 触发的 `finishPromise` |

`processPayload` 字段映射（judge.ts:44-69）：

| 请求字段 | 写入 |
|---|---|
| `cases[]` | `$push.testCases = { $each: cases }`（每项经 `parseCaseResult` 补 `id/subtaskId/score/message` 默认值） |
| `case` | `$push.testCases = case` |
| `message` | `$push.judgeTexts = message` |
| `compilerText` | `$push.compilerTexts = compilerText` |
| `status` | `$set.status` |
| `score`（有限数） | `$set.score = Math.floor(score * 100) / 100` |
| `time`（有限数） | `$set.time`（毫秒） |
| `memory`（有限数） | `$set.memory`（KiB） |
| `progress` | `$set.progress` |
| `addProgress` | `$inc.progress` |
| `subtasks` | `$set.subtasks` |

**⑥ 统计回写（`JudgeResultCallbackContext.postJudge`，judge.ts:121-142）**

1. `rdoc.contest` 以 23 个 `0` 开头（自测）→ 直接返回，不统计；
2. `problem.updateStatus(domainId, pid, uid, rid, status, score)` → `updated`（仅当 AC，或原状态非 AC / `rid` 匹配时才真正更新）；
3. 有 `contest` → `contest.updateStatus(domainId, contest, teamVid ?? uid, rid, pid, rdoc)`；否则若 AC 且 `updated` → `domain.incUserInDomain(domainId, uid, 'nAccept', 1)`；
4. `isNormalSubmission = status ∉ { ETC, HACK_SUCCESSFUL, HACK_UNSUCCESSFUL, FORMAT_ERROR, SYSTEM_ERROR, CANCELED }`；
5. AC 且 `updated` 时 `problem.inc(..., 'nAccept', 1)`，否则 `problem.get(...)`；
6. `isNormalSubmission` 时并行 `problem.inc(..., 'stats.<STATUS_SHORT_TEXTS[status]>', 1)` 与 `problem.inc(..., 'stats.s<floor(score)>', 1)`；
7. `await app.parallel('record/judge', rdoc, updated, pdoc, context)`。

**⑦ `record/judge` 事件的两个订阅者**

| 订阅者 | 行为 |
|---|---|
| `model/record.ts:292-324` | 跳过自测/生成；`rdoc.notify` 时给用户发站内信（`Judge Result\n{0}: {1}`，参数为题目标题与 `STATUS_TEXTS[status]`）；AC 且 `updated` 时按 `record.statMode`（默认 `unique`，先删同 `uid+pid+domainId` 的其它统计）写入 `record.stat`（`time`/`memory`/`length`/`lang`），供 `problem_statistics` 使用 |
| `handler/judge.ts:360-393` | 若 `rdoc.status === STATUS_HACK_SUCCESSFUL` 且无 `contest`：读取该记录 `files.hack` 的输入，追加到 `config.subtasks` 最后一组的 `cases`（文件名 `hack-<rid>-<n>.in`），写回测试数据与 `config.yaml`，然后对该题所有 AC 记录触发一次带 `{ hackRejudge: input }` 的重测（`record.submissionPriority(uid, -5000 - n*5 - 50)`）。失败时通过 `context.next({ message: {...} })` 回报 |

**⑧ 实时推送**

`bus.broadcast('record/change', ...)` → `record_conn`（列表，100ms 节流合并）与 `record_detail_conn`（详情，1s 节流）推送到前端；详情连接在记录离开进行中状态后 30 秒自动以 `4001 Ended` 关闭。

**⑨ 其它入队入口**

| 入口 | 说明 |
|---|---|
| `problem_detail` 的 `postRejudge` | 重测整题：排除 `generate`/`pretest`/`CANCELED`/带 `files.hack` 的记录，`priority = submissionPriority(uid, -10000 - n*5 - 50)`，比赛记录 `{ detail:false }` |
| `record_detail` 的 `postRejudge` | 重测单条：`priority = submissionPriority(uid, -20)`，`record.reset(..., true)` |
| `problem_files` 的 `postGenerateTestdata` | 生成测试数据：`record.add(..., lang='_', code='<gen>\n<std>', { type:'generate' })`，任务 `type = 'generate'` |
| `problem_hack` 的 `post` | Hack：`record.add(..., type:'hack', hackTarget, files.hack)` |
| `judge_conn` 的 `cleanup` | 断连重排：`context.reset()` → `record.reset` + `task.add` |

---

### 题目配置（`config.yaml`）字段

配置文本由 `problem_config` 页面维护、`parseConfig`（`lib/testdataConfig.ts`）解析，类型定义在 `packages/common/types.ts`（`ProblemConfigFile`）与 `packages/hydrooj/src/interface.ts:147-160`（`ProblemConfig`，即 `pdoc.config` 解析后的形态）。

#### `ProblemConfigFile`（`config.yaml` 原始字段，`packages/common/types.ts:41-70`）

| 字段 | 类型 | 说明 |
|---|---|---|
| `type` | `ProblemType` | `default` / `submit_answer` / `interactive` / `communication` / `objective` / `remote_judge` |
| `subType` | string | 子类型（如远程评测平台名）；`type: default` 且配置了 `filename` 时会被 `filename` 覆盖 |
| `target` | string | 编译目标文件名 |
| `score` | number | 总分 |
| `time` | string | 全局时间限制（如 `1s`、`1000ms`） |
| `memory` | string | 全局内存限制（如 `256m`） |
| `filename` | string | 默认类型下的文件名（同时充当 `subType`） |
| `checker_type` | string | 比较器类型，默认 `default`；`{validator, checker}` 齐备且类型 ∉ `{default, strict}` 时题目 `hackable = true` |
| `num_processes` | number | 进程数上限 |
| `user_extra_files` | string[] | 下发给选手的额外文件 |
| `judge_extra_files` | string[] | 下发给评测机的额外文件 |
| `detail` | `'full' \| 'case' \| 'none' \| boolean` | 评测详情粒度（比赛记录会强制 `detail: false`） |
| `answers` | `Record<string, [string \| string[], number]>` | 客观题答案（`objective` 类型） |
| `redirect` | string | 重定向到 `<domainId>/<pid>`（`parseConfig` 中 `split('/', 2)`） |
| `cases` | `TestCaseConfig[]` | 单组测试点；存在时会被包成 `subtasks: [{ cases, type:'sum' }]` |
| `subtasks` | `SubtaskConfig[]` | 子任务（`time`/`memory`/`score`/`if`/`id`/`type`/`cases`） |
| `langs` | string[] | 允许的语言白名单（会与域/比赛白名单求交集） |
| `multi_pass` | number | 多遍评测次数 |
| `checker` | `CompilableSource` | 比较器源文件；不含 `.` 时按内置 `@hydrooj/hydrojudge/vendor/testlib/checkers/<name>.cpp` 解析 |
| `interactor` | `CompilableSource` | 交互器 |
| `manager` | `CompilableSource` | 通信题 Manager |
| `validator` | `CompilableSource` | 校验器（Hack 必需） |
| `time_limit_rate` | `Record<string, number>` | 按语言的时间倍率 |
| `memory_limit_rate` | `Record<string, number>` | 按语言的内存倍率 |

`TestCaseConfig`：`{ input: string; output: string; time?: string; memory?: string; score?: number }`。
`SubtaskConfig`：`{ time?; memory?; score?; if?: number[]; id?; type?: 'min'|'max'|'sum'; cases?: TestCaseConfig[] }`。
`CompilableSource`：`string | { file: string; lang: string }`。

**关于 `stackLimit`**：`config.yaml` 中**没有**该字段。栈限制由评测机在沙箱层决定（`packages/hydrojudge/src/sandbox.ts:114`：`stackLimit: getConfig('strict_memory') ? memory * 1024 * 1024 : 0`），即由评测机本地配置 `strict_memory` 控制。

#### `ProblemConfig`（`pdoc.config` 解析后的对象，`interface.ts:147-160`）

| 字段 | 类型 | 说明 |
|---|---|---|
| `count` | number | 测试点总数：`Object.keys(answers).length` 或各子任务 `cases` 之和；为 0 时再由测试数据文件名推断（`readSubtasksFromFiles`） |
| `memoryMin` / `memoryMax` | number | 最小/最大内存（MB）；无子任务时取全局 `memory`；非法时回退 256 |
| `timeMin` / `timeMax` | number | 最小/最大时间（ms）；无子任务时取全局 `time`；非法时回退 1000 |
| `type` | string | 同 `config.type`，默认 `default` |
| `subType` | string? | 同 `config.subType`（或 `filename`） |
| `target` | string? | 编译目标 |
| `langs` | string[]? | 语言白名单（`_prepare` 会进一步求交集） |
| `hackable` | boolean | `validator && checker && checker_type ∉ {default, strict}` |
| `redirect` | `[string, string]?` | `config.redirect.split('/', 2)` |

`parseConfig` 还保证：`memoryMax >= memoryMin`、`timeMax >= timeMin`（否则回退 256MB / 1000ms）。

---

### 语言与代码模板

#### 语言配置来源与结构

| 项 | 位置 | 说明 |
|---|---|---|
| 系统设置项 | `packages/hydrooj/src/model/setting.ts:368` | `Setting('setting_basic', 'hydrooj.langs', settingFile.langs.default, 'yaml', ...)`，默认值在 `packages/hydrooj/setting.yaml` 的 `langs.default` |
| 运行时表 | `packages/hydrooj/src/model/setting.ts:379` | `export const langs: Record<string, LangConfig> = {}`，由 `parseLang(system.get('hydrooj.langs'))` 填充；`system/setting` 事件触发时增量覆盖 |
| 解析器 | `packages/common/lang.ts:28-53` | `parseLang(config)`：忽略 `_` 开头的键；`foo.bar` 继承 `foo` 的全部字段；补默认值 |
| 类型 | `packages/common/lang.ts:3-27` | `LangConfig` |
| 默认语言集 | `packages/hydrooj/setting.yaml` | `bash`、`c`、`cc`（及 `cc.cc98/cc98o2/cc11/cc11o2/cc14/cc14o2/cc17/cc17o2/cc20/cc20o2`）、`pas`、`java`、`kt`/`kt.jvm`、`py`/`py.py2`(disabled)/`py.py3`/`py.pypy3`、`php`、`rs`、`hs`、`js`、`go`、`rb`、`cs`、`r` 等 |

`parseLang` 的默认值填充规则：

| 字段 | 默认值 |
|---|---|
| `highlight` | 语言 key |
| `monaco` | `highlight`（否则 key） |
| `time_limit_rate` | `1` |
| `memory_limit_rate` | `1` |
| `code_file` | `foo.<key>` |
| `execute` | `/w/foo` |
| `key` | 语言 key |
| `remote` | `!!remote`（布尔化） |
| `hidden` | `false` |
| `disabled` | `false` |
| `isBinary` | `false` |
| `validAs` | `{}` |

#### 语言倍率

`LangConfig.time_limit_rate` / `memory_limit_rate` 是**语言级**倍率（默认 1），在评测机侧生效；例如 `java` 默认 `time_limit_rate: 2`，`kt` 默认 `time_limit_rate: 2`（`packages/hydrooj/setting.yaml`）。题目级倍率则通过 `config.yaml` 的 `time_limit_rate` / `memory_limit_rate`（`Record<string, number>`）按语言覆盖；`normalizeSubtasks`（`packages/common/subtask.ts:144`）在归一化子任务时接收 `timeRate`/`memoryRate` 参数完成乘法。

#### 代码模板（`codeTemplate`）

| 项 | 说明 |
|---|---|
| 定义 | 用户偏好设置 `Setting('setting_usage', 'codeTemplate', '', 'textarea', 'Default Code Template', 'If left blank, the built-in template of the corresponding language will be used.')`（setting.ts:253） |
| 默认值 | 空字符串（`''`），即使用各语言内置模板 |
| 使用 | 模板层把 `handler.user.codeTemplate` 与 `handler.user.codeLang` 注入 `UiContext`（`ui-default/templates/problem_detail.html:3-10`）；前端在初始化编辑器时读取：`code: draft.code ?? UiContext.codeTemplate`（`ui-default/pages/problem_detail.page.tsx:156`）、`code: UiContext.codeTemplate`（`ui-default/components/scratchpad/reducers/editor.ts:3`） |
| 默认语言 | 偏好设置 `setting_usage.codeLang`（`LangSettingNode`，setting.ts:226-236）；服务端默认语言为 `setting_server.preference.codeLang`（setting.ts:237-247）；其 `range` 由 `langs` 表在设置加载时动态填充（setting.ts:397-403） |

#### 语言与提交页的交互要点

1. `problem_submit` 的 `langRange` 仅列出题目允许（或系统可用）的语言（见「语言列表来源」小节）。
2. 提交时服务端再次校验：`config.langs` 必须包含 `lang`、`setting.langs[lang]` 必须存在且 `disabled !== true`。
3. `objective` / `submit_answer` 类型强制 `lang = '_'`（`langs` 也固定为 `['_']`）。
4. 自测（`pretest`）只允许 `default` 与 `remote_judge` 类型；若语言配置了 `pretest` 字段（已废弃），提交语言会被替换为该值。
5. 二进制语言（`isBinary`）或 `.zip` 上传不会读入内存，而是以文件形式存储并在记录中保留 `files.code` 引用。

---

## 4. 比赛、作业与排行榜

### 比赛（Contest）路由

> 源码：`packages/hydrooj/src/handler/contest.ts`（共 1209 行，路由注册在 `apply()` 的 1077–1207 行）。
> 系统域路径直接为 `/contest/...`，其它域为 `/d/:domainId/contest/...`。
> 所有 Handler 在 `Accept: application/json` 下返回 `this.response.body` 原始对象；否则渲染模板。

#### 路由总览

| route_name | 方法 | 路径 | 处理器 | 路由级权限（`ctx.Route` 末位参数） |
|---|---|---|---|---|
| `contest_create` | GET / POST | `/contest/create` | `ContestEditHandler`（contest.ts:410） | — |
| `contest_main` | GET | `/contest` | `ContestListHandler`（contest.ts:34） | `PERM_VIEW_CONTEST` |
| `contest_team` | GET / POST | `/contest/team` | `ContestTeamHandler`（contest.ts:983） | —（`prepare` 内 `checkPriv(PRIV_USER_PROFILE)`） |
| `contest_detail` | GET / POST | `/contest/:tid` | `ContestDetailHandler`（contest.ts:159） | `PERM_VIEW_CONTEST` |
| `contest_problemlist` | GET / POST | `/contest/:tid/problems` | `ContestProblemListHandler`（contest.ts:332） | `PERM_VIEW_CONTEST` |
| `contest_edit` | GET / POST | `/contest/:tid/edit` | `ContestEditHandler`（contest.ts:410） | `PERM_VIEW_CONTEST` |
| `contest_print` | GET / POST | `/contest/:tid/print` | `ContestPrintHandler`（contest.ts:248） | `PERM_VIEW_CONTEST` |
| `contest_print_alt` | GET / POST | `/contest/:tid/api/printing/team` | `ContestPrintHandler`（contest.ts:248） | `PERM_VIEW_CONTEST` |
| `contest_manage` | GET / POST | `/contest/:tid/management` | `ContestManagementHandler`（contest.ts:602） | —（基类 `prepare` 校验） |
| `contest_clarification` | GET / POST | `/contest/:tid/clarification` | `ContestClarificationHandler`（contest.ts:680） | —（基类 `prepare` 校验） |
| `contest_code` | GET | `/contest/:tid/code` | `ContestCodeHandler`（contest.ts:551） | `PERM_VIEW_CONTEST` |
| `contest_file_download` | GET | `/contest/:tid/file/:type/:filename` | `ContestFileDownloadHandler`（contest.ts:731） | `PERM_VIEW_CONTEST` |
| `contest_user` | GET / POST | `/contest/:tid/user` | `ContestUserHandler`（contest.ts:758） | `PERM_VIEW_CONTEST` |
| `contest_balloon` | GET / POST | `/contest/:tid/balloon` | `ContestBalloonHandler`（contest.ts:825） | `PERM_VIEW_CONTEST` |
| `contest_scoreboard` | GET / POST | `/contest/:tid/scoreboard` | `ContestScoreboardHandler`（contest.ts:890） | `PERM_VIEW_CONTEST_SCOREBOARD` |
| `contest_scoreboard_view` | GET / POST | `/contest/:tid/scoreboard/:view` | `ContestScoreboardHandler`（contest.ts:890） | `PERM_VIEW_CONTEST_SCOREBOARD` |

**HTTP 方法与 `operation` 派发（逐类核对结果，依据 `method-map.md` 的源码提取）**

- 框架派发规则（`framework/framework/server.ts:546-563`）：`POST` 时若 body 含 `operation`，先把 `_${operation}` 按下划线转驼峰（`early_end` → `EarlyEnd`）后调用 `post${Operation}`；若 `operation` 不存在，则要求类上有普通 `post` 方法，否则 `MethodNotAllowedError`（405）。因此 **operation 名称必须使用下划线写法**。
- 本文件中**只有 `ContestPrintHandler` 实现了普通 `post`**（contest.ts:262，用于 DOMjudge 兼容）；其余所有 Handler 都只有具名 `postXxx`，不带 `operation` 的 POST 一律 405。
- 仅实现 `get` 的 Handler（POST 完全不可用，也无 `operation`）：`ContestListHandler.get@39`、`ContestCodeHandler.get@554`、`ContestFileDownloadHandler.get@736`、`HomeworkMainHandler.get@37`。

| Handler | 具名 POST 方法（→ `operation` 名） |
|---|---|
| `ContestDetailHandler` | `postAttend@191`（`attend`）、`postSubscribe@231`（`subscribe`）、`postEarlyEnd@238`（`early_end`） |
| `ContestEditHandler` | `postUpdate@468`（`update`）、`postDelete@523`（`delete`） |
| `ContestTeamHandler` | `postCreate@1005`（`create`）、`postRename@1017`（`rename`）、`postInvite@1027`（`invite`）、`postAccept@1044`（`accept`）、`postReject@1054`（`reject`）、`postLeave@1063`（`leave`） |
| `ContestPrintHandler` | 普通 `post@262`（无 `operation`）+ `postPrint@283`（`print`）、`postGetPrintTask@299`（`get_print_task`）、`postAllocatePrintTask@309`（`allocate_print_task`）、`postUpdatePrintTask@321`（`update_print_task`） |
| `ContestProblemListHandler` | `postClarification@394`（`clarification`） |
| `ContestManagementHandler` | `postUploadFile@630`（`upload_file`）、`postDeleteFiles@657`（`delete_files`）、`postSetScore@670`（`set_score`） |
| `ContestClarificationHandler` | `postClarification@703`（`clarification`） |
| `ContestUserHandler` | `postAddUser@786`（`add_user`）、`postRank@793`（`rank`）、`postResume@802`（`resume`）、`postRemoveUser@816`（`remove_user`） |
| `ContestBalloonHandler` | `postSetColor@849`（`set_color`）、`postDone@863`（`done`） |
| `ContestScoreboardHandler` | `postUnlock@931`（`unlock`） |
| `HomeworkDetailHandler` | `postAttend@139`（`attend`） |
| `HomeworkEditHandler` | `postUpdate@193`（`update`）、`postDelete@242`（`delete`） |
| `HomeworkFilesHandler` | `postUploadFile@280`（`upload_file`）、`postDeleteFiles@300`（`delete_files`） |

**关于 `ContestScoreboardHandler` 的注册位置（已查清）**：它**不是**由 `contest_detail` 触发的。`contest_detail` 的 `after` 钩子（contest.ts:117-156）只是把 `contest_scoreboard` 放进 `overrideNav` 侧边栏导航，真正的路由注册在 `apply()` 中：

```ts
// contest.ts:1108-1110
await ctx.inject(['scoreboard'], ({ Route, scoreboard }) => {
    Route('contest_scoreboard', '/contest/:tid/scoreboard', ContestScoreboardHandler, PERM.PERM_VIEW_CONTEST_SCOREBOARD);
    Route('contest_scoreboard_view', '/contest/:tid/scoreboard/:view', ContestScoreboardHandler, PERM.PERM_VIEW_CONTEST_SCOREBOARD);
```

`homework.ts:317-319` 用同一个 Handler 注册了 `homework_scoreboard` / `homework_scoreboard_view`。详见后文《比赛排行榜（Scoreboard）详解》。

**通用基类**：
- `ContestDetailBaseHandler`（contest.ts:79-157）为 `contest_detail` / `contest_problemlist` / `contest_print` / `contest_file_download` / `contest_scoreboard` 的公共父类，其 `__prepare`（contest.ts:85）解析 `tid`、加载 `tdoc`/`tsdoc`、处理 `assign` 组限制；`after`（contest.ts:117）生成侧边栏。
- `ContestManagementBaseHandler`（contest.ts:545-549）为 `contest_manage` / `contest_clarification` / `contest_user` / `contest_balloon` 的公共父类：

```ts
export class ContestManagementBaseHandler extends ContestDetailBaseHandler {
    async prepare() {
        if (!this.user.own(this.tdoc)) this.checkPerm(PERM.PERM_EDIT_CONTEST);
    }
}
```

即：比赛管理员页面**没有**路由级权限，改为「比赛所有者，或拥有 `PERM_EDIT_CONTEST`」。

---

#### 比赛列表 · `contest_main`

```http
GET /contest
GET /d/:domainId/contest
```

| 项 | 值 |
|---|---|
| 处理器 | `ContestListHandler`（contest.ts:34） |
| 方法（源码） | 仅 `get@39`——**没有普通 `post`**；POST 不带 `operation` 时框架抛 `MethodNotAllowedError`（405） |
| 认证 | 匿名可访问（路由级要求域权限 `PERM_VIEW_CONTEST`） |
| 域权限 | `PERM_VIEW_CONTEST` |
| 响应 | JSON 对象 / HTML 模板 `contest_main.html` |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 无 |

**查询参数**（`ContestListHandler.get`，contest.ts:35-39；装饰器 source 为 `all`，因此也可放在 POST body）

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `rule` | `Types.Range(contest.RULES)` | 否 | 赛制过滤，取值必须是 `RULES` 的键（`acm`/`oi`/`ioi`/`strictioi`/`ledo`/`homework`）；传入隐藏赛制（`homework`）会抛 `BadRequestError` |
| `group` | `Types.Name`（1–255 字符） | 否 | 按用户组过滤；必须在当前用户可见的组名列表中，否则 `NotAssignedError` |
| `page` | `Types.PositiveInt` | 否 | 页码，默认 1（`this.paginate(cursor, page, 'contest')`） |
| `q` | `Types.String` | 否 | 标题搜索；长度 < 2 时使用 `\A` 前缀正则（`escapeRegExp` 转义），≥ 2 时为全局模糊正则 |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| （同上四个） | — | 否 | 装饰器 source 为 `all`，query 与 body 都会读取 |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `page` | number | 当前页码 |
| `tpcount` | number | 总页数（`this.paginate` 返回值） |
| `qs` | string | 回显用的查询串片段，如 `rule=acm&group=g1&q=xx` |
| `rule` | string | 回显的赛制 |
| `tdocs` | `Tdoc[]` | 比赛文档数组，排序 `{ endAt: -1, beginAt: -1, _id: -1 }` |
| `tsdict` | `Record<string, ContestStatusDoc \| null>` | 以 `tid` 十六进制字符串为键的当前用户参赛状态（`contest.getListStatus`，contest.ts:951） |
| `groups` | `{ name: string, uids: number[] }[]` | 当前用户可见的组列表（已过滤纯数字组名） |
| `group` | string | 回显的组名 |
| `q` | string | 回显的搜索词 |

> 另有扩展钩子 `await this.ctx.parallel('contest/list', filter, this)`（contest.ts:55），插件可修改查询条件。

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| — | 该 Handler 只实现 `get`，POST 无 `operation` 时框架抛 `MethodNotAllowedError`（405） | — |

**示例**

```http
GET /d/system/contest?rule=acm&page=1
Accept: application/json
```

```json
{
  "page": 1,
  "tpcount": 3,
  "qs": "rule=acm",
  "rule": "acm",
  "tdocs": [{ "_id": "64f0...", "docId": "64f0...", "docType": 30, "title": "Example", "rule": "acm", "pids": [1001], "attend": 5 }],
  "tsdict": { "64f0...": null },
  "groups": [],
  "group": "",
  "q": ""
}
```

---

#### 创建比赛 · `contest_create`

```http
GET  /contest/create
POST /contest/create
```

| 项 | 值 |
|---|---|
| 处理器 | `ContestEditHandler`（contest.ts:410） |
| 方法（源码） | `prepare@414`、`get@425`、`postUpdate@468`、`postDelete@523`——**没有普通 `post`** |
| 认证 | 匿名可访问路由，`prepare` 内 `this.checkPerm(PERM_CREATE_CONTEST)` |
| 域权限 | `PERM_CREATE_CONTEST`（contest.ts:423） |
| 响应 | JSON 对象 / HTML 模板 `contest_edit.html` |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 创建路由无 `tid` |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | `get` 无 `@query` 装饰器 |

**请求体（`operation=update` → `postUpdate`，contest.ts:449-468）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `beginAtDate` | `Types.Date`（`YYYY-M-D`） | 是 | 开始日期 |
| `beginAtTime` | `Types.Time`（`H:mm`） | 是 | 开始时间（按 `this.user.timeZone` 解析） |
| `duration` | `Types.Float` | 是 | 时长（小时），`endAt = beginAt + duration`；若 `beginAt >= endAt` 抛 `ValidationError('duration')` |
| `title` | `Types.Title`（1–64 字符） | 是 | 比赛标题 |
| `content` | `Types.Content`（< 65536 字符） | 是 | 比赛描述（Markdown） |
| `rule` | `Types.String` | 是 | 赛制，必须是 `RULES` 的键且非 hidden，否则 `ValidationError('rule')` |
| `pids` | `Types.Content` | 是 | 题目 ID 列表，中文逗号会先替换为英文逗号后按 `,` 切分；非法/0 值被过滤 |
| `rated` | `Types.Boolean` | 否（默认 `false`） | 是否 Rated |
| `code` | `Types.String` | 否 | 比赛邀请码（`_code`） |
| `autoHide` | `Types.Boolean` | 否（默认 `false`） | 结束前隐藏题目；为真时需 `PERM_EDIT_PROBLEM`（contest.ts:489） |
| `assign` | `Types.CommaSeperatedArray` | 否 | 可参赛用户组名列表 |
| `lock` | `Types.UnsignedInt` | 否 | 封榜时长（分钟），`lockAt = endAt - lock`；与 `contestDuration` 同时给出会抛 `ValidationError` |
| `contestDuration` | `Types.Float` | 否 | 灵活时间模式（从首次打开比赛算起的可用小时数），写入 `tdoc.duration` |
| `maintainer` | `Types.NumericArray` | 否 | 协管员 uid 列表（逗号分隔字符串会被切分） |
| `allowViewCode` | `Types.Boolean` | 否（默认 `false`） | 允许查看他人代码 |
| `allowPrint` | `Types.Boolean` | 否（默认 `false`） | 开启打印服务 |
| `keepScoreboardHidden` | `Types.Boolean` | 否（默认 `false`） | 结束后仍隐藏榜单 |
| `allowTeam` | `Types.Boolean` | 否（默认 `false`） | 允许组队（创建时置位后不可关闭，contest.ts:490） |
| `langs` | `Types.CommaSeperatedArray` | 否 | 允许的语言列表 |

> 说明：`Types.Boolean` 在 `framework/framework/validator.ts:114` 中自带 `isOptional=true`，故所有布尔参数都可省略，省略时按 JS 默认值处理。

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `tid` | `ObjectId`（字符串） | 新建比赛 ID |
| `url` | string | 由 `base.ts:64-67` 注入：`this.response.redirect`（指向 `contest_detail`） |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| `update` | `postUpdate`，创建/更新比赛 | 见上表 |
| `delete` | `postDelete`（contest.ts:523），删除比赛、答疑、提交关联与附件 | `tid` |

**示例**

```http
POST /d/system/contest/create
Accept: application/json
Content-Type: application/json

{
  "operation": "update",
  "beginAtDate": "2026-01-01",
  "beginAtTime": "09:00",
  "duration": 5,
  "title": "Newbie Cup",
  "content": "Welcome!",
  "rule": "acm",
  "pids": "1001,1002"
}
```

```json
{ "tid": "64f0c0f0f0f0f0f0f0f0f0f0", "url": "/d/system/contest/64f0c0f0f0f0f0f0f0f0f0f0" }
```

---

#### 我的队伍 · `contest_team`

```http
GET  /contest/team
POST /contest/team
```

| 项 | 值 |
|---|---|
| 处理器 | `ContestTeamHandler`（contest.ts:983） |
| 方法（源码） | `prepare@984`、`get@988`、`postCreate@1005`、`postRename@1017`、`postInvite@1027`、`postAccept@1044`、`postReject@1054`、`postLeave@1063`——**没有普通 `post`** |
| 认证 | **需登录**（`prepare` 内 `this.checkPriv(PRIV.PRIV_USER_PROFILE)`，contest.ts:984-986） |
| 域权限 | — |
| 响应 | JSON 对象 / HTML 模板 `contest_team.html` |

> 注册注释（contest.ts:1081）：`// before /contest/:tid: "team" is not a valid ObjectId`，因此必须注册在 `contest_detail` 之前。

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 无 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | `get` 无装饰器 |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `name` | `Types.String` | 视 operation | 队伍显示名（`postCreate` / `postRename`），trim 后为空抛 `ValidationError('name')` |
| `vuid` | `Types.Int` | 视 operation | 虚拟用户（队伍）ID |
| `uid` | `Types.Int` | 视 operation | 目标用户 uid（`postInvite`、`postLeave`） |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `mine` | `VUserDoc[]` | 我所属的队伍（`user.getVusersByMember(this.user._id)`） |
| `invites` | `VUserDoc[]` | 邀请我的队伍（`user.getVusersByInvite(this.user._id)`） |
| `udict` | `UserDict` | 队伍成员与待接受邀请者的用户字典 |
| `page_name` | string | 固定 `"contest_team"` |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| `create` | 创建队伍，`key = team:{uid}:{randomstring(6)}`，创建者自动入队 | `name` |
| `rename` | 重命名队伍（需为队伍成员） | `vuid`、`name` |
| `invite` | 邀请用户入队；限流 `limitRate('contest_team_invite', 3600, 20)`；超出 `limit.team_members` 抛 `TeamMemberLimitError` | `vuid`、`uid` |
| `accept` | 接受邀请（`$pull invite` + `$addToSet members`） | `vuid` |
| `reject` | 拒绝邀请 | `vuid` |
| `leave` | 退出/移出队伍；`uid` 省略时为「我自己退出」，否则需为队伍成员（`mustMember`） | `vuid`、`uid`（可选） |

**示例**

```http
POST /d/system/contest/team
Accept: application/json
Content-Type: application/json

{ "operation": "create", "name": "Team A" }
```

```json
{}
```

---

#### 比赛详情 · `contest_detail`

```http
GET  /contest/:tid
POST /contest/:tid
```

| 项 | 值 |
|---|---|
| 处理器 | `ContestDetailHandler`（contest.ts:159），继承 `ContestDetailBaseHandler`（contest.ts:79） |
| 方法（源码） | `prepare@161`、`get@166`、`postAttend@191`、`postSubscribe@231`、`postEarlyEnd@238`；基类 `__prepare@85`、`after@117`——**没有普通 `post`** |
| 认证 | 匿名可访问（路由级 `PERM_VIEW_CONTEST`）；若 `tdoc.assign` 非空则需属于对应组，或 `PERM_VIEW_HIDDEN_CONTEST` |
| 域权限 | `PERM_VIEW_CONTEST` |
| 响应 | JSON 对象 / HTML 模板 `contest_detail.html` |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `tid` | `Types.ObjectId` | 比赛 ID（`__prepare`，contest.ts:84-85） |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | `get` 无查询参数 |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `code` | `Types.String` | 否 | 邀请码（`postAttend`） |
| `vuid` | `Types.Int` | 否 | 以队伍身份参赛时的虚拟用户 ID |
| `subscribe` | `Types.Boolean` | 否（默认 `false`） | `postSubscribe` 订阅开关 |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `tdoc` | `Tdoc` | 比赛文档（含 `title`/`content`/`rule`/`beginAt`/`endAt`/`pids`/`attend`/`owner`/`rated`/`lockAt`/`unlocked`/`duration`/`score`/`allowTeam` 等；见 `interface.ts:260-301`） |
| `tsdoc` | object \| null | `tsdocAsPublic()`（contest.ts:107-114）裁剪结果：`attend`、`subscribe`、`startAt`、`displayName`、`members`，以及存在 `duration`/`endAt` 时的 `endAt` |
| `udict` | `UserDict` | 仅含比赛所有者（`user.getList(domainId, [tdoc.owner])`） |
| `team_vdocs` | `VUserDoc[]` | 已登录且 `allowTeam` 时，我为成员的队伍列表；否则 `[]` |
| `files` | `FileInfo[]` | 私有附件（已 `sortFiles`）；仅在 `tsdoc.attend && !isNotStarted(tdoc)` 时非空 |
| `urlForFile` | function | `(filename) => url('contest_file_download', {tid, filename, type:'private'})`。**函数在 JSON 序列化时会被丢弃**（`JSON.stringify` 语义） |

> 非 JSON（HTML）模式下，`tdoc.content` 中的 `(file://` 与 `="file://` 会被重写为 `./{tid}/file/public/`（contest.ts:185-187）；JSON 模式在重写前 `return`（contest.ts:184），因此 JSON 中的 `content` 保持原始 `file://` 链接。

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| `attend` | `postAttend`（contest.ts:191）：报名。需 `PERM_ATTEND_CONTEST`；比赛已结束抛 `ContestNotLiveError`；`_code` 不匹配抛 `InvalidTokenError`；`vuid` 分支会校验队伍成员关系、`assign` 组、以及队员是否已报名（`ContestAlreadyAttendedError`） | `code`（可选）、`vuid`（可选） |
| `subscribe` | `postSubscribe`（contest.ts:231）：切换邮件/消息订阅；未报名抛 `ContestNotAttendedError` | `subscribe`（布尔） |
| `early_end` | `postEarlyEnd`（contest.ts:238）：提前结束我的参赛计时，写入 `tsdoc.endAt = now`；`rule === 'homework'` 抛 `ContestNotFoundError` | — |

**示例**

```http
GET /d/system/contest/64f0c0f0f0f0f0f0f0f0f0f0
Accept: application/json
```

```json
{
  "tdoc": {
    "_id": "64f0...", "docId": "64f0...", "docType": 30, "domainId": "system",
    "title": "Example Contest", "content": "...", "rule": "acm",
    "beginAt": "2026-01-01T01:00:00.000Z", "endAt": "2026-01-01T06:00:00.000Z",
    "pids": [1001, 1002], "attend": 5, "owner": 2, "rated": false
  },
  "tsdoc": { "attend": 1, "subscribe": 1, "startAt": "2026-01-01T01:02:11.000Z" },
  "udict": { "2": { "_id": 2, "uname": "admin" } },
  "team_vdocs": [],
  "files": []
}
```

---

#### 比赛题目列表 · `contest_problemlist`

```http
GET  /contest/:tid/problems
POST /contest/:tid/problems
```

| 项 | 值 |
|---|---|
| 处理器 | `ContestProblemListHandler`（contest.ts:332） |
| 方法（源码） | `prepare@334`、`get@339`、`postClarification@394`——**没有普通 `post`** |
| 认证 | 匿名可访问（`PERM_VIEW_CONTEST`）；未开始抛 `ContestNotLiveError`，未报名且未结束抛 `ContestNotAttendedError`（contest.ts:340-341） |
| 域权限 | `PERM_VIEW_CONTEST` |
| 响应 | JSON 对象 / HTML 模板 `contest_problemlist.html` |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `tid` | `Types.ObjectId` | 比赛 ID |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | `get` 无装饰器 |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `content` | `Types.Content` | 是（`postClarification`） | 提问内容 |
| `subject` | `Types.Int` | 是（`postClarification`） | 关联题目 pid；`0` 表示对整场比赛提问 |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `pdict` | `ProblemDict` | `problem.getList(..., PROJECTION_CONTEST_LIST)`：`_id`/`domainId`/`docType`/`docId`/`pid`/`owner`/`title`/`config` |
| `psdict` | `Record<pid, ContestDetailEntry>` | 当前用户的 `tsdoc.detail`（未参赛时为 `{}`） |
| `udict` | `UserDict` | 比赛所有者与当前用户 |
| `rdict` | `Record<rid, RecordDoc>` | 可见提交的评测记录字典；不可见时退化为 `{ [rid]: { _id } }` |
| `rdocs` | `RecordDoc[]` | 我（或我队伍）在本场的提交，按 `_id` 倒序；不可见时为 `[]` |
| `tdoc` | `Tdoc` | 比赛文档 |
| `tcdocs` | `ContestClarificationDoc[]` | 答疑列表（`contest.getMultiClarification`，未报名时仅公开 `owner=0` 的广播） |
| `showScore` | boolean | `Object.values(tdoc.score).some(i => i && i !== 100)`，即是否存在非满分权重的题目 |
| `tsdoc` | object \| null | 公开化的参赛状态（`tsdocAsPublic()`） |
| `canViewRecord` | boolean | `contest.canShowRecord.call(this, tdoc)`（contest.ts:1057） |
| `correction` | `Record<string, ProblemStatusDoc>` | 仅比赛结束且 `canViewRecord` 时出现：赛后订正记录（题目级 `psdoc`，剔除与本场 `detail[pid].rid` 相同者） |
| `rdict` / `psdict` | — | 非管理员（`!own && !PERM_EDIT_CONTEST`）时逐条经 `contest.applyProjection` 脱敏（见各赛制 `applyProjection`） |

> 副作用：首次访问且比赛进行中时，会把 `tsdoc.startAt` 置为当前时间（contest.ts:355-358），这是「灵活时长模式」的计时起点。

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| `clarification` | `postClarification`（contest.ts:394）：提交提问。需已报名且比赛进行中；限流 `limitRate('add_discussion', 3600, 60)`；非比赛所有者时向 `maintainer + owner` 发送站内信 | `content`、`subject` |

**示例**

```http
GET /d/system/contest/64f0c0f0f0f0f0f0f0f0f0f0/problems
Accept: application/json
```

```json
{
  "pdict": { "1001": { "docId": 1001, "pid": "A1", "title": "A+B" } },
  "psdict": {},
  "udict": { "2": { "_id": 2, "uname": "admin" } },
  "rdict": {},
  "rdocs": [],
  "tdoc": { "docId": "64f0...", "rule": "acm" },
  "tcdocs": [],
  "showScore": false,
  "tsdoc": { "attend": 1, "startAt": "2026-01-01T01:02:11.000Z" },
  "canViewRecord": true
}
```

---

#### 编辑比赛 · `contest_edit`

```http
GET  /contest/:tid/edit
POST /contest/:tid/edit
```

| 项 | 值 |
|---|---|
| 处理器 | `ContestEditHandler`（contest.ts:410），与 `contest_create` 同一类 |
| 方法（源码） | 同 `ContestEditHandler`：`prepare@414`、`get@425`、`postUpdate@468`、`postDelete@523`——**没有普通 `post`** |
| 认证 | 匿名可访问路由（路由级 `PERM_VIEW_CONTEST`）；`prepare`（contest.ts:413-423）：非所有者需 `PERM_EDIT_CONTEST`，所有者需 `PERM_EDIT_CONTEST_SELF`；隐藏赛制抛 `ContestNotFoundError` |
| 域权限 | `PERM_VIEW_CONTEST` + `PERM_EDIT_CONTEST` / `PERM_EDIT_CONTEST_SELF` |
| 响应 | JSON 对象 / HTML 模板 `contest_edit.html` |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `tid` | `Types.ObjectId`（可选，`@param('tid', Types.ObjectId, true)`） | 比赛 ID；省略即进入「创建」模式 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 无 |

**请求体（`operation=update`）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `tid` | `Types.ObjectId` | 否 | 目标比赛；与路径 `:tid` 二选一（装饰器 source 为 `all`，路径参数优先被 `Object.assign(args, ctx.params)` 覆盖） |
| `beginAtDate` | `Types.Date` | 是 | 开始日期 |
| `beginAtTime` | `Types.Time` | 是 | 开始时间 |
| `duration` | `Types.Float` | 是 | 时长（小时） |
| `title` | `Types.Title` | 是 | 标题 |
| `content` | `Types.Content` | 是 | 描述 |
| `rule` | `Types.String` | 是 | 赛制 |
| `pids` | `Types.Content` | 是 | 题目列表（逗号分隔，支持中文逗号） |
| `rated` | `Types.Boolean` | 否 | Rated |
| `code` | `Types.String` | 否 | 邀请码 |
| `autoHide` | `Types.Boolean` | 否 | 结束前隐藏题目（需 `PERM_EDIT_PROBLEM`） |
| `assign` | `Types.CommaSeperatedArray` | 否 | 允许参赛的组 |
| `lock` | `Types.UnsignedInt` | 否 | 封榜分钟数 |
| `contestDuration` | `Types.Float` | 否 | 灵活时长（小时） |
| `maintainer` | `Types.NumericArray` | 否 | 协管员 uid |
| `allowViewCode` | `Types.Boolean` | 否 | 允许看代码 |
| `allowPrint` | `Types.Boolean` | 否 | 开启打印 |
| `keepScoreboardHidden` | `Types.Boolean` | 否 | 结束后仍隐藏榜单 |
| `allowTeam` | `Types.Boolean` | 否 | 允许组队（`allowTeam \|\|= !!this.tdoc?.allowTeam`，无法关闭） |
| `langs` | `Types.CommaSeperatedArray` | 否 | 语言白名单 |

> 关键副作用（contest.ts:495-520）：`beginAt`/`endAt`/`pids`/`rule`/`lockAt` 任一变化都会触发 `contest.recalcStatus(domainId, tid)` 重算所有 `tsdoc` 统计；`autoHide` 为真且未结束时，会写入一条 `ScheduleModel`（`type:'schedule', subType:'contest', operation:['unhide'], executeAfter:endAt`），由 worker `ctx.worker.addHandler('contest', ...)`（contest.ts:1094-1105）在赛后取消隐藏题目。

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `tid` | `ObjectId`（字符串） | 比赛 ID（创建时为新 ID） |
| `url` | string | 重定向到 `contest_detail`（`this.response.redirect`） |

> `GET` 的响应字段：`rules`（`{ rule: TEXT }`，已过滤 hidden）、`tdoc`、`duration`（小时，由 `beginAt` 与 `endAt` 差算出；新建时默认 `2`）、`pids`（逗号串）、`beginAt`（moment，已按用户时区）、`page_name`（`contest_edit` 或 `contest_create`）、`files`、`urlForFile`。

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| `update` | `postUpdate`（contest.ts:449） | 见上表 |
| `delete` | `postDelete`（contest.ts:523）：删除比赛、其答疑、`record.updateMulti` 解除提交关联、删除调度任务与全部附件；非所有者需 `PERM_EDIT_CONTEST` | — |

**示例**

```http
POST /d/system/contest/64f0c0f0f0f0f0f0f0f0f0f0/edit
Accept: application/json
Content-Type: application/json

{ "operation": "update", "beginAtDate": "2026-01-01", "beginAtTime": "09:00",
  "duration": 5, "title": "Example Contest", "content": "...", "rule": "acm",
  "pids": "1001,1002", "lock": 60 }
```

```json
{ "tid": "64f0c0f0f0f0f0f0f0f0f0f0", "url": "/d/system/contest/64f0c0f0f0f0f0f0f0f0f0f0" }
```

---

#### 打印服务 · `contest_print`

```http
GET  /contest/:tid/print
POST /contest/:tid/print
```

| 项 | 值 |
|---|---|
| 处理器 | `ContestPrintHandler`（contest.ts:248） |
| 方法（源码） | `prepare@250`、`get@257`、**`post@262`（本文件中唯一实现普通 `post` 的比赛类 Handler）**、`postPrint@283`、`postGetPrintTask@299`、`postAllocatePrintTask@309`、`postUpdatePrintTask@321` |
| 认证 | 匿名可访问路由（`PERM_VIEW_CONTEST`）；`prepare`（contest.ts:249-255）：`!tdoc.allowPrint` 抛 `NotFoundError`；非所有者、无 `PERM_EDIT_CONTEST`、且未报名 → `ContestNotAttendedError` |
| 域权限 | `PERM_VIEW_CONTEST` |
| 响应 | JSON 对象 / HTML 模板 `contest_print.html` |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `tid` | `Types.ObjectId` | 比赛 ID |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 无 |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `title` | `Types.Title` | 否（`postPrint`） | 打印任务标题；若上传了文件则回退为 `file.originalFilename` 或 `"file"` |
| `content` | `Types.Content` | 否（`postPrint`） | 打印正文；两者皆空抛 `ValidationError('content')` |
| `taskId` | `Types.ObjectId` | 是（`postUpdatePrintTask`） | 打印任务 ID |
| `status` | `Types.Range(['printed','pending'])` | 是（`postUpdatePrintTask`） | 目标状态 |

> 文件上传字段：`multipart/form-data` 的 `file`（`this.request.files.file`），限制 1 MiB（contest.ts:288-293）。

**响应字段（`Accept: application/json`）**

| 场景 | 字段 | 类型 | 说明 |
|---|---|---|---|
| `GET` | `tdoc` | `Tdoc` | 比赛文档 |
| `operation=print` | — | — | 成功后走 `this.back()`（302 回上一页），JSON 下为 `{ "url": "<referer>" }` |
| `operation=get_print_task` | `tasks` | `{_id,title,owner,status}[]` | 打印任务列表，按 `_id` 升序；非管理员只能看到自己的（`{ owner: this.user._id }`） |
| `operation=get_print_task` | `udict` | `UserDict` | 任务所有者的用户字典（`getListForRender`，是否含 `displayName` 取决于 `PERM_VIEW_USER_PRIVATE_INFO`） |
| `operation=allocate_print_task` | `task` | `ContestPrintDoc \| null` | 原子取出一个 `pending` 任务并置为 `printing`（`contest.allocatePrintTask`） |
| `operation=allocate_print_task` | `udoc` | `UserDoc \| null` | 任务所属用户 |
| `operation=update_print_task` | `success` | boolean | 固定 `true` |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| `print` | `postPrint`（contest.ts:283）：提交打印任务。需已报名且比赛进行中（`ContestNotAttendedError` / `ContestNotLiveError`）；限流 `limitRate('add_print', 3600, 60)`；写入 `contest.addPrintTask`（`docType = 32`，状态 `pending`） | `title`、`content`（或 `file` 上传） |
| `get_print_task` | `postGetPrintTask`（contest.ts:299） | — |
| `allocate_print_task` | `postAllocatePrintTask`（contest.ts:309）：非所有者需 `PERM_EDIT_CONTEST`，否则 `PermissionError` | — |
| `update_print_task` | `postUpdatePrintTask`（contest.ts:321）：同上权限校验 | `taskId`、`status` |

**特例：无 `operation` 的 POST（DOMjudge 兼容）**（contest.ts:262-281）

`post()` 会拦截：若 `this.args.operation` 存在则直接 `return`（交回框架派发子操作）；否则若同时存在 `file_contents`（base64）与 `original_name`，则内部调用 `postPrint({ ...args, title: original_name, content: Buffer.from(file_contents,'base64').toString('utf-8') })`，并返回：

| 字段 | 类型 | 说明 |
|---|---|---|
| `success` | boolean | 是否成功 |
| `output` | string | 成功时为 `""`，失败时为异常 `message` |

否则抛 `MethodNotAllowedError('POST')`。

**示例**

```http
POST /d/system/contest/64f0c0f0f0f0f0f0f0f0f0f0/print
Accept: application/json
Content-Type: application/json

{ "operation": "print", "title": "Debug A", "content": "print(1)" }
```

```json
{ "url": "/d/system/contest/64f0c0f0f0f0f0f0f0f0f0f0/print" }
```

---

#### 打印服务（DOMjudge 兼容） · `contest_print_alt`

```http
GET  /contest/:tid/api/printing/team
POST /contest/:tid/api/printing/team
```

| 项 | 值 |
|---|---|
| 处理器 | `ContestPrintHandler`（contest.ts:248），与 `contest_print` 同一个类 |
| 方法（源码） | 同 `ContestPrintHandler`（含普通 `post@262`，故支持无 `operation` 的 POST） |
| 认证 | 同 `contest_print`（路由级 `PERM_VIEW_CONTEST`；`prepare` 校验 `allowPrint` 与报名状态） |
| 域权限 | `PERM_VIEW_CONTEST` |
| 响应 | JSON 对象 / HTML 模板 `contest_print.html` |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `tid` | `Types.ObjectId` | 比赛 ID |

**查询参数 / 请求体 / POST 子操作**

与 `contest_print` **完全一致**（同一 Handler、同一批装饰器）。注册源码注释即说明用途：

```ts
// contest.ts:1086-1087
// Support for DOMJudge printfile
ctx.Route('contest_print_alt', '/contest/:tid/api/printing/team', ContestPrintHandler, PERM.PERM_VIEW_CONTEST);
```

主要差异在**调用约定**：DOMjudge 客户端会以 `POST` 提交 `file_contents`（base64）+ `original_name`，命中 contest.ts:262-281 的兼容分支，返回 `{ success, output }`。

**示例**

```http
POST /d/system/contest/64f0c0f0f0f0f0f0f0f0f0f0/api/printing/team
Accept: application/json
Content-Type: application/x-www-form-urlencoded

original_name=main.cpp&file_contents=I2luY2x1ZGU8Y3N0ZGlvPgo=
```

```json
{ "success": true, "output": "" }
```

---

#### 比赛文件管理 · `contest_manage`

```http
GET  /contest/:tid/management
POST /contest/:tid/management
```

| 项 | 值 |
|---|---|
| 处理器 | `ContestManagementHandler`（contest.ts:602），继承 `ContestManagementBaseHandler`（contest.ts:545） |
| 方法（源码） | `get@606`、`postUploadFile@630`、`postDeleteFiles@657`、`postSetScore@670`；基类 `ContestManagementBaseHandler.prepare@546`——**没有普通 `post`** |
| 认证 | 路由无权限；基类 `prepare` 要求「比赛所有者」或 `PERM_EDIT_CONTEST` |
| 域权限 | `PERM_EDIT_CONTEST`（或所有者） |
| 响应 | JSON 对象 / HTML 模板 `contest_manage.html`（pjax 片段 `partials/files.html`） |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `tid` | `Types.ObjectId` | 比赛 ID |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `d` | `Types.Range(['public','private'])` | 否 | 只渲染对应分栏的 pjax 片段 |
| `sidebar` | `Types.Boolean` | 否 | 传给 `partials/files.html` 的 `sidebar` 标志 |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `filename` | `Types.Filename`（`@post`，来自 body） | 否 | 上传文件名；省略时取 `file.originalFilename` 或 `randomstring(16)` |
| `type` | `Types.Range(['private','public'])`（`@post`） | 否（默认 `private`） | 附件可见性；存储路径 `contest/{domainId}/{tid}/{type}/{filename}` |
| `files` | `Types.ArrayOf(Types.Filename)`（`@post`） | 是（`postDeleteFiles`） | 待删除文件名数组 |
| `pid` | `Types.PositiveInt` | 是（`postSetScore`） | 题目 ID，必须属于 `tdoc.pids`，否则 `ValidationError('pid')` |
| `score` | `Types.PositiveInt` | 是（`postSetScore`） | 该题分值权重（`tdoc.score[pid]`），随后 `recalcStatus` 重算 |
| 文件字段 | multipart `file` | 是（`postUploadFile`） | 上传内容；缺省抛 `ValidationError('file')` |

> 容量限制：文件数 `>= limit.contest_files` 抛 `FileLimitExceededError('count')`；总大小 `>= limit.contest_files_size` 抛 `FileLimitExceededError('size')`（contest.ts:631-640）。

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `tdoc` | `Tdoc` | 比赛文档 |
| `tsdoc` | `ContestStatusDoc` | 当前用户的参赛状态（原始文档，非 public 裁剪） |
| `owner_udoc` | `UserDoc` | 比赛所有者 |
| `pdict` | `ProblemDict` | `PROJECTION_CONTEST_LIST + 'tag'` |
| `files` | `FileInfo[]` | 公共附件（已排序） |
| `privateFiles` | `FileInfo[]` | 私有附件（已排序） |
| `urlForFile` | function | `(filename, type) => url('contest_file_download', {tid, filename, type})`（JSON 中会被丢弃） |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| `upload_file` | `postUploadFile`（contest.ts:630）：`storage.put` 后把 `{_id,name,size,lastModified,etag}` 写入 `files` 或 `privateFiles`（按 `_id` 去重合并） | `filename`（可选）、`type`（可选）、multipart `file` |
| `delete_files` | `postDeleteFiles`（contest.ts:657）：删除存储对象并从 `files`/`privateFiles` 中移除 | `files`、`type`（可选） |
| `set_score` | `postSetScore`（contest.ts:670）：设置单题权重分，并 `contest.recalcStatus` | `pid`、`score` |

> 注意：`postSetScore` 的装饰器只有 `pid`/`score`，没有 `tid`，它直接使用 `this.tdoc.docId`（由 `__prepare` 填充）。

**示例**

```http
POST /d/system/contest/64f0c0f0f0f0f0f0f0f0f0f0/management
Accept: application/json
Content-Type: application/json

{ "operation": "set_score", "pid": 1001, "score": 200 }
```

```json
{ "url": "/d/system/contest/64f0c0f0f0f0f0f0f0f0f0f0/management" }
```

---

#### 比赛答疑 · `contest_clarification`

```http
GET  /contest/:tid/clarification
POST /contest/:tid/clarification
```

| 项 | 值 |
|---|---|
| 处理器 | `ContestClarificationHandler`（contest.ts:680，非导出类），继承 `ContestManagementBaseHandler` |
| 方法（源码） | `get@682`、`postClarification@703`——**没有普通 `post`** |
| 认证 | 路由无权限；基类 `prepare` 要求「所有者」或 `PERM_EDIT_CONTEST` |
| 域权限 | `PERM_EDIT_CONTEST`（或所有者） |
| 响应 | JSON 对象 / HTML 模板 `contest_clarification.html`（pjax 片段 `partials/contest_clarification.html`） |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `tid` | `Types.ObjectId` | 比赛 ID |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 无 |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `content` | `Types.Content` | 是 | 回复或广播内容 |
| `did` | `Types.ObjectId` | 否 | 若给出则为「回复某条提问」（`contest.addClarificationReply`，并给提问者发 `FLAG_ALERT` 站内信） |
| `subject` | `Types.Int` | 否（默认 `0`） | 关联题目 pid；仅在广播分支（无 `did`）使用 |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `tdoc` | `Tdoc` | 比赛文档 |
| `tsdoc` | `ContestStatusDoc` | 当前用户参赛状态 |
| `owner_udoc` | `UserDoc` | 比赛所有者 |
| `pdict` | `ProblemDict` | `PROJECTION_CONTEST_LIST + 'tag'` |
| `tcdocs` | `ContestClarificationDoc[]` | 全部答疑（`getMultiClarification(domainId, tid)`，按 `_id` 倒序） |
| `udict` | `UserDict` | 答疑发起者的用户字典 |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| `clarification` | `postClarification`（contest.ts:703）。`did` 存在 → 回复该提问并通知提问者；否则 → 广播给所有 `subscribe:1` 的参赛者（含队伍成员展开），进行中时使用 `FLAG_ALERT`，否则 `FLAG_UNREAD` | `content`、`did`（可选）、`subject`（可选） |

**示例**

```http
POST /d/system/contest/64f0c0f0f0f0f0f0f0f0f0f0/clarification
Accept: application/json
Content-Type: application/json

{ "operation": "clarification", "content": "请注意题目 A 的输入格式。", "subject": 0 }
```

```json
{ "url": "/d/system/contest/64f0c0f0f0f0f0f0f0f0f0f0/clarification" }
```

---

#### 导出比赛代码 · `contest_code`

```http
GET /contest/:tid/code
GET /d/:domainId/contest/:tid/code
```

| 项 | 值 |
|---|---|
| 处理器 | `ContestCodeHandler`（contest.ts:551） |
| 方法（源码） | 仅 `get@554`——**没有 `post`，也不支持任何 `operation`**；POST 一律 `MethodNotAllowedError` |
| 认证 | 匿名可访问路由（`PERM_VIEW_CONTEST`）；`get` 内：非所有者时需 `PRIV_READ_RECORD_CODE` 或 `PERM_READ_RECORD_CODE`，且比赛必须已结束（`ContestNotEndedError`）；再经 `canShowRecord` 校验，否则 `PermissionError(PERM_VIEW_CONTEST_HIDDEN_SCOREBOARD)` |
| 域权限 | `PERM_VIEW_CONTEST` + `PERM_READ_RECORD_CODE` |
| 响应 | **二进制 ZIP**（`this.binary(zip, `${tdoc.title}.zip`)`），无 JSON 结构 |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `tid` | `Types.ObjectId` | 比赛 ID |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `all` | `Types.Boolean` | 否（默认假） | `true` → 导出 `tsdoc.journal` 中全部提交；否则只导出 `tsdoc.detail`（每题最后一次） |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `all` | `Types.Boolean` | 否 | 装饰器 source 为 `all`，body 亦可 |

**响应字段（`Accept: application/json`）**

响应为 ZIP 二进制流，无 JSON 结构。压缩包内文件命名规则（contest.ts:565-577）：

- `all=true`：`U{uid}_P{pid}_R{rid}`，若该提交带数字 `score` 则追加 `_S{status}@{score}`
- `all=false`：同上，但取 `tsdoc.detail[pid]`
- 扩展名：有文件存储的提交用 `rdoc.files.code` 中 `#` 后的文件名（缺省 `txt`），否则用 `rdoc.lang`

**其它**

- 限流：`await this.limitRate('contest_code', 60, 10)`（contest.ts:555），60 秒内最多 10 次。

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| — | 该 Handler 只实现 `get`；POST 无 `operation` 时抛 `MethodNotAllowedError` | — |

**示例**

```http
GET /d/system/contest/64f0c0f0f0f0f0f0f0f0f0f0/code?all=true
Cookie: sid=<token>
```

```
（响应体为 application/zip 二进制）
```

---

#### 比赛附件下载 · `contest_file_download`

```http
GET /contest/:tid/file/:type/:filename
GET /d/:domainId/contest/:tid/file/:type/:filename
```

| 项 | 值 |
|---|---|
| 处理器 | `ContestFileDownloadHandler`（contest.ts:731） |
| 方法（源码） | 仅 `get@736`——**没有 `post`** |
| 认证 | 匿名可访问路由（`PERM_VIEW_CONTEST`）；`type=private` 时需「所有者 / `PERM_EDIT_CONTEST` / 已报名且比赛进行中或已结束」 |
| 域权限 | `PERM_VIEW_CONTEST` |
| 响应 | **302 重定向**到 `storage.signDownloadLink(...)`（`this.response.redirect`），无 JSON 结构 |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `tid` | `Types.ObjectId` | 比赛 ID |
| `type` | `Types.Range(['public','private'])` | 附件分区；默认 `private`。存储键为 `contest/{domainId}/{tid}/{type}/{filename}` |
| `filename` | `Types.Filename` | 文件名（正则 `^[^\\/?#~!\|*]{1,255}$` 且 `sanitize` 后不变） |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `noDisposition` | `Types.Boolean` | 否（默认 `false`） | `true` → 不设置 `Content-Disposition` 文件名（内联预览） |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `noDisposition` | `Types.Boolean` | 否 | source 为 `all`，body 亦可 |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `url` | string | 签名下载链接（`response.body.url = response.redirect`，由 `base.ts:64-67` 注入） |

**行为细节**（contest.ts:737-755）

- 隐藏赛制且 `features` 不含 `download` → `ContestNotFoundError`。
- `type=private` 且非管理员时：未报名 → `ContestNotAttendedError`；既未进行也未结束 → `ContestNotLiveError`；`tsdoc.startAt` 为空则补写当前时间（开始计时）。
- 写入操作日志 `oplog.log(this, 'download.file.contest', { target, size })`，并设置 `Cache-Control: public`。

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| — | 只有 `get`；POST → `MethodNotAllowedError` | — |

**示例**

```http
GET /d/system/contest/64f0c0f0f0f0f0f0f0f0f0f0/file/public/statement.pdf
Accept: application/json
```

```json
{ "url": "https://cdn.example.com/contest/system/64f0.../public/statement.pdf?token=..." }
```

---

#### 参赛用户管理 · `contest_user`

```http
GET  /contest/:tid/user
POST /contest/:tid/user
```

| 项 | 值 |
|---|---|
| 处理器 | `ContestUserHandler`（contest.ts:758），继承 `ContestManagementBaseHandler` |
| 方法（源码） | `get@760`、`postAddUser@786`、`postRank@793`、`postResume@802`、`postRemoveUser@816`——**没有普通 `post`** |
| 认证 | 路由无权限；基类 `prepare` 要求「所有者」或 `PERM_EDIT_CONTEST` |
| 域权限 | `PERM_EDIT_CONTEST`（或所有者） |
| 响应 | JSON 对象 / HTML 模板 `contest_user.html`（pjax 片段 `partials/contest_user.html`） |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `tid` | `Types.ObjectId` | 比赛 ID |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 无 |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `uids` | `Types.NumericArray` | 是（`postAddUser`） | 批量添加的 uid，支持逗号分隔字符串 |
| `unrank` | `Types.Boolean` | 否（默认 `false`） | 添加时是否打星（不参与排名） |
| `uid` | `Types.PositiveInt`（`postRank` / `postResume`） | 是 | 目标用户 uid |
| `uid` | `Types.Int`（`postRemoveUser`） | 是 | 目标用户 uid（可为负，用于虚拟用户） |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `tdoc` | `Tdoc` | 比赛文档 |
| `tsdocs` | `ContestStatusDoc[]` | 全部参赛状态，projection 为 `{ uid, attend, startAt, unrank, endAt, displayName, members }`；灵活时长模式下会就地重算 `endAt`（contest.ts:763-771） |
| `udict` | `UserDict` | 所有者 + 全部参赛者 + 队伍成员（`getListForRender`） |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| `add_user` | `postAddUser`（contest.ts:786）：批量 `contest.attend(domainId, tid, uid, { unrank })` | `uids`、`unrank`（可选） |
| `rank` | `postRank`（contest.ts:793）：切换 `unrank`（打星/取消打星）；未报名抛 `ContestNotAttendedError(uid)` | `uid` |
| `resume` | `postResume`（contest.ts:802）：清空 `tsdoc.endAt`（恢复比赛时间）；比赛或灵活时长已过则 `ContestNotLiveError` | `uid` |
| `remove_user` | `postRemoveUser`（contest.ts:816）：取消报名；比赛已开始抛 `ContestAlreadyStartedError` | `uid` |

**示例**

```http
POST /d/system/contest/64f0c0f0f0f0f0f0f0f0f0f0/user
Accept: application/json
Content-Type: application/json

{ "operation": "add_user", "uids": "2,3,4", "unrank": false }
```

```json
{ "url": "/d/system/contest/64f0c0f0f0f0f0f0f0f0f0f0/user" }
```

---

#### 气球管理 · `contest_balloon`

```http
GET  /contest/:tid/balloon
POST /contest/:tid/balloon
```

| 项 | 值 |
|---|---|
| 处理器 | `ContestBalloonHandler`（contest.ts:825），继承 `ContestManagementBaseHandler` |
| 方法（源码） | `get@828`、`postSetColor@849`、`postDone@863`——**没有普通 `post`** |
| 认证 | 路由无权限；基类 `prepare` 要求「所有者」或 `PERM_EDIT_CONTEST` |
| 域权限 | `PERM_EDIT_CONTEST`（或所有者） |
| 响应 | JSON 对象 / HTML 模板 `contest_balloon.html`（pjax 片段 `partials/contest_balloon.html`） |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `tid` | `Types.ObjectId` | 比赛 ID |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `todo` | `Types.Boolean` | 否（默认 `false`） | 只看未发送气球（`{ sent: { $exists: false } }`） |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `color` | `Types.Content` | 是（`postSetColor`） | YAML 文本，形如 `1001: "#ff0000"`；必须覆盖 `tdoc.pids` 全部题目且可解析为对象，否则 `ValidationError('color')` |
| `balloon` | `Types.ObjectId` | 是（`postDone`） | 气球记录 `_id`（即首次 AC 的 `rid`） |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `tdoc` | `Tdoc` | 比赛文档 |
| `tsdoc` | `ContestStatusDoc` | 当前用户参赛状态 |
| `owner_udoc` | `UserDoc` | 比赛所有者 |
| `pdict` | `ProblemDict` | `PROJECTION_CONTEST_LIST` |
| `bdocs` | `BalloonDoc[]` | 气球记录，按 `_id` 倒序；未封榜或拥有 `PERM_VIEW_CONTEST_HIDDEN_SCOREBOARD` 时不受 `lockAt` 限制 |
| `udict` | `UserDict` | 首次 AC 者与发送者的用户字典 |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| `set_color` | `postSetColor`（contest.ts:849）：把 `tdoc.balloon` 写为 `{ [pid]: color }` | `color`（YAML） |
| `done` | `postDone`（contest.ts:863）：把气球标记为已发送（`{ sent: this.user._id, sentAt: new Date() }`）；已发送则 `ValidationError('Balloon already sent')` | `balloon` |

**示例**

```http
POST /d/system/contest/64f0c0f0f0f0f0f0f0f0f0f0/balloon
Accept: application/json
Content-Type: application/json

{ "operation": "done", "balloon": "64f0a1b2c3d4e5f60718293a" }
```

```json
{ "url": "/d/system/contest/64f0c0f0f0f0f0f0f0f0f0f0/balloon" }
```

---

#### 比赛排行榜 · `contest_scoreboard`

```http
GET  /contest/:tid/scoreboard
POST /contest/:tid/scoreboard
```

| 项 | 值 |
|---|---|
| 处理器 | `ContestScoreboardHandler`（contest.ts:890） |
| 方法（源码） | `get@893`、`postUnlock@931`——**没有普通 `post`** |
| 认证 | 匿名可访问路由（`PERM_VIEW_CONTEST_SCOREBOARD`）；`get` 内非所有者时再校验 `canShowScoreboard`（`ContestScoreboardHiddenError`）与 `isNotStarted`（`ContestNotLiveError`） |
| 域权限 | `PERM_VIEW_CONTEST_SCOREBOARD` |
| 响应 | JSON 对象 / HTML 模板 `contest_scoreboard.html`（pjax 片段 `partials/scoreboard.html`） |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `tid` | `Types.ObjectId` | 比赛 ID |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `view` | `Types.String` | 否（默认 `'default'`） | 视图 ID，见 `ctx.scoreboard.views`；不存在抛 `NotFoundError('View {id} not found')` |
| `realtime` | `Types.Boolean` | 否 | **仅 `default` 视图**的参数。`true` 时不解封榜数据，且非所有者需 `PERM_VIEW_CONTEST_HIDDEN_SCOREBOARD` |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `view` | `Types.String` | 否 | 装饰器 source 为 `all` |
| `realtime` | `Types.Boolean` | 否 | 同上 |

**响应字段（`Accept: application/json`）**

`default` 视图的响应体（contest.ts:1126-1131）——**完整结构与各赛制差异见下一章**：

| 字段 | 类型 | 说明 |
|---|---|---|
| `tdoc` | `Tdoc` | 比赛文档 |
| `tsdoc` | object \| null | `tsdocAsPublic()` |
| `rows` | `ScoreboardRow[]` | **`rows[0]` 是表头**（列定义），`rows[1..]` 为数据行；本项目**没有** `cols` 字段 |
| `udict` | `UserDict` | 榜单用户字典（`getListForRender`） |
| `pdict` | `ProblemDict` | 题目字典（`PROJECTION_CONTEST_DETAIL`），并被就地附加 `nAccept` / `nSubmit` |
| `page_name` | string | `contest_scoreboard` 或 `homework_scoreboard` |
| `groups` | `{ name, uids }[]` | 可筛选的用户组（管理员看到全部组，普通用户只看到自己所属组） |
| `availableViews` | `Record<string, string>` | `ctx.scoreboard.getAvailableViews(rule, this)` 的结果：`{ viewId: 显示名 }` |

其它视图：`ghost` → `this.binary(text, '{title}.ghost')`；`html` → `this.binary(html, '{title}.html')`；`csv` → `this.binary(csv, '{title}.csv')`，均无 JSON 结构。

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| `unlock` | `postUnlock`（contest.ts:931）：解封榜单。非所有者需 `PERM_EDIT_CONTEST`；比赛未结束抛 `ContestNotEndedError`；内部调用 `contest.unlockScoreboard`（`model/contest.ts:1031`，置 `unlocked: true` 并 `recalcStatus`） | — |

**示例**

```http
GET /d/system/contest/64f0c0f0f0f0f0f0f0f0f0f0/scoreboard?realtime=true
Accept: application/json
```

```json
{
  "tdoc": { "docId": "64f0...", "rule": "acm", "pids": [1001] },
  "tsdoc": null,
  "rows": [
    [{ "type": "rank", "value": "#" }, { "type": "user", "value": "User" },
     { "type": "solved", "value": "Solved\nTotal Time" },
     { "type": "problem", "value": "A", "raw": 1001 }],
    [{ "type": "rank", "value": "1" }, { "type": "user", "value": "alice", "raw": 3 },
     { "type": "time", "value": "1\n00:12:03", "hover": "00:12:03" },
     { "type": "record", "score": 100, "value": "<span class=\"icon icon-check\"></span>\n00:12:03", "raw": "64f0b2..." }]
  ],
  "udict": { "3": { "_id": 3, "uname": "alice" } },
  "pdict": { "1001": { "docId": 1001, "title": "A+B", "nAccept": 1, "nSubmit": 2 } },
  "page_name": "contest_scoreboard",
  "groups": [],
  "availableViews": { "default": "Default", "ghost": "Ghost", "html": "HTML", "csv": "CSV" }
}
```

---

#### 比赛排行榜（指定视图） · `contest_scoreboard_view`

```http
GET  /contest/:tid/scoreboard/:view
POST /contest/:tid/scoreboard/:view
```

| 项 | 值 |
|---|---|
| 处理器 | `ContestScoreboardHandler`（contest.ts:890），与 `contest_scoreboard` 同一个类 |
| 方法（源码） | 同 `ContestScoreboardHandler`：`get@893`、`postUnlock@931`——**没有普通 `post`** |
| 认证 | 同 `contest_scoreboard`（`PERM_VIEW_CONTEST_SCOREBOARD`） |
| 域权限 | `PERM_VIEW_CONTEST_SCOREBOARD` |
| 响应 | 取决于视图：JSON 对象（`default`）/ ZIP 外二进制（`ghost`/`html`/`csv`） |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `tid` | `Types.ObjectId` | 比赛 ID |
| `view` | `Types.String` | 视图 ID，对应 `@param('view', Types.String, true)` 的第 3 个形参（`viewId = 'default'`） |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `realtime` | `Types.Boolean` | 否 | `default` 视图有效（见上表） |

**请求体 / POST 子操作**

与 `contest_scoreboard` 相同（`unlock`）。

**内置视图**（`scoreboard.addView`，contest.ts:1111-1206）

| view | 显示名 | 声明参数 | 行为 |
|---|---|---|---|
| `default` | Default | `{ tdoc: 'tdoc', groups: 'groups', realtime: Types.Boolean }` | 渲染 `contest_scoreboard.html`；`realtime` 时跳过 `lockAt` 限制（需 `PERM_VIEW_CONTEST_HIDDEN_SCOREBOARD`）；导出视图外唯一返回 JSON 的视图 |
| `ghost` | Ghost | `{ tdoc: 'tdoc' }` | 输出 DOMjudge `.ghost` 文本（`this.binary`）；封榜且非所有者时需 `PERM_VIEW_CONTEST_HIDDEN_SCOREBOARD` |
| `html` | HTML | `{ tdoc: 'tdoc' }` | `renderHTML('contest_scoreboard_download_html.html', { rows, tdoc })`，`config.isExport = true`；限流 `limitRate('scoreboard_download', 60, 3)` |
| `csv` | CSV | `{ tdoc: 'tdoc' }` | `toCSV(rows.map(r => r.map(c => c.value.toString())), { bom: true })`；同样限流 |

> 视图参数解析规则（contest.ts:902-924）：`view.args` 中值为**函数/数组**的按 `Types` 处理，值为**字符串**的按「fetcher」处理（当前支持 `'tdoc'` → `this.tdoc`、`'groups'` → `user.listGroup(...)`）。

**示例**

```http
GET /d/system/contest/64f0c0f0f0f0f0f0f0f0f0f0/scoreboard/csv
Cookie: sid=<token>
```

```
（响应体为 text/csv 二进制，带 BOM）
```

---

### 作业（Homework）路由

> 源码：`packages/hydrooj/src/handler/homework.ts`（共 322 行，路由注册在 309-321 行）。
> 作业本质上是一类特殊的比赛（`tdoc.rule === 'homework'`，TEXT 为 `Assignment`），文档与状态共用 `contest` 模型。

#### 路由总览

| route_name | 方法 | 路径 | 处理器 | 路由级权限 |
|---|---|---|---|---|
| `homework_main` | GET | `/homework` | `HomeworkMainHandler`（homework.ts:33） | `PERM_VIEW_HOMEWORK` |
| `homework_create` | GET / POST | `/homework/create` | `HomeworkEditHandler`（homework.ts:147） | — |
| `homework_detail` | GET / POST | `/homework/:tid` | `HomeworkDetailHandler`（homework.ts:79） | `PERM_VIEW_HOMEWORK` |
| `homework_code` | GET | `/homework/:tid/code` | `ContestCodeHandler`（contest.ts:551） | `PERM_VIEW_HOMEWORK` |
| `homework_edit` | GET / POST | `/homework/:tid/edit` | `HomeworkEditHandler`（homework.ts:147） | — |
| `homework_files` | GET / POST | `/homework/:tid/file` | `HomeworkFilesHandler`（homework.ts:254） | `PERM_VIEW_HOMEWORK` |
| `homework_file_download` | GET | `/homework/:tid/file/:type/:filename` | `ContestFileDownloadHandler`（contest.ts:731） | `PERM_VIEW_HOMEWORK` |
| `homework_scoreboard` | GET / POST | `/homework/:tid/scoreboard` | `ContestScoreboardHandler`（contest.ts:890） | `PERM_VIEW_HOMEWORK_SCOREBOARD` |
| `homework_scoreboard_view` | GET / POST | `/homework/:tid/scoreboard/:view` | `ContestScoreboardHandler`（contest.ts:890） | `PERM_VIEW_HOMEWORK_SCOREBOARD` |

**HTTP 方法与 `operation` 派发**：作业侧**没有任何 Handler 实现普通 `post`**，因此不带 `operation` 的 POST 一律 `MethodNotAllowedError`。可用 `operation` 仅：`homework_detail` → `attend`；`homework_create` / `homework_edit` → `update`、`delete`；`homework_files` → `upload_file`、`delete_files`；`homework_scoreboard(_view)` → `unlock`。`homework_main`、`homework_code`、`homework_file_download` 仅实现 `get`。

---

#### 作业列表 · `homework_main`

```http
GET /homework
GET /d/:domainId/homework
```

| 项 | 值 |
|---|---|
| 处理器 | `HomeworkMainHandler`（homework.ts:33） |
| 方法（源码） | 仅 `get@37`——**没有普通 `post`** |
| 认证 | 匿名可访问（路由级 `PERM_VIEW_HOMEWORK`） |
| 域权限 | `PERM_VIEW_HOMEWORK` |
| 响应 | JSON 对象 / HTML 模板 `homework_main.html` |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 无 |

**查询参数**（`HomeworkMainHandler.get`，homework.ts:34-37；source 为 `all`）

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `group` | `Types.Name` | 否 | 用户组过滤；不在可见组内抛 `NotAssignedError` |
| `page` | `Types.PositiveInt` | 否 | 页码，默认 1 |
| `q` | `Types.String` | 否 | 标题搜索（同比赛列表的正则策略） |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| （同上） | — | 否 | source 为 `all` |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `tdocs` | `Tdoc[]` | 作业列表，排序 `{ penaltySince: -1, endAt: -1, beginAt: -1, _id: -1 }` |
| `calendar` | `Tdoc[]` | 供日历视图使用：每项附带 `url = url('homework_detail', { tid })`；未延长且未结束时 `endAt` 被替换为 `penaltySince`（homework.ts:61-67） |
| `tpcount` | number | 总页数 |
| `page` | number | 当前页码 |
| `qs` | string | 查询串回显（`group=...&q=...`） |
| `groups` | `{ name, uids }[]` | 可见用户组（过滤纯数字名） |
| `group` | string | 回显组名 |
| `q` | string | 回显搜索词 |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| — | 只有 `get`；POST → `MethodNotAllowedError` | — |

**示例**

```http
GET /d/system/homework?page=1
Accept: application/json
```

```json
{
  "tdocs": [{ "docId": "64f1...", "title": "Homework 1", "rule": "homework", "penaltySince": "2026-01-08T15:59:00.000Z" }],
  "calendar": [{ "docId": "64f1...", "url": "/d/system/homework/64f1...", "endAt": "2026-01-08T15:59:00.000Z" }],
  "tpcount": 1, "page": 1, "qs": "", "groups": [], "group": "", "q": ""
}
```

---

#### 创建作业 · `homework_create`

```http
GET  /homework/create
POST /homework/create
```

| 项 | 值 |
|---|---|
| 处理器 | `HomeworkEditHandler`（homework.ts:147） |
| 方法（源码） | `get@149`、`postUpdate@193`、`postDelete@242`——**没有普通 `post`** |
| 认证 | 路由无权限；`get`/`postUpdate` 内：无 `tid` 时 `checkPerm(PERM_CREATE_HOMEWORK)` |
| 域权限 | `PERM_CREATE_HOMEWORK` |
| 响应 | JSON 对象 / HTML 模板 `homework_edit.html` |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 创建路由无 `tid` |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 无 |

**请求体（`operation=update` → `postUpdate`，homework.ts:180-193）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `beginAtDate` | `Types.Date` | 是 | 开始日期 |
| `beginAtTime` | `Types.Time` | 是 | 开始时间 |
| `penaltySinceDate` | `Types.Date` | 是 | 开始计罚日期 |
| `penaltySinceTime` | `Types.Time` | 是 | 开始计罚时间 |
| `extensionDays` | `Types.Float` | 是 | 延长天数；`endAt = penaltySince + extensionDays`；`beginAt >= penaltySince` 抛 `ValidationError` |
| `penaltyRules` | `Types.Content` + 校验 `validatePenaltyRules` + 转换 `convertPenaltyRules`（`yaml.load`） | 是 | 逾期系数表，形如 `{"1": 1, "24": 0.8}`（键为小时数，值为系数）；必须能解析为「键→数字」的对象 |
| `title` | `Types.Title` | 是 | 作业标题 |
| `content` | `Types.Content` | 是 | 作业描述 |
| `pids` | `Types.Content` | 是 | 题目 ID 列表（逗号分隔，支持中文逗号） |
| `rated` | `Types.Boolean` | 否（默认 `false`） | 是否 Rated |
| `maintainer` | `Types.NumericArray` | 否 | 协管员 uid（仅编辑分支写入） |
| `assign` | `Types.CommaSeperatedArray` | 否 | 允许的组（创建时写入，编辑时覆盖） |
| `langs` | `Types.CommaSeperatedArray` | 否 | 语言白名单（仅编辑分支写入） |

> 创建分支调用 `contest.add(domainId, title, content, uid, 'homework', beginAt, endAt, pids, rated, { penaltySince, penaltyRules, assign })`（homework.ts:223-227）。

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `tid` | `ObjectId`（字符串） | 新建作业 ID |
| `url` | string | 重定向到 `homework_detail` |

> `GET` 响应字段（homework.ts:181-190）：`tdoc`、`dateBeginText`、`timeBeginText`、`datePenaltyText`、`timePenaltyText`、`extensionDays`、`penaltyRules`（YAML 字符串）、`pids`、`page_name`（`homework_edit` 或 `homework_create`）。

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| `update` | `postUpdate`（homework.ts:193） | 见上表 |
| `delete` | `postDelete`（homework.ts:242）：解除提交关联、删除作业与公共附件；非所有者需 `PERM_EDIT_HOMEWORK` | `tid` |

**示例**

```http
POST /d/system/homework/create
Accept: application/json
Content-Type: application/json

{
  "operation": "update",
  "beginAtDate": "2026-01-01", "beginAtTime": "00:00",
  "penaltySinceDate": "2026-01-08", "penaltySinceTime": "00:00",
  "extensionDays": 7,
  "penaltyRules": "1: 1\n24: 0.8",
  "title": "Homework 1", "content": "Do it.", "pids": "1001"
}
```

```json
{ "tid": "64f1a0b1c2d3e4f5a6b7c8d9", "url": "/d/system/homework/64f1a0b1c2d3e4f5a6b7c8d9" }
```

---

#### 作业详情 · `homework_detail`

```http
GET  /homework/:tid
POST /homework/:tid
```

| 项 | 值 |
|---|---|
| 处理器 | `HomeworkDetailHandler`（homework.ts:79） |
| 方法（源码） | `prepare@83`、`get@95`、`postAttend@139`——**没有普通 `post`** |
| 认证 | 匿名可访问（路由级 `PERM_VIEW_HOMEWORK`）；`prepare`（homework.ts:82-90）：`tdoc.rule !== 'homework'` 抛 `ContestNotFoundError`；`assign` 非空且不在组内抛 `NotAssignedError` |
| 域权限 | `PERM_VIEW_HOMEWORK` |
| 响应 | JSON 对象 / HTML 模板 `homework_detail.html` |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `tid` | `Types.ObjectId` | 作业 ID |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `page` | `Types.PositiveInt` | 否 | 讨论区分页，默认 1 |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | `postAttend` 无装饰器参数 |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `tdoc` | `Tdoc` | 作业文档（`content` 已重写 `file://` 为相对路径，homework.ts:121-123） |
| `tsdoc` | `ContestStatusDoc \| null` | 我的作业状态（含 `journal`/`detail`/`score`/`penaltyScore`/`time`） |
| `udict` | `UserDict` | 讨论发起者 + 作业所有者 |
| `ddocs` | `DiscussionDoc[]` | 讨论区帖子（`this.paginate` 分页） |
| `page` | number | 当前页 |
| `dpcount` | number | 讨论总页数 |
| `dcount` | number | 讨论总数 |
| `pdict` | `ProblemDict` | `PROJECTION_CONTEST_LIST`（**仅当**已开始且（已认领或已结束）或拥有 `PERM_VIEW_HOMEWORK_HIDDEN_SCOREBOARD` 时返回，否则字段缺失） |
| `psdict` | `Record<pid, ContestDetailEntry>` | 每题有效提交（仅取 `tdoc.pids` 内的 `journal` 条目） |
| `rdict` | `Record<rid, RecordDoc \| {_id}>` | 可见时通过 `record.getList` 取详情，否则仅占位 `{ _id }` |

> 关键行为：`tsdoc.attend && !tsdoc.startAt && isOngoing` 时把 `startAt` 置为当前时间（homework.ts:130-133），即作业也有「灵活时长」语义。若 `isNotStarted || (!tsdoc.attend && !isDone)` 且非所有者、无隐藏榜单权限，函数提前 `return`（homework.ts:125-129），此时**没有** `pdict`/`psdict`/`rdict` 字段。

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| `attend` | `postAttend`（homework.ts:139）：认领作业。需 `PERM_ATTEND_HOMEWORK`；已结束抛 `HomeworkNotLiveError`；调用 `contest.attend(domainId, tid, uid)`（不带 `subscribe`） | — |

**示例**

```http
POST /d/system/homework/64f1a0b1c2d3e4f5a6b7c8d9
Accept: application/json
Content-Type: application/json

{ "operation": "attend" }
```

```json
{ "url": "/d/system/homework/64f1a0b1c2d3e4f5a6b7c8d9" }
```

---

#### 作业代码导出 · `homework_code`

```http
GET /homework/:tid/code
GET /d/:domainId/homework/:tid/code
```

| 项 | 值 |
|---|---|
| 处理器 | `ContestCodeHandler`（contest.ts:551，由 homework.ts:21 导入复用） |
| 方法（源码） | 仅 `get@554`——**没有 `post`，也不支持任何 `operation`** |
| 认证 | 匿名可访问路由（`PERM_VIEW_HOMEWORK`）；`get` 内：非所有者需 `PRIV_READ_RECORD_CODE` 或 `PERM_READ_RECORD_CODE`，且作业必须已结束；再经 `canShowRecord` 校验 |
| 域权限 | `PERM_VIEW_HOMEWORK` + `PERM_READ_RECORD_CODE` |
| 响应 | **二进制 ZIP**（`this.binary(zip, '{title}.zip')`），无 JSON 结构 |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `tid` | `Types.ObjectId` | 作业 ID |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `all` | `Types.Boolean` | 否（默认假） | `true` 导出全部 `journal`，否则只导出 `detail` |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `all` | `Types.Boolean` | 否 | source 为 `all` |

**响应字段（`Accept: application/json`）**

响应为 ZIP 二进制，无 JSON 结构；内部文件名规则与 `contest_code` 完全相同（`U{uid}_P{pid}_R{rid}[_S{status}@{score}]`）。

**其它**

- 限流：`limitRate('contest_code', 60, 10)`。

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| — | 只有 `get`；POST → `MethodNotAllowedError` | — |

**示例**

```http
GET /d/system/homework/64f1a0b1c2d3e4f5a6b7c8d9/code?all=true
Cookie: sid=<token>
```

```
（响应体为 application/zip 二进制）
```

---

#### 编辑作业 · `homework_edit`

```http
GET  /homework/:tid/edit
POST /homework/:tid/edit
```

| 项 | 值 |
|---|---|
| 处理器 | `HomeworkEditHandler`（homework.ts:147） |
| 方法（源码） | 同 `HomeworkEditHandler`：`get@149`、`postUpdate@193`、`postDelete@242`——**没有普通 `post`** |
| 认证 | 路由无权限；`get`/`postUpdate` 内：非所有者需 `PERM_EDIT_HOMEWORK`，所有者需 `PERM_EDIT_HOMEWORK_SELF` |
| 域权限 | `PERM_EDIT_HOMEWORK` / `PERM_EDIT_HOMEWORK_SELF` |
| 响应 | JSON 对象 / HTML 模板 `homework_edit.html` |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `tid` | `Types.ObjectId`（`@param('tid', Types.ObjectId, true)`） | 作业 ID；省略即「创建」模式 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 无 |

**请求体（`operation=update`）**

与 `homework_create` 的 `postUpdate` 完全一致（同一方法、同一批装饰器，homework.ts:180-193）：`beginAtDate`、`beginAtTime`、`penaltySinceDate`、`penaltySinceTime`、`extensionDays`、`penaltyRules`、`title`、`content`、`pids` 必填；`rated`、`maintainer`、`assign`、`langs` 可选。

编辑分支的额外行为（homework.ts:229-240）：当 `beginAt`/`endAt`/`penaltySince`/`pids` 任一变化时调用 `contest.recalcStatus(domainId, tid)` 重算全部 `tsdoc`。

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `tid` | `ObjectId`（字符串） | 作业 ID |
| `url` | string | 重定向到 `homework_detail` |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| `update` | `postUpdate`（homework.ts:193） | 同 `homework_create` |
| `delete` | `postDelete`（homework.ts:242） | — |

**示例**

```http
POST /d/system/homework/64f1a0b1c2d3e4f5a6b7c8d9/edit
Accept: application/json
Content-Type: application/json

{ "operation": "update", "beginAtDate": "2026-01-01", "beginAtTime": "00:00",
  "penaltySinceDate": "2026-01-08", "penaltySinceTime": "00:00", "extensionDays": 7,
  "penaltyRules": "1: 1", "title": "Homework 1", "content": "Do it.", "pids": "1001" }
```

```json
{ "tid": "64f1a0b1c2d3e4f5a6b7c8d9", "url": "/d/system/homework/64f1a0b1c2d3e4f5a6b7c8d9" }
```

---

#### 作业附件管理 · `homework_files`

```http
GET  /homework/:tid/file
POST /homework/:tid/file
```

| 项 | 值 |
|---|---|
| 处理器 | `HomeworkFilesHandler`（homework.ts:254） |
| 方法（源码） | `prepare@258`、`get@265`、`postUploadFile@280`、`postDeleteFiles@300`——**没有普通 `post`** |
| 认证 | 路由级 `PERM_VIEW_HOMEWORK`；`prepare`（homework.ts:257-262）与 `get`：非所有者需 `PERM_EDIT_HOMEWORK`，所有者需 `PERM_EDIT_HOMEWORK_SELF` |
| 域权限 | `PERM_VIEW_HOMEWORK` + `PERM_EDIT_HOMEWORK(_SELF)` |
| 响应 | JSON 对象 / HTML 模板 `homework_files.html`（pjax 片段 `partials/files.html`） |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `tid` | `Types.ObjectId` | 作业 ID |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 无 |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `filename` | `Types.Filename`（`@post`，仅 body） | 否 | 上传文件名；直接用作存储键 `contest/{domainId}/{tid}/public/{filename}`（**未提供时不会自动生成**，与 `contest_manage` 不同） |
| `files` | `Types.ArrayOf(Types.Filename)`（`@post`，仅 body） | 是（`postDeleteFiles`） | 待删除文件名数组 |
| 文件字段 | multipart `file` | 是（`postUploadFile`） | 上传内容；缺省抛 `ValidationError('file')` |

> 限制来自系统设置：`system.get('limit.contest_files')` 与 `system.get('limit.contest_files_size')`（homework.ts:281-288）。

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `tdoc` | `Tdoc` | 作业文档 |
| `tsdoc` | `ContestStatusDoc` | 当前用户的作业状态（`contest.getStatus(domainId, docId, this.user._id)`） |
| `udoc` | `UserDoc` | 作业所有者 |
| `files` | `FileInfo[]` | 公共附件（已 `sortFiles`） |
| `urlForFile` | function | `(filename) => url('homework_file_download', { tid, filename, type: 'public' })`（JSON 中会被丢弃） |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| `upload_file` | `postUploadFile`（homework.ts:280）：`storage.put` 后把 `{_id,name,size,lastModified,etag}` 追加到 `tdoc.files` | `filename`（可选）、multipart `file` |
| `delete_files` | `postDeleteFiles`（homework.ts:300）：删除存储对象并从 `tdoc.files` 中移除 | `files` |

**示例**

```http
POST /d/system/homework/64f1a0b1c2d3e4f5a6b7c8d9/file
Accept: application/json
Content-Type: application/json

{ "operation": "delete_files", "files": ["statement.pdf"] }
```

```json
{ "url": "/d/system/homework/64f1a0b1c2d3e4f5a6b7c8d9/file" }
```

---

#### 作业附件下载 · `homework_file_download`

```http
GET /homework/:tid/file/:type/:filename
GET /d/:domainId/homework/:tid/file/:type/:filename
```

| 项 | 值 |
|---|---|
| 处理器 | `ContestFileDownloadHandler`（contest.ts:731，由 homework.ts:21 导入复用） |
| 方法（源码） | 仅 `get@736`——**没有 `post`** |
| 认证 | 路由级 `PERM_VIEW_HOMEWORK`；`type=private` 时需「所有者 / `PERM_EDIT_CONTEST` / 已报名」 |
| 域权限 | `PERM_VIEW_HOMEWORK` |
| 响应 | **302 重定向**到签名下载链接，无 JSON 结构 |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `tid` | `Types.ObjectId` | 作业 ID |
| `type` | `Types.Range(['public','private'])` | 附件分区，默认 `private` |
| `filename` | `Types.Filename` | 文件名 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `noDisposition` | `Types.Boolean` | 否（默认 `false`） | 不设置 `Content-Disposition` 文件名 |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `noDisposition` | `Types.Boolean` | 否 | source 为 `all` |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `url` | string | 签名下载链接 |

**注意**：由于复用 `ContestFileDownloadHandler`，其内部判断使用的是比赛语义——`type=private` 时非管理员需已报名（`ContestNotAttendedError`）且比赛处于进行中或已结束；对 `rule === 'homework'` 的文档，`contest.RULES['homework'].hidden === true`，但 `features` 含 `download`，因此**不会**被 `ContestNotFoundError` 拦截（contest.ts:737-739）。

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| — | 只有 `get`；POST → `MethodNotAllowedError` | — |

**示例**

```http
GET /d/system/homework/64f1a0b1c2d3e4f5a6b7c8d9/file/public/statement.pdf
Accept: application/json
```

```json
{ "url": "https://cdn.example.com/contest/system/64f1.../public/statement.pdf?token=..." }
```

---

#### 作业排行榜 · `homework_scoreboard` / `homework_scoreboard_view`

```http
GET  /homework/:tid/scoreboard
GET  /homework/:tid/scoreboard/:view
POST /homework/:tid/scoreboard
```

| 项 | 值 |
|---|---|
| 处理器 | `ContestScoreboardHandler`（contest.ts:890） |
| 方法（源码） | `get@893`、`postUnlock@931`——**没有普通 `post`** |
| 认证 | 匿名可访问路由（`PERM_VIEW_HOMEWORK_SCOREBOARD`） |
| 域权限 | `PERM_VIEW_HOMEWORK_SCOREBOARD` |
| 响应 | JSON 对象（`default` 视图，字段同 `contest_scoreboard`，`page_name = "homework_scoreboard"`）/ 二进制导出 |

**注册源码**（homework.ts:317-319）

```ts
await ctx.inject(['scoreboard'], ({ Route }) => {
    Route('homework_scoreboard', '/homework/:tid/scoreboard', ContestScoreboardHandler, PERM.PERM_VIEW_HOMEWORK_SCOREBOARD);
    Route('homework_scoreboard_view', '/homework/:tid/scoreboard/:view', ContestScoreboardHandler, PERM.PERM_VIEW_HOMEWORK_SCOREBOARD);
});
```

**参数、POST 子操作、视图列表**

与 `contest_scoreboard` / `contest_scoreboard_view` 完全一致（`tid`、`view`、`realtime`；`operation=unlock`）。作业使用 `homework` 赛制（`TEXT: 'Assignment'`，`showScoreboard: () => true`，`showSelfRecord: () => true`，`showRecord: (tdoc, now) => now > tdoc.endAt`），榜单永不隐藏，但封榜（`lockAt`）逻辑仍适用于 `acm` 之外的赛制判定分支。

**示例**

```http
GET /d/system/homework/64f1a0b1c2d3e4f5a6b7c8d9/scoreboard
Accept: application/json
```

```json
{
  "tdoc": { "docId": "64f1...", "rule": "homework" },
  "tsdoc": null,
  "rows": [
    [{ "type": "rank", "value": "Rank" }, { "type": "user", "value": "User" },
     { "type": "total_score", "value": "Score" }, { "type": "time", "value": "Total Time" },
     { "type": "problem", "value": "A", "raw": 1001 }],
    [{ "type": "rank", "value": "1" }, { "type": "user", "value": "alice", "raw": 3 },
     { "type": "string", "value": "100" }, { "type": "time", "value": "00:12:03", "raw": 723 },
     { "type": "record", "score": 100, "value": "100\n00:12:03", "raw": "64f0b2..." }]
  ],
  "udict": { "3": { "_id": 3, "uname": "alice" } },
  "pdict": { "1001": { "docId": 1001, "title": "A+B", "nAccept": 1, "nSubmit": 2 } },
  "page_name": "homework_scoreboard",
  "groups": [],
  "availableViews": { "default": "Default", "ghost": "Ghost", "html": "HTML", "csv": "CSV" }
}
```

---

### 比赛排行榜（Scoreboard）详解

> 本节全部结论来自：`packages/hydrooj/src/handler/contest.ts`（Handler 与视图注册）、`packages/hydrooj/src/model/contest.ts`（赛制与榜单构建）、`packages/hydrooj/src/service/db.ts`（排名算法）、`packages/hydrooj/src/interface.ts`（类型定义）、`packages/ui-default/templates/partials/scoreboard.html` 与 `packages/ui-default/pages/contest_scoreboard.page.ts`（前端消费方式）。

#### 接口与参数

| 项 | 值 |
|---|---|
| 路径 | `/contest/:tid/scoreboard`（`contest_scoreboard`）、`/contest/:tid/scoreboard/:view`（`contest_scoreboard_view`）；作业域为 `/homework/...` |
| 方法 | `GET` 取榜单；`POST` 仅支持 `operation=unlock` |
| 处理器 | `ContestScoreboardHandler`（contest.ts:890-937），`extends ContestDetailBaseHandler`（contest.ts:79）；**全类只有两个方法**：`get@893`（方法体 891-928）、`postUnlock@931`（方法体 930-936） |
| 权限 | 路由级 `PERM_VIEW_CONTEST_SCOREBOARD`（作业为 `PERM_VIEW_HOMEWORK_SCOREBOARD`） |

**路由注册位置**：榜单路由**不在** `apply()` 顶部的 `ctx.Route` 列表里，而是等 `scoreboard` 服务就绪后注册（contest.ts:1108-1110）：

```ts
await ctx.inject(['scoreboard'], ({ Route, scoreboard }) => {
    Route('contest_scoreboard', '/contest/:tid/scoreboard', ContestScoreboardHandler, PERM.PERM_VIEW_CONTEST_SCOREBOARD);
    Route('contest_scoreboard_view', '/contest/:tid/scoreboard/:view', ContestScoreboardHandler, PERM.PERM_VIEW_CONTEST_SCOREBOARD);
    // ...scoreboard.addView('default' | 'ghost' | 'html' | 'csv', ...)
});
```

作业侧（homework.ts:317-319）：

```ts
await ctx.inject(['scoreboard'], ({ Route }) => {
    Route('homework_scoreboard', '/homework/:tid/scoreboard', ContestScoreboardHandler, PERM.PERM_VIEW_HOMEWORK_SCOREBOARD);
    Route('homework_scoreboard_view', '/homework/:tid/scoreboard/:view', ContestScoreboardHandler, PERM.PERM_VIEW_HOMEWORK_SCOREBOARD);
});
```

**`postUnlock` 语义**（contest.ts:930-936）：`POST /contest/:tid/scoreboard`（或 `/homework/:tid/scoreboard`）携带 `operation=unlock`；非本人比赛需 `PERM_EDIT_CONTEST`，比赛未结束抛 `ContestNotEndedError`，随后 `contest.unlockScoreboard(domainId, tid)` 并 `this.back()`。

**参数（源码装饰器，contest.ts:891-893）**

| 参数 | 来源 | 类型 | 必填 | 说明 |
|---|---|---|---|---|
| `tid` | 路径 | `Types.ObjectId` | 是 | 比赛 ID；由 `__prepare`（contest.ts:85）加载 `tdoc`/`tsdoc` |
| `view` | query / body | `Types.String` | 否 | 视图 ID，默认 `'default'`；形参名 `viewId` |
| `realtime` | query / body | `Types.Boolean` | 否 | **仅 `default` 视图声明**（contest.ts:1111）。`true` 时：① 非所有者需 `PERM_VIEW_CONTEST_HIDDEN_SCOREBOARD`；② `config.lockAt` 不设置，即榜单「解封」显示封榜期间提交 |
| `group` / `filter` / `q` / `page` / `limit` | — | — | — | **服务端不接受**。这些是前端行为：`partials/scoreboard.html` 的 `<select class="select filter">` 提供 `all` / `star` / `rank` / 各组 uid 列表，筛选在浏览器端由 `contest_scoreboard.page.ts` 的 `update()` 通过 jQuery 显隐 `<tr>` 实现，并把选择写入 URL hash（`#filter=...`）。没有服务端分页/分页参数 |

**其它服务端参数**由视图声明决定（`ScoreboardView.args`，contest.ts:869-889）：

| 视图 | 声明的 args | 说明 |
|---|---|---|
| `default` | `{ tdoc: 'tdoc', groups: 'groups', realtime: Types.Boolean }` | `tdoc`/`groups` 是「fetcher」而非请求参数：`tdoc` → `this.tdoc`；`groups` → `user.listGroup(domainId, allGroups ? undefined : this.user._id)`（管理员看全部组） |
| `ghost` / `html` / `csv` | `{ tdoc: 'tdoc' }` | 无额外请求参数 |
| `xcpcio`（插件 `packages/scoreboard-xcpcio/index.ts:230-266`） | `tdoc`、`groups`、`json`、`realtime`、`badge`、`banner`、`gold`、`silver`、`bronze` | 第三方插件注册的视图，`supportedRules: ['acm']`；`json=true` 或 `Accept: application/json` 时返回 XCPCIO 格式 JSON；否则渲染 `xcpcio_board.html`，`dataSource` 指向 `/d/{domainId}/contest/{tid}/scoreboard/xcpcio?json=true`，`refreshInterval = isOngoing(tdoc) ? 30000 : 0`（进行中每 30 秒刷新） |

> `ScoreboardService` 定义在 `contest.ts:939-975`（模块声明 `declare module 'cordis'` 在 977-981）：`addView(id, name, args, { display, supportedRules, cacheTime, checker })`（contest.ts:942-964）、`getAvailableViews(rule, handler)`（contest.ts:965-971）、`getView(id)`（contest.ts:972-974）。视图通过 `ctx.effect` 注册，插件卸载时自动移除。
>
> 关于 `scoreboard-xcpcio` 的其它路由：本仓库快照的 `packages/scoreboard-xcpcio/` 中**只注册了 `xcpcio` 视图**（`index.ts:230`），未出现 `contest_resolver_cdp` → `/contest/:tid/resolver-cdp/:token` 的注册代码；若目标部署版本的该插件包含此路由，其形态为 `/contest/:tid/resolver-cdp/:token`（用于向 XCPCIO CDP 导出解题报告）。

**视图解析流程**（contest.ts:893-930）：

1. 隐藏赛制且 `features` 不含 `scoreboard` → `ContestNotFoundError`（`homework` 的 `features = ['scoreboard','download']`，因此作业正常放行）。
2. 非所有者：`canShowScoreboard` 为假 → `ContestScoreboardHiddenError`；未开始 → `ContestNotLiveError`。
3. `ctx.scoreboard.getView(viewId)` 不存在 → `NotFoundError`。
4. 按 `view.args` 逐项解析：值为 `Type`（数组/函数）→ 用 `this.args[key]` 转换并校验，失败抛 `ValidationError(key)`；值为字符串 → 走 fetcher。
5. `await view.display.call(this, args)` 真正输出。

**可用视图列表**由 `ScoreboardService.getAvailableViews(rule, handler)`（contest.ts:965-971）过滤：`supportedRules` 含该赛制（或 `'*'`）且 `checker` 通过者才会出现在响应体 `availableViews` 中。核心内置的 4 个视图（`default`/`ghost`/`html`/`csv`）全部声明 `supportedRules: ['*']`，因此对所有赛制可见；`html` 与 `csv` 还各自调用 `this.limitRate('scoreboard_download', 60, 3)` 限流。

**`default` 视图的响应体**（contest.ts:1111-1132，逐行照录）：

```js
scoreboard.addView('default', 'Default', { tdoc: 'tdoc', groups: 'groups', realtime: Types.Boolean }, {
    async display({ realtime, tdoc, groups }) {
        if (realtime && !this.user.own(tdoc)) this.checkPerm(PERM.PERM_VIEW_CONTEST_HIDDEN_SCOREBOARD);
        const config = { isExport: false, showDisplayName: this.user.hasPerm(PERM.PERM_VIEW_USER_PRIVATE_INFO) };
        if (!realtime && this.tdoc.lockAt && !this.tdoc.unlocked) config.lockAt = this.tdoc.lockAt;
        const [, rows, udict, pdict] = await contest.getScoreboard.call(this, tdoc.domainId, tdoc._id, config);
        const page_name = tdoc.rule === 'homework' ? 'homework_scoreboard' : 'contest_scoreboard';
        const availableViews = scoreboard.getAvailableViews(tdoc.rule, this);
        this.response.body = { tdoc: this.tdoc, tsdoc: this.tsdocAsPublic(), rows, udict, pdict, page_name, groups, availableViews };
        this.response.pjax = 'partials/scoreboard.html';
        this.response.template = 'contest_scoreboard.html';
    },
    supportedRules: ['*'],
});
```

#### 赛制（rule）全表

赛制定义在 `model/contest.ts`，由 `buildContestRule`（model/contest.ts:105-118）构建，导出为 `RULES`（model/contest.ts:820-823）：

```ts
export const RULES: ContestRules = { acm, oi, homework, ioi, ledo, strictioi };
```

`buildContestRule` 会把 `scoreboard` / `scoreboardRow` / `scoreboardHeader` / `stat` / `applyProjection` 五个函数 `bind` 到规则对象上，并允许基于已有规则派生（`buildContestRule(def, baseRule)`）。

| rule 键 | `TEXT` | 定义位置 | 继承自 | `hidden` | `features` | `submitAfterAccept` | `statusSort` |
|---|---|---|---|---|---|---|---|
| `acm` | `XCPC` | model/contest.ts:120-304 | — | 否 | — | `false` | `{ accept: -1, time: 1 }` |
| `oi` | `OI` | model/contest.ts:306-483 | — | 否 | — | `true` | `{ score: -1 }` |
| `ioi` | `IOI` | model/contest.ts:485-495 | `oi` | 否 | — | `false` | 继承 `{ score: -1 }` |
| `strictioi` | `IOI(Strict)` | model/contest.ts:497-574 | `ioi` | 否 | — | `false` | 继承 |
| `ledo` | `Ledo` | model/contest.ts:576-654 | `oi` | 否 | — | `false` | 继承 |
| `homework` | `Assignment` | model/contest.ts:656-818 | — | **是** | `['scoreboard','download']` | `false` | `{ penaltyScore: -1, time: 1 }` |

**可见性函数对比**（决定「什么时候能看到榜单 / 自己的记录 / 全部记录」）

| rule | `showScoreboard(tdoc, now)` | `showSelfRecord(tdoc, now)` | `showRecord(tdoc, now)` |
|---|---|---|---|
| `acm` | `now > tdoc.beginAt` | `() => true` | `now > tdoc.endAt && !isLocked(tdoc)` |
| `oi` | `now > tdoc.endAt && !tdoc.keepScoreboardHidden` | 同左 | 同左 |
| `ioi` | `now > tdoc.beginAt` | `() => true` | `now > tdoc.endAt && !isLocked(tdoc)` |
| `strictioi` | `now > tdoc.endAt && !tdoc.keepScoreboardHidden` | `!tdoc.keepScoreboardHidden \|\| !isDone(tdoc)` | `now > tdoc.endAt && !tdoc.keepScoreboardHidden` |
| `ledo` | `now > tdoc.beginAt` | `() => true` | `now > tdoc.endAt` |
| `homework` | `() => true` | `() => true` | `now > tdoc.endAt` |

这些函数经 `canShowScoreboard` / `canShowSelfRecord` / `canShowRecord`（model/contest.ts:1057-1073）包装，额外允许 `canViewHiddenScoreboard`（model/contest.ts:1051-1055）覆盖：

```ts
export function canViewHiddenScoreboard(this: { user: User }, tdoc: Tdoc) {
    if (this.user.own(tdoc)) return true;                       // 比赛所有者
    if (tdoc.rule === 'homework') return this.user.hasPerm(PERM.PERM_VIEW_HOMEWORK_HIDDEN_SCOREBOARD);
    return this.user.hasPerm(PERM.PERM_VIEW_CONTEST_HIDDEN_SCOREBOARD);
}
```

**脱敏投影 `applyProjection`**（用于 `contest_problemlist` 等页面对非管理员隐藏评测细节）

| rule | 比赛未结束时删除的字段 |
|---|---|
| `acm` | `time`、`memory`、`progress`、`subtasks`、`score`；`testCases = []`、`judgeTexts = []` |
| `oi` | `status`、`memory`、`time`、`score`、`subtasks`；`compilerTexts = []`、`judgeTexts = []`、`testCases = []` |
| `ioi` | 返回原对象（不脱敏） |
| `strictioi` | 继承 `ioi`（不脱敏） |
| `ledo` | 返回原对象 |
| `homework` | **未定义** → `buildContestRule` 的默认 `applyProjection: (_, rdoc) => rdoc` |

#### 榜单数据结构

**入口**：`contest.getScoreboard.call(this, domainId, tid, config)`（model/contest.ts:1075-1088）

```ts
const tdoc = await get(domainId, tid);
if (!canShowScoreboard.call(this, tdoc)) throw new ContestScoreboardHiddenError(tid);
const tsdocsCursor = getMultiStatus(domainId, { docId: tid }).sort(RULES[tdoc.rule].statusSort);
const pdict = await problem.getList(domainId, tdoc.pids, true, true, problem.PROJECTION_CONTEST_DETAIL);
const [rows, udict] = await RULES[tdoc.rule].scoreboard(config, this.translate.bind(this), tdoc, pdict, tsdocsCursor);
await bus.parallel('contest/scoreboard', tdoc, rows, udict, pdict);   // 插件扩展点
return [tdoc, rows, udict, pdict];
```

`ScoreboardConfig`（interface.ts:488-492）：`{ isExport: boolean, showDisplayName: boolean, lockAt?: Date }`。

- `isExport`：导出视图（`html`/`csv`）为 `true`，会把每题展开成「标题 / 罚时」等额外列。
- `showDisplayName`：`this.user.hasPerm(PERM_VIEW_USER_PRIVATE_INFO)`，为真时导出视图追加 `email` / `school` / `displayName` / `studentId` 列。
- `lockAt`：`default` 视图在 `!realtime && tdoc.lockAt && !tdoc.unlocked` 时传入；导出视图**总是**传入 `this.tdoc.lockAt`。

**`rows` 结构**（`ScoreboardRow = ScoreboardNode[] & { raw?: any }`，interface.ts:241-249）

`rows` 即 `getScoreboard` 返回的第二个元素（model/contest.ts:1082-1085），由各赛制的 `scoreboard(config, _, tdoc, pdict, cursor)` 组装：先 `await this.scoreboardHeader(...)` 得到列定义，再把 `db.ranked(cursor, equ)` 的结果逐行交给 `this.scoreboardRow(...)`。因此：

- **`rows[0]` 是表头**，`rows[1..n]` 是数据行。本版本**没有** `cols` 字段（旧版本才有），前端 `partials/scoreboard.html` 直接遍历 `rows[0]` 生成 `<thead>`，并用 `rows[0][i].type` 作为 `<td>` 的 class（`col--{type}`）。
- 单元格是 `ScoreboardNode`（interface.ts:241-248 的**完整定义**，共 6 个可选/必选字段，**没有 `status` 字段**）：

| 字段 | 类型 | 说明 |
|---|---|---|
| `type` | `'string' \| 'rank' \| 'user' \| 'email' \| 'record' \| 'records' \| 'problem' \| 'solved' \| 'time' \| 'total_score'` | 单元格类型，决定前端渲染方式与 CSS class |
| `value` | string | 显示文本（可能含 HTML，如 `<span class="icon icon-check"></span>`；模板用 `nl2br\|safe` 输出） |
| `raw` | any | 原始值：`rank` 无、`user` 为 uid、`problem` 为 pid、`record` 为 rid、`records` 为 `[{value, raw, score}]` 数组 |
| `score` | number | 原始分数（不含赛制加成），用于前端配色 `utils.status.getScoreColor` |
| `style` | string | 内联样式；首次通过高亮为 `background-color: rgb(217, 240, 199);` |
| `hover` | string | tooltip 文本 |

**`udict`**：`getScoreboardUdict`（model/contest.ts:88-103）

```ts
const udict = await UserModel.getListForRender(tdoc.domainId, [...uids, ...memberUids], showDisplayName ? ['displayName'] : []);
if (tdoc.allowTeam) { /* 为每个队伍 tsdoc 写入 udict[uid].teamMembers = 成员 uid 列表 */ }
```

**`pdict`**：`problem.getList(..., PROJECTION_CONTEST_DETAIL)`，投影为 `_id`/`domainId`/`docType`/`docId`/`pid`/`owner`/`title`/`config`/`content`/`html`/`data`/`additional_file`/`reference`/`maintainer`（`model/problem.ts:75-98`）；渲染表头/行时被就地写入 `nAccept`、`nSubmit`（各赛制的 `scoreboardHeader` 初始化、`scoreboardRow` 累加）。

**`tsdoc`**：`tsdocAsPublic()`（contest.ts:107-114）——`attend`、`subscribe`、`startAt`、`displayName`、`members`，以及存在 `duration`/`endAt` 时的 `endAt`。

**前端消费**（`partials/scoreboard.html`）

- 封榜提示：`model.contest.isLocked(tdoc)` 为真时显示提示条（进行中提示「剩余 X 分钟被封榜」）。
- 表头：`problem` 列链接到 `problem_detail` 并显示 `pdict[pid].nAccept/nSubmit`。
- 行：`rank` 列 `value === '0'` 时渲染为 `*`（打星），加 class `rank--unrank`；`user` 列渲染 `udict[uid]`，`teamMembers` 存在时额外列出队员；`record` 列在 `canViewAll`（所有者或 `canShowRecord`）时链接到 `record_detail`。
- 自动刷新（`pages/contest_scoreboard.page.ts`）：`setInterval(updateScoreboard, 180000)`，即比赛进行中每 **180 秒** 通过 `pjax.request({ url: UiContext.scoreboardUrl })` 拉取 `partials/scoreboard.html` 片段刷新。

#### 封榜、打星与实时

**封榜（freeze / lock）**

| 概念 | 字段 / 函数 | 说明 |
|---|---|---|
| 封榜时刻 | `tdoc.lockAt` | 编辑比赛时由 `lock`（分钟）算出：`lockAt = moment(endAt).add(-lock, 'minutes').toDate()`（contest.ts:508）；与 `contestDuration` 互斥 |
| 是否处于封榜中 | `isLocked(tdoc, time = new Date())`（model/contest.ts:78-81） | `tdoc.lockAt < time && !tdoc.unlocked` |
| 解封 | `tdoc.unlocked` | 由 `operation=unlock` 触发 `unlockScoreboard`（model/contest.ts:1031-1037）：置 `unlocked: true` 并 `recalcStatus` 重算全部统计；仅比赛所有者或有 `PERM_EDIT_CONTEST`，且比赛必须已结束（`ContestNotEndedError`） |
| 榜单配置 | `ScoreboardConfig.lockAt` | `default` 视图仅在 `!realtime && tdoc.lockAt && !tdoc.unlocked` 时传入；导出视图总是传入 `tdoc.lockAt` |
| 封榜期间表现 | 各赛制 `stat` / `scoreboardRow` | 见下 |

封榜期内的具体行为：

- **`acm`**：`stat`（model/contest.ts:128-163）中 `lockAt = isLocked(tdoc) ? tdoc.lockAt : null`；对 `rid` 时间戳晚于 `lockAt` 的提交只累计 `npending`（`display[pid].npending`），不覆盖 `display[pid]`；行渲染时 `tsddict = config.lockAt ? tsdoc.display : tsdoc.detail`，未通过格子显示 `-n` 或 `<span style="color:orange">+npending</span>`。
- **`oi`**：`scoreboardRow`（model/contest.ts:368-431）用 `((config.lockAt && isLocked(tdoc, new Date())) ? tsdoc.display : tsdoc.detail)`；`display` 中封榜后的提交同样只记 `npending`（model/contest.ts:311-330）。
- **`strictioi` / `ledo` / `homework`**：`scoreboardRow` 直接取 `tsdoc.detail`，**不做封榜隐藏**（`strictioi` 的 `stat` 也不处理 `lockAt`）。因此这三类赛制的封榜效果仅体现在 `showRecord` / `showScoreboard` 的可见性上，而不是行内数据。
- `nSubmit` / `nAccept` 统计（表头计数）：`acm`（model/contest.ts:210-214）与 `oi`（model/contest.ts:378-382）的 `scoreboardRow` 会跳过 `config.lockAt` 之后的提交（`if (config.lockAt && s.rid.getTimestamp() > config.lockAt) continue;`）；而 `strictioi`、`ledo`、`homework` 的 `scoreboardRow` **没有**该过滤，其 `nSubmit`/`nAccept` 统计包含封榜后的提交。

**打星（unrank / star）**

- 数据字段：`ContestStatusDoc.unrank`（interface.ts 继承自 `ContestStat`）。
- 设置方式：`operation=rank`（`ContestUserHandler.postRank`，contest.ts:793）切换 `unrank`；`operation=add_user` 可带 `unrank=true` 直接打星；`postAttend` 的 `vuid` 分支也接受 `unrank`。
- 排名影响：`db.ranked`（`service/db.ts:168-185`）遇到 `doc.unrank` 直接 `results.push([0, doc])` 且**不计数**（不占用名次），前端把 `rank.value === '0'` 渲染为 `*`。
- 前端筛选：`partials/scoreboard.html` 的 `rank` 选项会隐藏 `.rank--unrank` 所在行；`star` 选项与用户手动收藏（存于浏览器 IndexedDB `scoreboard-star`，见 `contest_scoreboard.page.ts`）配合使用——**「星标」是前端本地收藏，与后端的 `unrank` 是两回事**。

**实时刷新**

- 服务端：`realtime=true` 时 `default` 视图不设置 `config.lockAt`（contest.ts:1118-1120），于是 `acm` 的 `tsddict` 取 `tsdoc.detail`（封榜后的提交也参与展示），且非所有者必须拥有 `PERM_VIEW_CONTEST_HIDDEN_SCOREBOARD`。
- 客户端：`pages/contest_scoreboard.page.ts` 每 180 秒 pjax 刷新一次 `partials/scoreboard.html`。
- 插件侧：`scoreboard-xcpcio` 用 LRU 缓存 + `record/judge` 钩子做增量推送，`refreshInterval = isOngoing(tdoc) ? 30000 : 0`（`packages/scoreboard-xcpcio/index.ts:127-160, 260`）。

#### 排名与统计计算规则

**排名算法**：`db.ranked(cursor, equ)`（`service/db.ts:168-185`）

```ts
async ranked<T extends Record<string, any>>(cursor: T[] | FindCursor<T>, equ: (a: T, b: T) => boolean): Promise<[number, T][]> {
    let last = null; let r = 0; let count = 0; const results = [];
    const docs = cursor instanceof Array ? cursor : await cursor.toArray();
    for (const doc of docs) {
        if ((doc as any).unrank) { results.push([0, doc]); continue; }   // 打星：名次 0，不计数
        count++;
        if (!last || !equ(last, doc)) r = count;                          // 与上一名不等则刷新名次
        last = doc;
        results.push([r, doc]);
    }
    return results;
}
```

游标排序由 `RULES[rule].statusSort` 决定，并列判定 `equ` 由各赛制给出：

| rule | 游标排序 | 并列判定 | 说明 |
|---|---|---|---|
| `acm` | `{ accept: -1, time: 1 }` | `(a.score \|\| 0) === (b.score \|\| 0) && (a.time \|\| 0) === (b.time \|\| 0)` | `acm.stat` **不产出 `score` 字段**，故实际等价于「`time` 相同即并列」 |
| `oi` | `{ score: -1 }` | `(a.score \|\| 0) === (b.score \|\| 0)` | |
| `ioi` | 继承 `oi` | 继承 `oi` | |
| `strictioi` | 继承 `oi` | 继承 `oi` | |
| `ledo` | 继承 `oi` | 继承 `oi` | |
| `homework` | `{ penaltyScore: -1, time: 1 }` | `a.score === b.score` | 注意排序用 `penaltyScore`，并列判定用 `score`（源码如此） |

`RULES[rule].ranked`（model/contest.ts:290、468、815）是接口要求的方法，但 `getScoreboard` 实际调用的是各赛制内部的 `scoreboard`，`ranked` 由外部插件/工具使用。

---

**`acm`（XCPC）——通过题数 / 罚时**

`stat`（model/contest.ts:128-163）：

- 遍历 `journal`（`_getStatusJournal` 已按 `rid` 时间戳升序，model/contest.ts:825-827）。
- 跳过不属于 `tdoc.pids` 的提交；`submitAfterAccept = false` 时，某题已 AC 后的提交不再计入。
- **罚时计数 `naccept[pid]`**：状态不属于 `AC / CompileError / FormatError / Canceled` 时 `naccept[pid]++`（即 CE、格式错误、取消不罚时）。
- `real = floor((rid 时间戳 - tdoc.beginAt) / 1000)`（秒）；`penalty = 20 * 60 * naccept[pid]`（**每次非 AC 罚 20 分钟**）；`time = real + penalty`。
- `detail[pid] = { ...j, naccept, time, real, penalty }`；封榜后的提交只更新 `display[pid].npending`。
- 汇总：`accept = display 中 status === AC 的题目数`；`time = Σ display 中 AC 题目的 time`。
- 返回 `{ accept, time, detail, display }`。

行渲染（model/contest.ts:201-256）：`solved` 列 `value = "{accept}\n{formatSeconds(time)}"`，`hover = formatSeconds(time)`；每题为 `record` 节点：

- 未通过但有提交：`value = "-{naccept}"`
- 通过：`value = "{+naccept 或 ✓图标}\n{formatSeconds(real)}"`，`hover = formatSeconds(time)`，`score = 100`
- 封榜追加：`<span style="color:orange">+{npending}</span>`
- 首 A 高亮：`style = 'background-color: rgb(217, 240, 199);'`，判定条件为「该 rid 时间戳 === `meta.first[pid]`」，`first` 由对 `collStatus` 的聚合（`$min: '$r.v.rid'`，model/contest.ts:262-277）得到。

**`oi`（OI）——总分**

`stat`（model/contest.ts:311-333）：

- `submitAfterAccept = true`（每次提交都覆盖）。
- 无封榜时：`detail[pid]` 取分数更高的提交（`detail[j.pid].score < j.score`）；有封榜时封榜后的提交只累计 `npending`。
- `score = Σ_pid (tdoc.score?.[pid] ?? 100) * (display[pid].score ?? 0) / 100`，即题目权重按百分比缩放（`tdoc.score` 由 `contest_manage` 的 `set_score` 设置）。
- 返回 `{ score, detail, display }`。

行渲染：`total_score` 列 = `tsdoc.score`；每题 `record` 节点 `value = displayScore(pid, score)`（同样按权重缩放），`score = tsddict[pid].score`；比赛结束后若题目级 `psdoc`（`meta.psdict`）的 rid 晚于 `tdoc.endAt` 且与本场 rid 不同，则输出 `type: 'records'`，前后并列展示「场内 / 赛后订正」两个分数（model/contest.ts:395-420）。

**`ioi`（IOI）**

继承 `oi` 的 `stat`/`scoreboardHeader`/`scoreboard`/`ranked`，覆盖：`submitAfterAccept = false`（首次得分即定，不再覆盖）、`showScoreboard/showSelfRecord: now > beginAt`、`showRecord: now > endAt && !isLocked`、`applyProjection` 直接返回原对象（不脱敏）。

**`strictioi`（IOI Strict）——子任务取最大**

`stat`（model/contest.ts:503-517）：

- 按 pid 维护 `subtasks[pid][i]`，对每个子任务取历史最高分：`if (!subtasks[pid][i] || subtasks[pid][i].score < j.subtasks[i].score) subtasks[pid][i] = j.subtasks[i]`。
- `j.score = sumBy(subtasks[pid], 'score')`；`j.status = Math.max(...subtasks[pid].map(i => i.status))`。
- 每题保留分数更高的那次快照：`if (!detail[j.pid] || detail[j.pid].score < j.score) detail[j.pid] = { ...j, subtasks }`。
- `score = Σ (tdoc.score?.[pid] ?? 100) * (detail[pid].score ?? 0) / 100`。
- 返回 `{ score, detail }`（**无 `display`**，因此 `scoreboardRow` 只用 `tsdoc.detail`，天然不受封榜影响）。

行渲染（model/contest.ts:519-568）：`total_score` 列 = `tsdoc.score`；每题 `record` 节点 `value = score * 权重`，`hover = Object.values(subtasks).map(i => "{STATUS_SHORT_TEXTS[i.status]} {i.score}").join(',')`（如 `AC 100,WA 0`）；比赛结束后同样可能输出 `records` 双记录；首 A 高亮判定用 `tsdoc.startAt || tdoc.beginAt` 作为起点。

**`ledo`（Ledo）——指数衰减惩罚**

`stat`（model/contest.ts:583-608）：

- `ntry[pid]` 只在非 `CompileError` / `FormatError` 时累加（有效尝试）。
- 每次提交的惩罚分：`penaltyScore = round(max(0.7, 0.95 ** (ntry[pid] - 1)) * j.score)`，即第 1 次 100%、第 2 次 95%、第 3 次 90.25%…**下限 70%**；CE/格式错误记 0 分。
- `detail[pid]` 取 `penaltyScore` 更高者，并记录 `ntry = max(0, ntry[pid] - 1)`。
- `score = Σ penaltyScore * 权重`，`originalScore = Σ detail.score * 权重`。
- 返回 `{ score, originalScore, detail }`。

行渲染（model/contest.ts:610-653）：`total_score` 的 `hover` 在 `score !== originalScore` 时显示 `Original score: {originalScore}`；每题 `value = penaltyScore * 权重`，`hover = "-{ntry} ({round(max(0.7, 0.95**ntry)*100)}%)"`。

**`homework`（Assignment）——逾期系数**

`stat`（model/contest.ts:666-700）：

- `effective[j.pid] = j`：**后一次提交直接覆盖前一次**（不是取最高分），即「最后一次提交为准」。
- `time(jdoc) = floor((rid 时间戳 - tdoc.beginAt) / 1000)`（秒）。
- `penaltyScore(jdoc)`：

```ts
const rate = (tdoc.score?.[jdoc.pid] || 100) / 100;
const exceedSeconds = Math.floor((rid 时间戳 - tdoc.penaltySince) / 1000);
if (exceedSeconds < 0) return rate * jdoc.score;            // 未逾期：不罚
let coefficient = 1;
const keys = Object.keys(tdoc.penaltyRules).map(Number.parseFloat).sort((a, b) => a - b);
for (const i of keys) { if (i * 3600 <= exceedSeconds) coefficient = tdoc.penaltyRules[i]; else break; }
return rate * jdoc.score * coefficient;
```

  即：`penaltyRules` 的键为**逾期小时数**阈值，取「不超过逾期时长的最大阈值」对应的系数。
- 汇总：`score = Σ score`（原始分）、`penaltyScore = Σ penaltyScore`、`time = Σ time`、`detail = effective`。

行渲染（model/contest.ts:747-800）：

- `total_score`/`string` 列 = `tsdoc.penaltyScore`（导出视图额外输出原始 `score`）。
- `time` 列 = `formatSeconds(tsdoc.time, false)`，`raw = tsdoc.time`。
- 每题 `record` 节点：`score = tsddict[pid].score`；`value` 在「惩罚分 === 原始分」时为 `"{penaltyScore}\n{time}"`，否则为 `"{penaltyScore} / {score}\n{time}"`。
- 该赛制调用 `scoreboardRow` 时**不传 `meta`**，因此没有首 A 高亮，也没有赛后订正双记录。
- `stat` 返回 `detail` 为对象映射（与 `acm`/`oi` 一致），但注意 `detail.push(...)` 的写法只用于构造返回值，最终仍是 `effective` 对象。

#### 榜单 JSON 速查（按赛制的列顺序）

| rule | 表头列顺序（非导出、非 `showDisplayName`） | 数据行附加列 |
|---|---|---|
| `acm` | `rank` → `user` → `solved`（"Solved\nTotal Time"）→ 每题 `problem`（A、B…） | 每题 `record`（`score`/`value`/`hover`/`raw`/`style`） |
| `oi` | `rank` → `user` → `total_score` → 每题 `problem` | 每题 `record` 或 `records` |
| `ioi` | 同 `oi` | 同 `oi` |
| `strictioi` | 同 `oi` | 每题 `record`/`records`，`hover` 为子任务明细 |
| `ledo` | 同 `oi` | 每题 `record`，`hover` 为尝试次数与衰减率 |
| `homework` | `rank`（"Rank"）→ `user` → `total_score`（"Score"）→ `time`（"Total Time"）→ 每题 `problem` | 每题 `record` |

导出视图（`isExport: true`）会把每题列展开为多个 `string` / `time` 列：`acm` 为「标题 + 罚时（分钟）」；`oi` 为「标题」；`homework` 为「标题 + 原始分 + 耗时（秒）」；并且当 `showDisplayName` 为真时统一追加 `email` / `school` / `displayName` / `studentId` 四列。

---

## 5. 训练、讨论、域与系统管理

### 训练（Training）

> 源码：`packages/hydrooj/src/handler/training.ts`（路由注册见 training.ts:309-316）。
> 域前缀规则、`operation` 派发、`@param`/`@query`/`@post`/`@route` 的取值来源见「通用约定」章节；本节的参数名严格取自源码装饰器。
> 全篇 `operation` 一律使用源码中的**下划线写法**：派发时会先拼成 `_xxx` 再转驼峰，因此 `upload_file` → `postUploadFile`、`set_users` → `postSetUsers`。

训练（Training）的 DAG 节点结构（`packages/hydrooj/src/interface.ts:253-258`）：

| 字段 | 类型 | 说明 |
|---|---|---|
| `_id` | number | 节点 ID，DAG 内唯一 |
| `title` | string | 节点标题 |
| `requireNids` | number[] | 前置节点 ID 列表（去重） |
| `pids` | number[] | 该节点包含的题目 ID 列表（去重，至少一个） |

节点状态（由 `packages/hydrooj/src/model/training.ts:65-83` 计算）：

| 字段 | 类型 | 说明 |
|---|---|---|
| `progress` | number | 完成百分比 `Math.floor(100 * doneCount / totalCount)`；无题目时为 `100` |
| `isDone` | boolean | `requireNids` 全部完成且 `pids` 全部通过 |
| `isProgress` | boolean | 前置完成、题目未全通过，但已有一题在进度中 |
| `isOpen` | boolean | 前置完成、题目未全通过，且尚无任何题目处于进度中 |
| `isInvalid` | boolean | `requireNids` 未全部完成（锁定状态） |

#### 训练列表 · `training_main`

```http
GET /training
```

| 项 | 值 |
|---|---|
| 处理器 | `TrainingMainHandler`（training.ts:60），方法 `get`（training.ts:63） |
| 认证 | 匿名 |
| 域权限 | `PERM_VIEW_TRAINING` |
| 响应 | JSON 对象（HTML 模板 `training_main.html`） |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 无 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `page` | `Types.PositiveInt` | 否 | 页码，默认 `1`；每页条数取设置 `pagination.training`（缺省 20） |
| `q` | `Types.String` | 否 | 标题模糊搜索，内部用 `escapeRegExp` 转义后做不区分大小写的正则匹配 |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 该路由未定义 `post` 方法，POST 请求返回 405 `MethodNotAllowedError` |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `tdocs` | `TrainingDoc[]` | 当前页训练列表 |
| `page` | number | 当前页码 |
| `tpcount` | number | 总页数（`paginate` 返回的 numPages） |
| `tsdict` | `Record<string, TrainingStatusDoc>` | 训练 ID 十六进制字符串 → 当前用户状态（需 `PRIV_USER_PROFILE`，否则为 `{}`） |
| `tdict` | `Record<string, TrainingDoc>` | 训练 ID 十六进制字符串 → 训练文档（含已报名但不在当前页的训练） |
| `q` | string | 回显的搜索关键词 |

**示例**

```http
GET /training?page=1&q=dp
Accept: application/json
```

```json
{
  "tdocs": [{ "docId": "64f0...", "title": "动态规划入门", "dag": [], "pin": 0 }],
  "page": 1,
  "tpcount": 3,
  "tsdict": { "64f0...": { "docId": "64f0...", "enroll": 1, "doneNids": [1], "done": false } },
  "tdict": { "64f0...": { "docId": "64f0...", "title": "动态规划入门" } },
  "q": "dp"
}
```

#### 训练详情 · `training_detail`

```http
GET /training/:tid
POST /training/:tid
```

| 项 | 值 |
|---|---|
| 处理器 | `TrainingDetailHandler`（training.ts:99），方法 `get`（training.ts:102） |
| 认证 | 匿名（`uid` 对比需登录） |
| 域权限 | `PERM_VIEW_TRAINING` |
| 响应 | JSON 对象（HTML 模板 `training_detail.html`，pjax 片段 `partials/training_detail.html`） |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `tid` | `Types.ObjectId` | 训练 ID，必填 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `uid` | `Types.PositiveInt` | 否 | 查看他人进度。仅当用户有 `PRIV_USER_PROFILE` 且设置 `training.enrolled-users` 为真时生效；此时 `uid` 默认取当前用户，且 `shouldCompare = (uid !== this.user._id)` |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 无普通 `post` 方法；POST 必须带 `operation`（见下），否则 405 |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `tdoc` | `TrainingDoc` | 训练文档；`description` 中的 `file://` 会被改写为 `./:tid/file/` |
| `tsdoc` | `TrainingStatusDoc` | 当前用户的训练状态，由 `training.setStatus` 写入 `doneNids`/`donePids`/`done` |
| `pids` | number[] | DAG 中去重后的全部题目 ID |
| `pdict` | `Record<number, ProblemDoc>` | 题目 ID → 题目文档（受隐藏题目可见性限制） |
| `psdict` | `Record<string, ProblemStatusDoc>` | 题目 ID → 被查看用户的提交状态（`uid` 对应用户） |
| `selfPsdict` | `Record<string, ProblemStatusDoc>` | 题目 ID → 当前用户自己的提交状态（仅 `shouldCompare` 时非空） |
| `ndict` | `Record<number, TrainingNode>` | 节点 ID → 节点定义 |
| `nsdict` | `Record<number, NodeStatus>` | 节点 ID → 节点状态（`progress`/`isDone`/`isProgress`/`isOpen`/`isInvalid`） |
| `udoc` | `User` | 训练创建者 |
| `udict` | `Record<number, User>` | 已报名用户（最多 500 人，需 `training.enrolled-users`） |
| `groups` | `GroupDoc[]` | 域内用户组，仅当用户有 `PERM_EDIT_DOMAIN` 时非空 |
| `missing` | number[] | DAG 中引用但已不存在的题目 ID |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| `enroll` | 报名训练。方法 `postEnroll`（training.ts:170）；先 `checkPriv(PRIV.PRIV_USER_PROFILE)`，重复报名抛 `TrainingAlreadyEnrollError` | `tid`（路径参数） |
| `delete` | 删除训练并清理 `training/<domainId>/<tid>/*` 附件。方法 `postDelete`（training.ts:178）；非作者需 `PERM_EDIT_TRAINING`；成功后 302 跳转 `training_main` | `tid`（路径参数） |

**示例**

```http
GET /d/system/training/64f0a1b2c3d4e5f60718293a
Accept: application/json
```

```json
{
  "tdoc": { "docId": "64f0a1b2c3d4e5f60718293a", "title": "动态规划入门", "dag": [{ "_id": 1, "title": "线性 DP", "requireNids": [], "pids": [1000, 1001] }] },
  "tsdoc": { "docId": "64f0a1b2c3d4e5f60718293a", "enroll": 1, "doneNids": [], "donePids": [1000], "done": false },
  "pids": [1000, 1001],
  "ndict": { "1": { "_id": 1, "title": "线性 DP", "requireNids": [], "pids": [1000, 1001] } },
  "nsdict": { "1": { "progress": 50, "isDone": false, "isProgress": true, "isOpen": false, "isInvalid": false } },
  "missing": []
}
```

```http
POST /d/system/training/64f0a1b2c3d4e5f60718293a
Content-Type: application/json

{ "operation": "enroll" }
```

#### 创建 / 编辑训练 · `training_create` / `training_edit`

```http
GET /training/create
POST /training/create
GET /training/:tid/edit
POST /training/:tid/edit
```

| 项 | 值 |
|---|---|
| 处理器 | `TrainingEditHandler`（training.ts:189）；`prepare`（training.ts:193）、`get`（training.ts:201）、`post`（training.ts:216） |
| 认证 | 匿名（由域权限决定） |
| 域权限 | 路由本身未注册权限，全部在 `prepare` 中检查：有 `tid` 且非作者 → `PERM_EDIT_TRAINING`；有 `tid` 且为作者 → `PERM_EDIT_TRAINING_SELF`；无 `tid` → `PERM_CREATE_TRAINING` |
| 响应 | `get`：JSON 对象（HTML 模板 `training_edit.html`）；`post`：JSON 对象 + 302 跳转 `training_detail` |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `tid` | `Types.ObjectId` | 可选（`@param('tid', Types.ObjectId, true)`）。存在即为编辑，缺失即为创建 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 无查询参数（`tid` 由路径参数提供） |

**请求体（`POST`）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `tid` | `Types.ObjectId` | 否 | 同路径参数，`@param` 会从路径/查询/请求体合并取值 |
| `title` | `Types.Title` | 是 | 标题，长度 1–64 且去除首尾空白后非空 |
| `content` | `Types.Content` | 是 | 训练说明（Markdown），长度 < 65536 |
| `dag` | `Types.Content` | 是 | DAG 的 JSON 字符串，形如 `[{"_id":1,"title":"节点","requireNids":[],"pids":[1000]}]` |
| `pin` | `Types.UnsignedInt` | 是 | 置顶顺序，默认 `0`；若置顶状态发生变化（`!!tdoc.pin !== !!pin`）需 `PERM_PIN_TRAINING` |
| `description` | `Types.Content` | 是 | 简介 |

`dag` 的校验规则（`_parseDagJson`，training.ts:18-57）：

| 规则 | 失败结果 |
|---|---|
| 必须是 JSON 数组 | `ValidationError('dag')` |
| 至少一个节点 | `ValidationError('dag', 'must have at least one node')` |
| 节点 `_id` 必须唯一 | `ValidationError('dag', '_id must be unique')` |
| 每个节点必须有 `_id`、`title` | `ValidationError('dag', ...)` |
| `requireNids`、`pids` 必须是数组 | `ValidationError('dag', ...)` |
| `pids` 至少一个题目 | `ValidationError('dag', 'each node must contain at lease one problem')` |
| `requireNids` 引用的节点必须存在 | `ValidationError('dag', 'required nid X not found')` |
| `pids` 中的题目必须存在 | `ProblemNotFoundError` |
| 所有题目 ID 去重后非空 | `ValidationError('dag', 'Please specify at least one problem')` |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `page_name` | string | 仅 `get`：`'training_edit'`（有 `tid`）或 `'training_create'`（无 `tid`） |
| `tdoc` | `TrainingDoc` | 仅 `get` 且存在 `tid` |
| `dag` | string | 仅 `get` 且存在 `tid`：`JSON.stringify(tdoc.dag, null, 2)` |
| `tid` | `ObjectId` | 仅 `post`：创建或编辑的训练 ID（创建时为新增 ID） |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| — | 该 Handler 只实现普通 `post`，无 `postXxx` 子操作 | — |

**示例**

```http
POST /d/system/training/create
Content-Type: application/json
Accept: application/json

{
  "title": "动态规划入门",
  "content": "# 说明",
  "description": "入门训练",
  "pin": 0,
  "dag": "[{\"_id\":1,\"title\":\"线性 DP\",\"requireNids\":[],\"pids\":[1000,1001]}]"
}
```

```json
{ "tid": "64f0a1b2c3d4e5f60718293a", "url": "/d/system/training/64f0a1b2c3d4e5f60718293a" }
```

> `post` 设置了 `this.response.redirect`，框架会把跳转地址写入 `body.url`（`framework/framework/base.ts:64-67`）。

#### 训练附件管理 · `training_files`

```http
GET /training/:tid/file
POST /training/:tid/file
```

| 项 | 值 |
|---|---|
| 处理器 | `TrainingFilesHandler`（training.ts:237）；`prepare`（training.ts:241）、`get`（training.ts:248）、`postUploadFile`（training.ts:263）、`postDeleteFiles`（training.ts:283） |
| 认证 | 匿名（由域权限决定） |
| 域权限 | 路由权限 `PERM_VIEW_TRAINING`；`prepare` 中：非作者 → `PERM_EDIT_TRAINING`，作者 → `PERM_EDIT_TRAINING_SELF`；`get` 中非作者再校验一次 `PERM_EDIT_TRAINING` |
| 响应 | `get`：JSON 对象（HTML 模板 `training_files.html`，pjax 片段 `partials/files.html`）；两个子操作：JSON 对象 + 302 回跳 Referer |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `tid` | `Types.ObjectId` | 训练 ID，必填 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `pjax` | `Types.Boolean` | 否 | 由框架读取（`args.pjax`）；为真时返回 `{ "fragments": [{ "html": "..." }] }` |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `operation` | string | 是 | `upload_file` 或 `delete_files`（该 Handler 没有普通 `post`） |
| `filename` | `Types.Filename` | 否（`postUploadFile`，`@post('filename', Types.Filename, true)`） | 附件名，只从请求体取值；长度 1–255，且 `sanitize-filename` 后不变 |
| `file` | 文件（multipart） | 是（`postUploadFile`） | 通过 `this.request.files.file` 读取，缺失抛 `ValidationError('file')` |
| `files` | `Types.ArrayOf(Types.Filename)` | 是（`postDeleteFiles`） | 待删除的附件名数组，`@post` 只从请求体取值 |

上传限制（`postUploadFile`）：数量上限 `system.get('limit.contest_files')`，超限抛 `FileLimitExceededError('count')`；总大小上限 `system.get('limit.contest_files_size')`，超限抛 `FileLimitExceededError('size')`。存储路径为 `training/<domainId>/<tid>/<filename>`。

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `tdoc` | `TrainingDoc` | 训练文档 |
| `tsdoc` | `TrainingStatusDoc` | 当前用户的训练状态 |
| `udoc` | `User` | 训练创建者 |
| `files` | `FileInfo[]` | `sortFiles(tdoc.files)` 排序后的附件元数据 |
| `urlForFile` | function | `(filename) => url('training_file_download', { tid, filename })`；JSON 序列化时被丢弃 |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| `upload_file` | 上传附件并写回 `tdoc.files`，随后 `back()` | `filename`（请求体）、`file`（multipart） |
| `delete_files` | 批量删除附件（同时删除存储对象）并 `back()` | `files`（请求体，文件名数组） |

> 无 `operation` 的 POST 会抛 405 `MethodNotAllowedError`（该 Handler 未实现普通 `post`）。

**示例**

```http
POST /d/system/training/64f0a1b2c3d4e5f60718293a/file
Content-Type: multipart/form-data; boundary=----X

------X
Content-Disposition: form-data; name="operation"

delete_files
------X
Content-Disposition: form-data; name="files"

a.zip
------X--
```

```json
{ "url": "/d/system/training/64f0a1b2c3d4e5f60718293a/file" }
```

#### 训练附件下载 · `training_file_download`

```http
GET /training/:tid/file/:filename
```

| 项 | 值 |
|---|---|
| 处理器 | `TrainingFileDownloadHandler`（training.ts:291），方法 `get`（training.ts:295） |
| 认证 | 匿名 |
| 域权限 | `PERM_VIEW_TRAINING` |
| 响应 | 302 重定向到签名下载链接（`storage.signDownloadLink`），并写入 `Cache-Control: public` |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `tid` | `Types.ObjectId` | 训练 ID，必填 |
| `filename` | `Types.Filename` | 附件名，必填 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `noDisposition` | `Types.Boolean` | 否 | `@param`（合并取值）；为真时不带 `filename` 下载名（内联预览），默认 `false` |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 仅 GET |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `url` | string | 302 跳转地址（`response.redirect` 被框架写入 `body.url`） |

同时记录操作日志 `oplog.log(this, 'download.file.training', { target, size })`。

**示例**

```http
GET /d/system/training/64f0a1b2c3d4e5f60718293a/file/data.zip
```

```json
{ "url": "/storage?target=training%2Fsystem%2F64f0...%2Fdata.zip&expire=1730000000000&secret=..." }
```

---

### 讨论（Discussion）

> 源码：`packages/hydrooj/src/handler/discussion.ts`（路由注册见 discussion.ts:426-434）。

**父节点类型映射**（`typeMapper`，discussion.ts:16-22）：

| 路径 `:type` | 对应 `document` 常量 | 反查显示名（`discussion.typeDisplay`） |
|---|---|---|
| `problem` | `TYPE_PROBLEM` | `problem` |
| `contest` | `TYPE_CONTEST` | `contest` |
| `homework` | `TYPE_CONTEST` | `contest` |
| `training` | `TYPE_TRAINING` | `training` |
| `node` | `TYPE_DISCUSSION_NODE` | `node` |

**公共准备钩子** `DiscussionHandler._prepare`（discussion.ts:35，装饰器 discussion.ts:29-33）：

| 装饰器 | 参数名 | 类型 | 必填 | 说明 |
|---|---|---|---|---|
| `@param` | `type` | `Types.Range(Object.keys(typeMapper))` | 否 | 父节点类型 |
| `@param` | `name` | `Types.String` | 否 | 父节点 ID |
| `@param` | `did` | `Types.ObjectId` | 否 | 讨论 ID；存在时加载 `ddoc`（不存在抛 `DiscussionNotFoundError`），并用 `ddoc.parentType`/`ddoc.parentId` 覆盖 `type`/`name` |
| `@param` | `drid` | `Types.ObjectId` | 否 | 回复 ID；加载 `drdoc`，若其 `parentId` 与 `ddoc._id` 不符抛 `DocumentNotFoundError` |
| `@param` | `drrid` | `Types.ObjectId` | 否 | 楼中楼回复 ID；与 `drid` 一起加载 `drdoc`/`drrdoc` |

`_prepare` 首先 `checkPerm(PERM.PERM_VIEW_DISCUSSION)`，随后 `discussion.getVnode(...)` 取父节点，并用 `discussion.checkVNodeVisibility` 校验可见性（隐藏题目需 `PERM_VIEW_PROBLEM_HIDDEN` 或本人；比赛/训练的 `assign` 需与用户所在分组相交），失败抛 `DiscussionNodeNotFoundError`。

#### 讨论列表 · `discussion_main`

```http
GET /discuss
```

| 项 | 值 |
|---|---|
| 处理器 | `DiscussionMainHandler`（discussion.ts:69），方法 `get`（discussion.ts:72） |
| 认证 | 匿名 |
| 域权限 | `PERM_VIEW_DISCUSSION` |
| 响应 | JSON 对象（HTML 模板 `discussion_main_or_node.html`） |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 无 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `page` | `Types.PositiveInt` | 否 | 页码，默认 `1`；每页条数取设置 `pagination.discussion` |
| `all` | `Types.Boolean` | 否 | 默认 `false`；仅当用户有 `PERM_MOD_BADGE` 时才生效（否则被强制为 `false`），为真时包含隐藏讨论 |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 未实现 `post`，POST 返回 405 |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `ddocs` | `DiscussionDoc[]` | 当前页讨论（按 `pin`、`docId` 倒序） |
| `dpcount` | number | 总页数 |
| `udict` | `Record<number, User>` | 讨论作者 |
| `page` | number | 当前页码 |
| `page_name` | string | 固定 `'discussion_main'` |
| `vndict` | `Record<number, Record<string, VNode>>` | 父节点类型 → 父节点 ID → 节点信息 |
| `vnode` | object | 固定 `{}` |
| `vnodes` | `VNode[]` | 全部分区节点（`discussion.getNodes`） |

**示例**

```http
GET /discuss?page=1
Accept: application/json
```

```json
{
  "ddocs": [{ "docId": "64f0...", "title": "如何评价 Hydro？", "parentType": 10, "parentId": "qa", "nReply": 3, "views": 42 }],
  "dpcount": 2,
  "page": 1,
  "page_name": "discussion_main",
  "vnode": {},
  "vnodes": [{ "_id": "qa", "title": "问答" }]
}
```

#### 讨论分区 · `discussion_node`

```http
GET /discuss/:type/:name
```

| 项 | 值 |
|---|---|
| 处理器 | `DiscussionNodeHandler`（discussion.ts:93），方法 `get`（discussion.ts:97） |
| 认证 | 匿名 |
| 域权限 | `PERM_VIEW_DISCUSSION`（由 `_prepare` 检查） |
| 响应 | JSON 对象（HTML 模板 `discussion_main_or_node.html`） |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `type` | `Types.Range(['problem','contest','node','training','homework'])` | 父节点类型，必填 |
| `name` | `Types.String` | 父节点 ID，必填；内部按「合法 ObjectId → 安全整数 → 字符串」顺序转换 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `page` | `Types.PositiveInt` | 否 | 页码，默认 `1` |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 未实现 `post`，POST 返回 405 |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `ddocs` | `DiscussionDoc[]` | 该父节点下的讨论；无 `PERM_EDIT_DISCUSSION` 且非节点所有者时只返回 `hidden: false` |
| `dpcount` | number | 总页数 |
| `udict` | `Record<number, User>` | 讨论作者 + 节点所有者 |
| `page` | number | 当前页码 |
| `vndict` | `Record<number, Record<string, VNode>>` | `{ [typeMapper[type]]: { [name]: vnode } }` |
| `vnode` | `VNode` | 当前父节点（题目/比赛/训练/分区） |
| `page_name` | string | 固定 `'discussion_node'` |
| `vnodes` | `VNode[]` | 全部分区节点 |

**示例**

```http
GET /discuss/node/qa?page=1
Accept: application/json
```

```json
{
  "ddocs": [{ "docId": "64f0...", "title": "提问：如何配置评测机？", "parentType": 10, "parentId": "qa" }],
  "dpcount": 1,
  "page": 1,
  "vndict": { "10": { "qa": { "_id": "qa", "title": "问答", "type": 10 } } },
  "vnode": { "_id": "qa", "title": "问答", "type": 10 },
  "page_name": "discussion_node"
}
```

#### 发起讨论 · `discussion_create`

```http
GET /discuss/:type/:name/create
POST /discuss/:type/:name/create
```

| 项 | 值 |
|---|---|
| 处理器 | `DiscussionCreateHandler`（discussion.ts:129）；`get`（discussion.ts:130）、`post`（discussion.ts:146） |
| 认证 | 需要登录（路由级 `PRIV.PRIV_USER_PROFILE`） |
| 域权限 | `PERM_CREATE_DISCUSSION`（路由级）；`_prepare` 内另需 `PERM_VIEW_DISCUSSION` |
| 响应 | `get`：JSON 对象（HTML 模板 `discussion_create.html`）；`post`：JSON 对象 + 302 跳转 `discussion_detail` |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `type` | `Types.Range(['problem','contest','node','training','homework'])` | 必填 |
| `name` | `Types.String` | 必填 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 无 |

**请求体（`POST`）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `type` | `Types.Range([...])` | 是 | `@param` 合并取值（路径参数即可满足） |
| `title` | `Types.Title` | 是 | 标题，1–64 字符 |
| `content` | `Types.Content` | 是 | 正文（Markdown），长度 < 65536 |
| `highlight` | `Types.Boolean` | 否 | 默认 `false`；为真需 `PERM_HIGHLIGHT_DISCUSSION` |
| `pin` | `Types.Boolean` | 否 | 默认 `false`；为真需 `PERM_PIN_DISCUSSION` |

`post` 会调用 `limitRate('add_discussion', 3600, 60)`（每 IP/用户每小时 60 次），并继承父节点 `hidden` 属性（`hidden = this.vnode.hidden ?? false`）。

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `path` | `[string, string, object?, boolean?][]` | 仅 `get`：面包屑（Hydro → 讨论 → 节点 → 发起讨论） |
| `vnode` | `VNode` | 仅 `get`：当前父节点 |
| `did` | `ObjectId` | 仅 `post`：新建讨论 ID |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| — | 只实现普通 `post` | — |

**示例**

```http
POST /d/system/discuss/node/qa/create
Content-Type: application/json

{ "title": "提问", "content": "正文", "highlight": false, "pin": false }
```

```json
{ "did": "64f0b2c3d4e5f60718293a4b", "url": "/d/system/discuss/64f0b2c3d4e5f60718293a4b" }
```

#### 讨论详情 · `discussion_detail`

```http
GET /discuss/:did
POST /discuss/:did
```

| 项 | 值 |
|---|---|
| 处理器 | `DiscussionDetailHandler`（discussion.ts:163）；`get`（discussion.ts:166）、`post`（discussion.ts:207）及全部 `postXxx` 子操作 |
| 认证 | `get` 匿名（`dsdoc` 需登录）；`post` 需登录（`checkPriv(PRIV.PRIV_USER_PROFILE)`） |
| 域权限 | `PERM_VIEW_DISCUSSION`（由 `_prepare` 检查） |
| 响应 | `get`：JSON 对象（HTML 模板 `discussion_detail.html`）；子操作：JSON 对象 + 302 回跳 Referer |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `did` | `Types.ObjectId` | 讨论 ID，必填 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `page` | `Types.PositiveInt` | 否 | 回复分页页码，默认 `1`；每页条数取设置 `pagination.reply` |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `operation` | string | 否 | 子操作派发键；无 `operation` 时调用 `post()`（仅校验登录，响应体为空对象 `{}`） |

**响应字段（`Accept: application/json`，`GET`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `path` | `[string, string, object?, boolean?][]` | 面包屑（Hydro → 讨论 → 节点 → 当前讨论） |
| `ddoc` | `DiscussionDoc` | 讨论文档 |
| `dsdoc` | `DiscussionStatusDoc \| null` | 当前用户对讨论的状态（含 `star`、`react`、`view`）；未登录为 `null` |
| `drdocs` | `DiscussionReplyDoc[]` | 当前页回复（按 `_id` 倒序，含 `reply` 楼中楼数组） |
| `page` | number | 当前页码 |
| `pcount` | number | 回复总页数 |
| `drcount` | number | 回复总数 |
| `udict` | `Record<number, User>` | 节点所有者、讨论作者、回复作者、楼中楼作者 |
| `vnode` | `VNode` | 父节点 |
| `reactions` | `Record<string, Record<string, number>>` | 讨论 ID 十六进制串与回复 ID 十六进制串 → 表情 → 数量（含当前用户的反应） |

`get` 副作用：当 `dsdoc.view` 为空且用户已登录时，讨论 `views + 1` 并写入 `view: true`。

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| `set_lock` | 锁定/解锁讨论。方法 `postSetLock`（discussion.ts:213）；非作者需 `PERM_LOCK_DISCUSSION`；`back()` | `did`、`lock`（`Types.Boolean`） |
| `reaction` | 表情回应。方法 `postReaction`（discussion.ts:223）；需 `PERM_ADD_REACTION`；返回 `{ doc, sdoc }` 并 `back()` | `nodeType`（`Types.Range(['did','drid'])`）、`id`（`Types.ObjectId`）、`emoji`（`Types.Emoji`）、`reverse`（`Types.Boolean`，默认 `false`） |
| `reply` | 发表回复。方法 `postReply`（discussion.ts:233）；需 `PERM_REPLY_DISCUSSION`；讨论锁定时抛 `DiscussionLockedError`；`limitRate('add_discussion', 3600, 60)`；向正文中 `@[](/user/<uid>)` 提及的用户发站内信；`back({ drid })` | `did`、`content`（`Types.Content`） |
| `tail_reply` | 楼中楼回复。方法 `postTailReply`（discussion.ts:250）；需 `PERM_REPLY_DISCUSSION`；同上限流与提及通知；`back()` | `drid`（`Types.ObjectId`）、`content` |
| `edit_reply` | 编辑回复。方法 `postEditReply`（discussion.ts:267）；需 `PERM_EDIT_DISCUSSION_REPLY_SELF` 且必须为作者，否则 `PermissionError`；`back()` | `drid`、`content` |
| `delete_reply` | 删除回复。方法 `postDeleteReply`（discussion.ts:278）；作者需 `PERM_DELETE_DISCUSSION_REPLY_SELF`，讨论作者需 `PERM_DELETE_DISCUSSION_REPLY_SELF_DISCUSSION`，否则需 `PERM_DELETE_DISCUSSION_REPLY`；非本人操作会发站内信；`back()` | `drid` |
| `edit_tail_reply` | 编辑楼中楼。方法 `postEditTailReply`（discussion.ts:306）；需 `PERM_EDIT_DISCUSSION_REPLY_SELF` 且必须为作者；`back()` | `drid`、`drrid`（`Types.ObjectId`）、`content` |
| `delete_tail_reply` | 删除楼中楼。方法 `postDeleteTailReply`（discussion.ts:318）；本人需 `PERM_DELETE_DISCUSSION_REPLY_SELF`，否则需 `PERM_DELETE_DISCUSSION_REPLY`；`back()` | `drid`、`drrid` |
| `star` | 收藏/取消收藏。方法 `postStar`（discussion.ts:342）；`back({ star })` | `did`、`star`（`Types.Boolean`，默认 `false`） |

**示例**

```http
GET /d/system/discuss/64f0b2c3d4e5f60718293a4b?page=1
Accept: application/json
```

```json
{
  "ddoc": { "docId": "64f0b2c3d4e5f60718293a4b", "title": "提问", "content": "正文", "nReply": 1, "views": 7, "lock": false },
  "dsdoc": { "star": true, "view": true, "react": { "👍": 1 } },
  "drdocs": [{ "docId": "64f0...", "owner": 2, "content": "回复内容", "reply": [] }],
  "page": 1,
  "pcount": 1,
  "drcount": 1,
  "reactions": { "64f0b2c3d4e5f60718293a4b": { "👍": 1 } },
  "vnode": { "_id": "qa", "title": "问答", "type": 10 }
}
```

```http
POST /d/system/discuss/64f0b2c3d4e5f60718293a4b
Content-Type: application/json

{ "operation": "reply", "content": "同意" }
```

```json
{ "drid": "64f0c3d4e5f60718293a4b5c", "url": "/d/system/discuss/64f0b2c3d4e5f60718293a4b" }
```

#### 编辑 / 删除讨论 · `discussion_edit`

```http
GET /discuss/:did/edit
POST /discuss/:did/edit
```

| 项 | 值 |
|---|---|
| 处理器 | `DiscussionEditHandler`（discussion.ts:371）；`get`（discussion.ts:372）、`postUpdate`（discussion.ts:382）、`postDelete`（discussion.ts:402） |
| 认证 | 匿名（由域权限决定） |
| 域权限 | `PERM_VIEW_DISCUSSION`（`_prepare`）；`postUpdate`：非作者 `PERM_EDIT_DISCUSSION`，作者 `PERM_EDIT_DISCUSSION_SELF`；`postDelete`：非作者 `PERM_DELETE_DISCUSSION`，作者 `PERM_DELETE_DISCUSSION_SELF` |
| 响应 | `get`：JSON 对象（HTML 模板 `discussion_edit.html`）；子操作：JSON 对象 + 302 跳转 |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `did` | `Types.ObjectId` | 讨论 ID，必填 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 无 |

**请求体（`postUpdate`）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `operation` | string | 是 | `update`（该 Handler 没有普通 `post`） |
| `did` | `Types.ObjectId` | 是 | 合并取值（路径参数即可满足） |
| `title` | `Types.Title` | 是 | 标题 |
| `content` | `Types.Content` | 是 | 正文 |
| `highlight` | `Types.Boolean` | 否 | 默认 `false`；无 `PERM_HIGHLIGHT_DISCUSSION` 时保留原值 |
| `pin` | `Types.Boolean` | 否 | 默认 `false`；无 `PERM_PIN_DISCUSSION` 时保留原值 |

**请求体（`postDelete`）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `operation` | string | 是 | `delete` |
| `did` | `Types.ObjectId` | 是 | 合并取值（路径参数即可满足） |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `ddoc` | `DiscussionDoc` | 仅 `get` |
| `did` | `ObjectId` | 仅 `postUpdate`；并 302 跳转 `discussion_detail` |
| `type` | number | 仅 `postDelete`：父节点类型常量 |
| `parent` | `ObjectId \| number \| string` | 仅 `postDelete`：父节点 ID；并 302 跳转 `discussion_node` |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| `update` | 保存修改（`postUpdate`），写操作日志 `discussion.edit` | `title`、`content`、`highlight`、`pin` |
| `delete` | 删除讨论（`postDelete`），写操作日志 `discussion.delete`，非本人操作会发站内信 | — |

> 无 `operation` 的 POST 返回 405（未实现普通 `post`）。

**示例**

```http
POST /d/system/discuss/64f0b2c3d4e5f60718293a4b/edit
Content-Type: application/json

{ "operation": "delete" }
```

```json
{ "type": 10, "parent": "qa", "url": "/d/system/discuss/node/qa" }
```

#### 讨论原文 · `discussion_raw` / `discussion_reply_raw` / `discussion_tail_reply_raw`

```http
GET /discuss/:did/raw
GET /discuss/:did/:drid/raw
GET /discuss/:did/:drid/:drrid/raw
```

| 项 | 值 |
|---|---|
| 处理器 | `DiscussionRawHandler`（discussion.ts:348），方法 `get`（discussion.ts:354），三个路由共用 |
| 认证 | 匿名 |
| 域权限 | `PERM_VIEW_DISCUSSION`（由 `_prepare` 检查） |
| 响应 | `text/markdown` 纯文本（`all=true` 时为 JSON 对象） |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `did` | `Types.ObjectId` | 可选；讨论 ID |
| `drid` | `Types.ObjectId` | 可选；回复 ID（路由 `discussion_reply_raw`、`discussion_tail_reply_raw` 提供） |
| `drrid` | `Types.ObjectId` | 可选；楼中楼 ID（路由 `discussion_tail_reply_raw` 提供） |

取值优先级：`drrid || drid || did`。

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `time` | `Types.UnsignedInt` | 否 | 装饰器名为 `time`，对应方法形参 `ts`；指定时按 `{ time: new Date(ts) }` 取历史版本，取不到抛 `DiscussionNotFoundError` |
| `all` | `Types.Boolean` | 否 | 默认 `false`；为真时返回该文档的全部历史版本 |
| `render` | `Types.Boolean` | 否 | 由 `ui-default` 的 `handler/after/DiscussionRaw` 钩子读取；为真时把 Markdown 渲染为 HTML 并改 `Content-Type` 为 `text/html`（`packages/ui-default/index.ts:205-210`） |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 仅 GET |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `history` | `DiscussionHistoryDoc[]` | 仅 `all=true`：`{ title, content, docId, uid, time }` 数组，按 `time` 倒序 |
| — | string | `all` 为假时响应体是纯 Markdown 文本（`this.response.type = 'text/markdown'`），无 JSON 结构 |

**示例**

```http
GET /d/system/discuss/64f0b2c3d4e5f60718293a4b/raw
```

```
正文
```

```http
GET /d/system/discuss/64f0b2c3d4e5f60718293a4b/raw?all=true
Accept: application/json
```

```json
{ "history": [{ "title": "提问", "content": "正文", "docId": "64f0...", "uid": 2, "time": "2026-09-17T04:00:00.000Z" }] }
```

---

### 排名与域管理（Domain）

> 源码：`packages/hydrooj/src/handler/domain.ts`（路由注册见 domain.ts:479-492）。
> `ManageHandler.prepare`（domain.ts:43）会先 `checkPerm(PERM.PERM_EDIT_DOMAIN)` 并把 `this.domain = await domain.get(domainId)`；`domain_dashboard`、`domain_edit`、`domain_user`、`domain_permission`、`domain_role`、`domain_group`、`domain_join_applications` 均继承该准备钩子（因此**都需要 `PERM_EDIT_DOMAIN`**）。
> `@requireSudo`（`packages/hydrooj/src/service/server.ts:55-73`）要求会话在 1 小时内通过过 sudo 校验（`session.sudo`），否则 302 跳转 `user_sudo`。

#### 域内排名 · `ranking`

```http
GET /ranking
```

| 项 | 值 |
|---|---|
| 处理器 | `DomainRankHandler`（domain.ts:25），方法 `get`（domain.ts:27） |
| 认证 | 匿名 |
| 域权限 | `PERM_VIEW_RANKING` |
| 响应 | JSON 对象（HTML 模板 `ranking.html`） |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 无 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `page` | `Types.PositiveInt` | 否 | 页码，默认 `1`；使用 `@query`，**只从 query 取值**；每页条数取设置 `pagination.ranking` |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 未实现 `post`，POST 返回 405 |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `udocs` | `User[]` | 当前页用户（按 `rp` 倒序，仅 `join: true` 且 `rp > 0`） |
| `upcount` | number | 总页数 |
| `ucount` | number | 总人数 |
| `page` | number | 当前页码 |

**示例**

```http
GET /d/system/ranking?page=1
Accept: application/json
```

```json
{ "udocs": [{ "_id": 2, "uname": "alice", "rp": 1234 }], "upcount": 5, "ucount": 87, "page": 1 }
```

#### 域仪表盘 · `domain_dashboard`

```http
GET /domain/dashboard
POST /domain/dashboard
```

| 项 | 值 |
|---|---|
| 处理器 | `DomainDashboardHandler`（domain.ts:69）；`get`（domain.ts:70）、`postInitDiscussionNode`（domain.ts:76）、`postDelete`（domain.ts:91） |
| 认证 | 匿名 |
| 域权限 | `PERM_EDIT_DOMAIN`（`ManageHandler.prepare`）；`postDelete` 另需 sudo 且必须是域主 |
| 响应 | `get`：JSON 对象（HTML 模板 `domain_dashboard.html`）；子操作：`init_discussion_node` 回跳 Referer，`delete` 302 跳转 `home_domain`（system 域） |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 无 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 无 |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `operation` | string | 是 | `init_discussion_node` 或 `delete` |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `domain` | `DomainDoc` | 当前域文档 |
| `owner` | `User` | 域主 |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| `init_discussion_node` | 清空并重建讨论分区节点：`discussion.flushNodes(domainId)` 后按 `system.get('discussion.nodes')`（YAML）逐项 `discussion.addNode`；`back()` | — |
| `delete` | 删除域：系统域抛 `CannotDeleteSystemDomainError`，非域主抛 `OnlyOwnerCanDeleteDomainError`；同时写日志 `domain.delete`，302 跳转 `home_domain`（`domainId: 'system'`） | — |

> 无 `operation` 的 POST 返回 405（未实现普通 `post`）。

**示例**

```http
POST /d/system/domain/dashboard
Content-Type: application/json

{ "operation": "init_discussion_node" }
```

```json
{ "url": "/d/system/domain/dashboard" }
```

#### 域设置 · `domain_edit`

```http
GET /domain/edit
POST /domain/edit
```

| 项 | 值 |
|---|---|
| 处理器 | `DomainEditHandler`（domain.ts:49）；`get`（domain.ts:50）、`post`（domain.ts:55） |
| 认证 | 匿名 |
| 域权限 | `PERM_EDIT_DOMAIN`（`ManageHandler.prepare`） |
| 响应 | `get`：JSON 对象（HTML 模板 `domain_edit.html`）；`post`：JSON 对象 + 302 跳转 `domain_dashboard` |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 无 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 无 |

**请求体**

`post(args)` 无装饰器，直接读取合并后的 `args`（含 `domainId`）：

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `domainId` | string | 是 | 目标域 ID |
| `booleanKeys` | object | 否 | 布尔型设置键的集合；形如 `{ "booleanKeys": { "showBulletin": 1 } }`，若 `args[key]` 为空则显式写入 `false` |
| `<settingKey>` | any | 否 | 仅 `DOMAIN_SETTINGS_BY_KEY` 中登记的键会被写入 `$set`（`packages/hydrooj/src/model/setting.ts:37`）。内置域设置键见下表 |

内置 `DOMAIN_SETTINGS` 键（`packages/hydrooj/src/model/setting.ts:276-281` 及插件追加项）：

| key | 默认值 | 类型 | 说明 |
|---|---|---|---|
| `name` | `New domain` | text | 域名称 |
| `avatar` | `''` | text | 域头像 |
| `share` | `''` | text | 题目共享域（`*` 表示任意） |
| `bulletin` | `''` | markdown | 公告 |
| `langs` | `''` | text | 允许的语言 |
| `host` | `''` | text | 自定义域名（`FLAG_HIDDEN \| FLAG_DISABLED`，不通过本接口修改） |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `current` | `DomainDoc` | 仅 `get`：当前域文档 |
| `settings` | `Setting[]` | 仅 `get`：`DOMAIN_SETTINGS`，元素结构见「`Setting` 结构」 |
| `url` | string | 仅 `post`：跳转地址 |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| — | 只实现普通 `post`；若请求体含 `operation` 则直接 `return`（不做任何修改） | — |

**示例**

```http
POST /d/system/domain/edit
Content-Type: application/json

{ "name": "示例域", "bulletin": "公告", "booleanKeys": { "showBulletin": 1 } }
```

```json
{ "url": "/d/system/domain/dashboard" }
```

#### 域成员管理 · `domain_user`

```http
GET /domain/user
POST /domain/user
```

| 项 | 值 |
|---|---|
| 处理器 | `DomainUserHandler`（domain.ts:102）；`get`（domain.ts:105）、`post`（domain.ts:169）、`postSetUsers`（domain.ts:177）、`postKick`（domain.ts:189） |
| 认证 | 匿名 |
| 域权限 | `PERM_EDIT_DOMAIN`（`ManageHandler.prepare`）；`get`、`postSetUsers`、`postKick` 另需 sudo |
| 响应 | `get`：JSON 对象（HTML 模板 `domain_user.html` 或 `domain_user_raw.html`）；子操作：JSON 对象 + 302 回跳 |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 无 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `format` | `Types.Range(['default','raw'])` | 否 | 仅 `get`；`raw` 时渲染 `domain_user_raw.html`，默认 `'default'` |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `uids` | `Types.NumericArray` | 是（`post`、`postSetUsers`、`postKick` 均需要） | 用户 ID 数组；支持 `"1,2,3"` 逗号分隔字符串或数组 |
| `role` | `Types.Role` | 是（`postSetUsers`） | 角色名（`/^[\w\u4E00-\u9FA5]{1,31}$/`） |
| `join` | `Types.Boolean` | 否（`postSetUsers`） | 默认 `false`；为真时同时 `domain.setJoin(true)`，且若未开启 `server.allowInvite` 需 `PRIV.PRIV_MANAGE_ALL_DOMAIN` |

**响应字段（`Accept: application/json`，`GET`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `roles` | `{ _id, perm, count? }[]` | 域内角色（`domain.getRoles(domainId)`） |
| `rudocs` | `Record<string, UserInDomain[]>` | 角色名 → 成员列表（含 `user._id`、`uname`、`avatar`、`role`、`join`；有 `PERM_VIEW_USER_PRIVATE_INFO` 时含 `displayName`） |
| `domain` | `DomainDoc` | 当前域 |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| `set_users` | 批量设置角色（并可选加入域）。方法 `postSetUsers`（domain.ts:177，需 sudo）；写日志 `domain.setRole`；`back()` | `uids`、`role`、`join` |
| `kick` | 踢出成员：`join=false` 且角色置为 `guest`，写日志 `domain.kick` 并发送站内信；`back()` | `uids` |
| —（无 `operation`） | 调用 `post`（domain.ts:169）：若 `uids` 包含域主则抛 `ForbiddenError`，否则无副作用 | `uids` |

**示例**

```http
POST /d/system/domain/user
Content-Type: application/json

{ "operation": "set_users", "uids": [2, 3], "role": "student", "join": true }
```

```json
{ "url": "/d/system/domain/user" }
```

#### 域角色权限 · `domain_permission`

```http
GET /domain/permission
POST /domain/permission
```

| 项 | 值 |
|---|---|
| 处理器 | `DomainPermissionHandler`（domain.ts:208）；`get`（domain.ts:210）、`post`（domain.ts:219） |
| 认证 | 匿名 |
| 域权限 | `PERM_EDIT_DOMAIN`（`ManageHandler.prepare`）+ 两个方法均需 sudo |
| 响应 | `get`：JSON 对象（HTML 模板 `domain_permission.html`）；`post`：JSON 对象 + 302 回跳 |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 无 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 无 |

**请求体（`POST`，直接读取 `this.request.body`）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `<roleName>` | `number[] \| string \| object` | 否 | 角色名 → 权限位序号列表（如 `1` 表示 `PERM_VIEW`、`46` 表示 `PERM_VIEW_TRAINING`）。值为数组、单个值或对象（取 `Object.values`）均可；位序号 `1000` 为占位符会被跳过；`root` 角色不可编辑 |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `roles` | `{ _id, perm }[]` | 仅 `get`：全部角色及其 `BigInt` 权限 |
| `PERMS_BY_FAMILY` | `Record<string, PermItem[]>` | 仅 `get`：按 `family` 分组的权限定义（`packages/hydrooj/src/model/builtin.ts:80-84`） |
| `domain` | `DomainDoc` | 仅 `get` |
| `log2` | function | 仅 `get`：模板辅助函数，JSON 序列化时丢弃 |
| `url` | string | 仅 `post`：跳转地址 |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| — | 只实现普通 `post`；提交后调用 `domain.setRoles` 并写日志 `domain.setRoles` | — |

**示例**

```http
POST /d/system/domain/permission
Content-Type: application/json

{ "default": [1, 7, 27, 46, 59], "student": [1, 7, 9, 27, 46] }
```

```json
{ "url": "/d/system/domain/permission" }
```

#### 域角色管理 · `domain_role`

```http
GET /domain/role
POST /domain/role
```

| 项 | 值 |
|---|---|
| 处理器 | `DomainRoleHandler`（domain.ts:240）；`get`（domain.ts:242）、`postAdd`（domain.ts:249）、`postDelete`（domain.ts:263） |
| 认证 | 匿名 |
| 域权限 | `PERM_EDIT_DOMAIN`（`ManageHandler.prepare`）；`get`、`postDelete` 需 sudo（`postAdd` 源码中**未**加 `@requireSudo`） |
| 响应 | `get`：JSON 对象（HTML 模板 `domain_role.html`）；子操作：JSON 对象 + 302 回跳 |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 无 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 无 |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `operation` | string | 是 | `add` 或 `delete`（该 Handler 没有普通 `post`） |
| `role` | `Types.Role` | 是（`postAdd`） | 新角色名 |
| `roles` | `Types.ArrayOf(Types.Role)` | 是（`postDelete`） | 待删除角色名数组 |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `roles` | `{ _id, perm, count? }[]` | 仅 `get`：角色列表（`domain.getRoles(domainId, true)`，含成员数 `count`） |
| `domain` | `DomainDoc` | 仅 `get` |
| `url` | string | 子操作：跳转地址 |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| `add` | 新建角色，权限继承 `default` 角色；重名抛 `RoleAlreadyExistError`；写日志 `domain.addRole`；`back()` | `role` |
| `delete` | 删除角色（成员回落到 `default`）；包含 `root`/`default`/`guest` 时抛 `ValidationError('role')`；写日志 `domain.deleteRoles`；`back()` | `roles` |

> 无 `operation` 的 POST 返回 405（未实现普通 `post`）。

**示例**

```http
POST /d/system/domain/role
Content-Type: application/json

{ "operation": "add", "role": "student" }
```

```json
{ "url": "/d/system/domain/role" }
```

#### 域用户组 · `domain_group`

```http
GET /domain/group
POST /domain/group
```

| 项 | 值 |
|---|---|
| 处理器 | `DomainUserGroupHandler`（domain.ts:317）；`get`（domain.ts:318）、`postDel`（domain.ts:327）、`postUpdate`（domain.ts:334） |
| 认证 | 匿名 |
| 域权限 | `PERM_EDIT_DOMAIN`（`ManageHandler.prepare`） |
| 响应 | `get`：JSON 对象（HTML 模板 `domain_group.html`）；子操作：JSON 对象 + 302 回跳 |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 无 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 无 |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `operation` | string | 是 | `del` 或 `update`（该 Handler 没有普通 `post`） |
| `name` | `Types.Name` | 是（`postDel`、`postUpdate`） | 用户组名 |
| `uids` | `Types.NumericArray` | 是（`postUpdate`） | 组成员用户 ID 数组（整体覆盖） |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `domain` | `DomainDoc` | 仅 `get` |
| `groups` | `GroupDoc[]` | 仅 `get`：`user.listGroup(domainId)` 返回的 `{ _id, domainId, name, uids }[]` |
| `url` | string | 子操作：跳转地址 |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| `del` | 删除用户组（`user.delGroup`）；`back()` | `name` |
| `update` | 覆盖式更新用户组成员（`user.updateGroup`，不存在则创建）；`back()` | `name`、`uids` |

> 无 `operation` 的 POST 返回 405（未实现普通 `post`）。

**示例**

```http
POST /d/system/domain/group
Content-Type: application/json

{ "operation": "update", "name": "一班", "uids": [2, 3, 4] }
```

```json
{ "url": "/d/system/domain/group" }
```

#### 加入申请设置 · `domain_join_applications`

```http
GET /domain/join_applications
POST /domain/join_applications
```

| 项 | 值 |
|---|---|
| 处理器 | `DomainJoinApplicationsHandler`（domain.ts:275）；`get`（domain.ts:276）、`post`（domain.ts:296） |
| 认证 | 匿名 |
| 域权限 | `PERM_EDIT_DOMAIN`（`ManageHandler.prepare`）；`post` 另需 sudo |
| 响应 | `get`：JSON 对象（HTML 模板 `domain_join_applications.html`）；`post`：JSON 对象 + 302 回跳 |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 无 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 无 |

**请求体（`POST`，均为 `@post`，只从请求体取值）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `method` | `Types.Range([JOIN_METHOD_NONE, JOIN_METHOD_ALL, JOIN_METHOD_CODE])` = `[0, 1, 2]` | 是 | 加入方式：`0` 禁止加入、`1` 允许任何人加入、`2` 需邀请码 |
| `role` | `Types.Role` | 否 | 加入后分配的角色；必须存在于域角色列表中，否则 `ValidationError('role')` |
| `group` | `Types.Name` | 否 | 加入后自动进入的用户组（默认 `''`） |
| `expire` | `Types.Int` | 否 | 过期时间：`0` 保持当前、`-1` 永不过期、其余为小时数（必须在 `JOIN_EXPIRATION_RANGE` 的键中，否则 `ValidationError('expire')`） |
| `invitationCode` | `Types.Content` | 否 | 邀请码，仅 `method = 2` 时写入 |

**响应字段（`Accept: application/json`，`GET`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `rolesWithText` | `[string, string][]` | 可选角色（剔除 `guest`），元素为 `[role, role]` |
| `joinSettings` | `JoinSettings \| null` | 当前生效的加入设置（`domain.getJoinSettings`） |
| `expirations` | `Record<number, string>` | `JOIN_EXPIRATION_RANGE` 的副本；当前无设置时删除 `0`（保持当前）项 |
| `url_prefix` | string | 域绑定域名（`domain.host[0]`）或 `server.url`，保证以 `/` 结尾 |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| — | 只实现普通 `post`；`method = 0` 时写入 `_join: null`，否则组装 `{ method, role, group, expire, code? }` 并 `domain.edit` | — |

**示例**

```http
POST /d/system/domain/join_applications
Content-Type: application/json

{ "method": 2, "role": "default", "group": "一班", "expire": 168, "invitationCode": "hydro2026" }
```

```json
{ "url": "/d/system/domain/join_applications" }
```

#### 加入域 · `domain_join`

```http
GET /domain/join
POST /domain/join
```

| 项 | 值 |
|---|---|
| 处理器 | `DomainJoinHandler`（domain.ts:340）；`prepare`（domain.ts:345）、`get`（domain.ts:366）、`post`（domain.ts:387） |
| 认证 | 需要登录（路由级 `PRIV.PRIV_USER_PROFILE`） |
| 域权限 | —（`noCheckPermView = true`，跳过默认 `PERM_VIEW` 检查） |
| 响应 | `get`：JSON 对象（HTML 模板 `domain_join.html`）；`post`：302 跳转 |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 无（`target` 通过查询/请求体传入） |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `code` | `Types.Content` | 否 | 邀请码，默认 `''` |
| `target` | `Types.DomainId` | 否 | 目标域，默认当前域；格式 `/^[a-zA-Z]\w{3,31}$/` |
| `redirect` | `Types.Content` | 否 | 加入成功后的跳转地址，默认 `''` |

`prepare` 校验：目标域不存在抛 `NotFoundError`；已是成员抛 `DomainJoinAlreadyMemberError`；有 `PRIV_MANAGE_ALL_DOMAIN` 时视为 `root`（跳过限制）；`guest` 角色抛 `DomainJoinForbiddenError('You are banned by the domain moderator.')`；非 `default` 角色以外的路径会删除 `joinSettings`，而 `default` 角色且无有效 `joinSettings` 时抛 `DomainJoinForbiddenError('The link is either invalid or expired.')`。

**请求体（`POST`）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `code` | `Types.Content` | 否 | 邀请码，默认 `''` |
| `target` | `Types.DomainId` | 否 | 目标域，默认当前域 |
| `redirect` | `Types.Content` | 否 | 跳转地址，默认 `''` |

**响应字段（`Accept: application/json`，`GET`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `joinSettings` | `JoinSettings \| null` | 生效的加入设置 |
| `code` | string | 回显邀请码 |
| `redirect` | string | 回显跳转地址 |
| `target` | string | 目标域 ID |
| `domainInfo` | `{ name, owner, avatar, bulletin }` | 域信息；`bulletin` 仅在 `showBulletin` 为真时返回 |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| — | 只实现普通 `post`：校验邀请码（`InvalidJoinInvitationCodeError`）、按 `joinSettings.group` 加入用户组、`domain.setUserInDomain({ join: true, role })`、写日志 `domain.join`，最后 302 跳转 `redirect` 或首页（带 `notification=Successfully joined domain.`） | — |

**示例**

```http
POST /d/system/domain/join?target=system&code=hydro2026
Content-Type: application/json
```

```json
{ "url": "/?notification=Successfully%20joined%20domain." }
```

#### 域搜索 · `domain_search`

```http
GET /domain/search
```

| 项 | 值 |
|---|---|
| 处理器 | `DomainSearchHandler`（domain.ts:410），方法 `get`（domain.ts:412） |
| 认证 | 需要登录（路由级 `PRIV.PRIV_USER_PROFILE`） |
| 域权限 | — |
| 响应 | JSON 数组（未设置 `response.template`，**始终返回原始 JSON**） |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 无 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `q` | `Types.Content` | 否 | 搜索前缀，默认 `''`；为空时返回当前用户**已加入**的域列表，否则按 `_id`/`name` 前缀（不区分大小写正则）取前 20 条 |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 未实现 `post`，POST 返回 405 |

**响应字段（`Accept: application/json`）**

响应体是 `DomainDoc[]` 数组，每个元素额外带一个字段：

| 字段 | 类型 | 说明 |
|---|---|---|
| `_id` | string | 域 ID |
| `owner` | number | 域主 UID |
| `roles` | `Record<string, string>` | 角色名 → `BigInt` 权限字符串 |
| `avatar` | string | 域头像路径 |
| `bulletin` | string | 公告 |
| `_join` | any | 加入设置（见「加入设置」） |
| `host` | string[] | 绑定域名列表 |
| `avatarUrl` | string | 由 `avatar(avatar, 64)` 生成；无头像时为 `/img/team_avatar.png` |

**示例**

```http
GET /d/system/domain/search?q=hyd
Accept: application/json
```

```json
[
  { "_id": "hydro", "name": "Hydro 官方域", "owner": 2, "avatar": "", "avatarUrl": "/img/team_avatar.png" }
]
```

#### 域 API（`DomainApi`）

> 源码：`packages/hydrooj/src/handler/domain.ts:426-478`，通过 `api.provide(DomainApi)` 注册（domain.ts:490-492）。
> HTTP 入口由 `applyApiHandler(childContext, 'api', '/api/:op')` 注册（`packages/hydrooj/src/service/server.ts:111`），即路由 `api`（`/api/:op`）与连接 `api_conn`（`/api/:op/conn`）。
> 调用约定（`framework/framework/api.ts:190-220`）：`op` 为 API 名；参数可放在 `args` 里（字符串会自动 `JSON.parse`）；`projection` 可传 JSON 或逗号分隔字段名；`Mutation` 不能用 GET 调用；Subscription 只能在连接上调用。

| op | 类型 | 输入 Schema | 返回 |
|---|---|---|---|
| `domain` | Query | `{ id?: string }` | `DomainDoc \| null`；`id` 缺省时取当前域；无 `PERM_VIEW` 且无 `PRIV_VIEW_ALL_DOMAIN` 时返回 `null` |
| `domain.current` | Query | `{}` | `{ domain: DomainDoc }` |
| `groups` | Query | `{ domainId: string（必填）, uid?: number, names?: string[], search?: string, limit?: number（≤100） }` | `GroupDoc[]`；`limit` 缺省为 `search ? 20 : undefined`；传入 `uid` 时会追加一个名为 `uid` 的伪分组；无 `PERM_VIEW` 且无 `PRIV_VIEW_ALL_DOMAIN` 抛 `PermissionError(PERM_VIEW)` |
| `domain.group` | Mutation | `{ name: string（必填）, uids?: number[] }` | `boolean`；无 `PERM_EDIT_DOMAIN` 抛 `PermissionError(PERM_EDIT_DOMAIN)`；有 `uids` 时 `updateGroup` 并返回 `upsertedCount > 0`，否则 `delGroup` 并返回 `deletedCount > 0` |

**示例**

```http
GET /api/domain?args=%7B%22id%22%3A%22system%22%7D
Accept: application/json
```

```json
{ "_id": "system", "name": "Hydro", "owner": 1, "roles": { "root": "-1" }, "avatar": "", "bulletin": "" }
```

```http
POST /api/domain.group
Content-Type: application/json

{ "args": { "name": "一班", "uids": [2, 3] } }
```

```json
true
```

---

### 系统管理（Manage）

> 源码：`packages/hydrooj/src/handler/manage.ts`（路由注册见 manage.ts:355-364）。
> `SystemHandler.prepare`（manage.ts:49）对所有 HTTP 管理路由执行 `checkPriv(PRIV.PRIV_EDIT_SYSTEM)`；`@requireSudo` 表示还需要 sudo 会话。

#### 管理首页 · `manage`

```http
GET /manage
```

| 项 | 值 |
|---|---|
| 处理器 | `SystemMainHandler`（manage.ts:54），方法 `get`（manage.ts:55） |
| 认证 | 需要 `PRIV_EDIT_SYSTEM` |
| 域权限 | — |
| 响应 | 302 重定向到 `/manage/dashboard`（JSON 模式下为 `{ "url": "/manage/dashboard" }`） |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 无 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 无 |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 未实现 `post`，POST 返回 405 |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `url` | string | 固定 `/manage/dashboard` |

**示例**

```http
GET /manage
Accept: application/json
```

```json
{ "url": "/manage/dashboard" }
```

#### 系统仪表盘 · `manage_dashboard`

```http
GET /manage/dashboard
POST /manage/dashboard
```

| 项 | 值 |
|---|---|
| 处理器 | `SystemDashboardHandler`（manage.ts:80）；`get`（manage.ts:81）、`postRestart`（manage.ts:85） |
| 认证 | 需要 `PRIV_EDIT_SYSTEM` |
| 域权限 | — |
| 响应 | `get`：HTML 模板 `manage_dashboard.html`（响应体为 `{}`）；子操作：JSON 对象 + 302 回跳 |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 无 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 无 |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `operation` | string | 是 | 仅 `restart` |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| — | — | `get` 未设置 `response.body`，JSON 模式下返回 `{}` |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| `restart` | 执行 `pm2 reload "${process.env.name}"` 重载进程；非 PM2 启动（无 `process.env.pm_cwd`）时抛 `NotLaunchedByPM2Error`；`back()` | — |

> 无 `operation` 的 POST 返回 405（未实现普通 `post`）。

**示例**

```http
POST /manage/dashboard
Content-Type: application/json

{ "operation": "restart" }
```

```json
{ "url": "/manage/dashboard" }
```

#### 脚本执行 · `manage_script`

```http
GET /manage/script
POST /manage/script
```

| 项 | 值 |
|---|---|
| 处理器 | `SystemScriptHandler`（manage.ts:92）；`get`（manage.ts:94）、`post`（manage.ts:102） |
| 认证 | 需要 `PRIV_EDIT_SYSTEM`，且两个方法均需 sudo |
| 域权限 | — |
| 响应 | `get`：JSON 对象（HTML 模板 `manage_script.html`）；`post`：JSON 对象 + 302 跳转 `record_detail` |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 无 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 无 |

**请求体（`POST`）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `id` | `Types.Name` | 是 | 脚本名，必须存在于 `global.Hydro.script`，否则 `ValidationError('id')` |
| `args` | `Types.Content` | 否 | 脚本参数 JSON 字符串，默认 `'{}'`；装饰器名为 `args`，方法形参名为 `raw`；会先 `JSON.parse`，再经脚本自身的 `validate`（若存在） |

脚本对象由 `ctx.addScript(name, description, validate, run)` 注册为 `{ description, validate, run }`（`packages/hydrooj/src/context.ts:28-32`）。

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `scripts` | `Record<string, { description, validate, run }>` | 仅 `get`：`global.Hydro.script`（函数字段在 JSON 序列化时被丢弃） |
| `rid` | number | 仅 `post`：为本次执行创建的 pretest 记录 ID（`record.add(domainId, -1, this.user._id, '-', id, false, { input: [raw], type: 'pretest' })`） |

执行结果通过 `JudgeResultCallbackContext` 流式写入该记录，最终 `status` 为 `STATUS_ACCEPTED` 或 `STATUS_SYSTEM_ERROR`。

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| — | 只实现普通 `post` | — |

**示例**

```http
POST /manage/script
Content-Type: application/json

{ "id": "problemStat", "args": "{}" }
```

```json
{ "rid": 12345, "url": "/record/12345" }
```

#### 系统设置 · `manage_setting`

```http
GET /manage/setting
POST /manage/setting
```

| 项 | 值 |
|---|---|
| 处理器 | `SystemSettingHandler`（manage.ts:136）；`get`（manage.ts:138）、`post`（manage.ts:148） |
| 认证 | 需要 `PRIV_EDIT_SYSTEM`，且两个方法均需 sudo |
| 域权限 | — |
| 响应 | `get`：JSON 对象（HTML 模板 `manage_setting.html`）；`post`：JSON 对象 + 302 回跳 |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 无 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 无 |

**请求体（`POST`，直接读取合并后的 `args`）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `<groupKey>` | object | 否 | 形如 `{ "server": { "name": "Hydro", "port": 8888 } }`，会按 `key.subkey` 写入 `system.set` |
| `booleanKeys` | object | 否 | 布尔型键的集合；形如 `{ "booleanKeys": { "server": { "checkUpdate": 1 } } }`，若对应 `args[key][subkey]` 为空则显式写入 `false` |

`set()` 的取值规则（manage.ts:24-45）：

| 情况 | 处理 |
|---|---|
| key 不在 `SYSTEM_SETTINGS_BY_KEY` 中 | 忽略（返回 `undefined`） |
| 设置项带 `FLAG_DISABLED` | 忽略 |
| 设置项带 `FLAG_SECRET` 且值为空 | 忽略 |
| `type === 'boolean'` | 值为字符串 `'on'` → `true`，否则 `false` |
| `type === 'number'` | 必须能转为安全整数，否则 `ValidationError(key)` |
| `subType === 'yaml'` | 先用 `yaml.load` 校验，失败抛 `ValidationError(key)` |

提交成功后广播 `system/setting`。

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `current` | `Record<string, any>` | 仅 `get`：`SYSTEM_SETTINGS` 中每个 `key` 的当前值（`system.get(key)`） |
| `settings` | `Setting[]` | 仅 `get`：`SYSTEM_SETTINGS`，元素结构见「`Setting` 结构」 |
| `url` | string | 仅 `post`：跳转地址 |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| — | 只实现普通 `post` | — |

**示例**

```http
POST /manage/setting
Content-Type: application/json

{ "server": { "name": "Hydro OJ" }, "booleanKeys": { "server": { "checkUpdate": 1 } } }
```

```json
{ "url": "/manage/setting" }
```

#### 系统配置（配置文件） · `manage_config`

```http
GET /manage/config
POST /manage/config
```

| 项 | 值 |
|---|---|
| 处理器 | `SystemConfigHandler`（manage.ts:175）；`get`（manage.ts:177）、`post`（manage.ts:210） |
| 认证 | 需要 `PRIV_EDIT_SYSTEM`，且两个方法均需 sudo |
| 域权限 | — |
| 响应 | `get`：JSON 对象（HTML 模板 `manage_config.html`）；`post`：无响应体（JSON 模式下为 `{}`） |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 无 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 无 |

**请求体（`POST`）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `value` | `Types.String` | 是 | 完整的 YAML 配置文本；解析失败抛 `ValidationError('value', '', e.message)` |

`get` 会把 schema 中标记为 `secret`（`meta.secret === true` 或 `meta.role === 'secret'`）的字符串字段显示为 `'[hidden]'`；`post` 时这些字段若仍为 `'[hidden]'` 则沿用旧值，最后调用 `ctx.setting.saveConfig(config)`。

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `schema` | object | 仅 `get`：`Schema.intersect(this.ctx.setting.settings).toJSON()` |
| `value` | string | 仅 `get`：经秘密字段脱敏后的 YAML 文本（异常时回退为原始 `configSource`） |
| — | — | `post` 未设置 `response.body`，返回 `{}` |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| — | 只实现普通 `post` | — |

**示例**

```http
POST /manage/config
Content-Type: application/json

{ "value": "server:\n  name: Hydro OJ\n" }
```

```json
{}
```

#### 批量导入用户 · `manage_user_import`

```http
GET /manage/userimport
POST /manage/userimport
```

| 项 | 值 |
|---|---|
| 处理器 | `SystemUserImportHandler`（manage.ts:237）；`get`（manage.ts:238）、`post`（manage.ts:245） |
| 认证 | 需要 `PRIV_EDIT_SYSTEM`（未加 `@requireSudo`） |
| 域权限 | — |
| 响应 | `get`：JSON 对象（HTML 模板 `manage_user_import.html`）；`post`：JSON 对象（未设置模板，返回原始 JSON） |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 无 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 无 |

**请求体（`POST`）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `users` | `Types.Content` | 是 | 多行文本，每行一个用户 |
| `draft` | `Types.Boolean` | 否 | 为真时仅解析并返回校验结果，不实际创建用户（`Types.Boolean` 自带 `isOptional = true`） |

每行格式：优先按 **Tab** 分隔为 `email / username / password / displayName / extra`；若前三个字段缺失则改用 **逗号** 分隔（逗号超过 5 段时，第 5 段起合并为 `extra`）。`extra` 需为 JSON，可携带 `group`（用户组名，创建后加入）以及任意自定义字段（会经 `ctx.serial('user/import/parse')` 钩子处理）。

实际创建（`draft` 为假）时：`user.create` → 按需 `domain.setUserInDomain({ displayName })`、`user.setById({ school })`、`user.setById({ studentId })` → 触发 `user/import/create` 钩子 → 汇总 `groups` 并 `user.updateGroup`。

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `users` | `object[]` | `get` 时为 `[]`；`post` 时为解析出的用户载荷数组 |
| `messages` | string[] | 仅 `post`：逐行校验信息（非法邮箱/用户名/密码、邮箱或用户名已存在、输入非法）与最终 `"N users found."` |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| — | 只实现普通 `post` | — |

**示例**

```http
POST /manage/userimport
Content-Type: application/json

{ "users": "a@example.com\talice\tpass123\tAlice\t{\"group\":\"一班\"}", "draft": true }
```

```json
{ "users": [{ "email": "a@example.com", "username": "alice", "password": "pass123", "displayName": "Alice" }], "messages": ["1 users found."] }
```

#### 用户权限 · `manage_userpriv`

```http
GET /manage/userpriv
POST /manage/userpriv
```

| 项 | 值 |
|---|---|
| 处理器 | `SystemUserPrivHandler`（manage.ts:316）；`get`（manage.ts:319）、`post`（manage.ts:338） |
| 认证 | 需要 `PRIV_EDIT_SYSTEM`，且两个方法均需 sudo |
| 域权限 | — |
| 响应 | `get`：JSON 对象（HTML 模板 `manage_user_priv.html`，pjax 片段 `partials/manage_user_priv.html`）；`post`：JSON 对象 + 302 回跳 |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 无 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `extraIgnore` | `Types.NumericArray` | 否 | 额外忽略的 `priv` 值（`@param`，合并取值），默认 `[]`；用于过滤查询结果 |

**请求体（`POST`）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `uid` | `Types.Int` | 是 | 目标用户 ID |
| `priv` | `Types.UnsignedInt` | 是 | 目标权限值 |
| `system` | `Types.Boolean` | 否 | 装饰器名为 `system`，方法形参名为 `editSystem`（`Types.Boolean` 自带 `isOptional = true`）。为真时批量修改「默认权限」；为假时修改单个用户 |

`system = false` 时：用户不存在抛 `UserNotFoundError`；`udoc.priv === -1` 或 `priv === -1` 或 `priv === allPriv`（所有 PRIV 位之和）抛 `CannotEditSuperAdminError`；否则 `user.setPriv`。

`system = true` 时：把当前所有等于 `default.priv` 的用户批量改为新值，并 `system.set('default.priv', priv)`，随后广播 `user/delcache`。

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `udocs` | `User[]` | 仅 `get`：`priv` 不等于 `0`/默认值/忽略值的用户（最多 1000 条）与所有 `priv === 0` 的封禁用户（最多 1000 条） |
| `defaultPriv` | number | 仅 `get`：`system.get('default.priv')` |
| `Priv` | `Record<string, number>` | 仅 `get`：`PRIV` 常量去掉 `PRIV_DEFAULT`、`PRIV_NEVER`、`PRIV_NONE`、`PRIV_ALL` 后的映射 |
| `url` | string | 仅 `post`：跳转地址 |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| — | 只实现普通 `post` | — |

**示例**

```http
POST /manage/userpriv
Content-Type: application/json

{ "uid": 2, "priv": 4, "system": false }
```

```json
{ "url": "/manage/userpriv" }
```

#### 系统自检（WebSocket） · `manage_check`

```http
WebSocket /manage/check-conn
```

| 项 | 值 |
|---|---|
| 处理器 | `SystemCheckConnHandler`（manage.ts:60）；`prepare`（manage.ts:63）、`check`（manage.ts:68）、`cleanup`（manage.ts:75） |
| 认证 | 需要 `PRIV_EDIT_SYSTEM`（在 `prepare` 中 `checkPriv`） |
| 域权限 | — |
| 响应 | WebSocket 文本帧（每条消息为 JSON 字符串）；启用 `server.enableSSE` 时同一路径也支持 `GET`（`text/event-stream`，带 `sse` 参数时每条消息前缀 `data: `） |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 无 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `sse` | `Types.Boolean` | 否 | 仅在 SSE（HTTP）模式下由框架读取：为真时按 `data: <json>\n\n` 格式输出 |

**请求体（客户端 → 服务端）**

| 消息 | 说明 |
|---|---|
| `"ping"` | 客户端心跳，服务端回 `"pong"` |
| 其他 | 会被 `JSON.parse` 后交给 `message()` 处理；非 RPC 模式下 `message()` 抛 `BadRequestError('Only RPC operations are supported')` |

**响应消息（服务端 → 客户端）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `type` | `'log' \| 'warn' \| 'error'` | 检查项报告级别 |
| `payload` | any | 报告内容（字符串或对象） |
| `error` | `{ name, params }` | 出错时发送的字段，随后以 4000 关闭连接 |

自检项由 `CheckService` 注册（`packages/hydrooj/src/service/check.ts`）：`Db`（MongoDB 读写/建索引）、`Storage`（存储连通性）、`System`（非 Linux 平台告警）、`Mail`（未配置 `smtp.from` 告警）、`Setting`（`server.url` 未设置、`server.xff` 与实际 IP 不符告警）、`Inspector`（启用 inspector 告警）。

**示例**

```text
> { "type": "log", "payload": "Storage: ok" }
> { "type": "warn", "payload": "SMTP account is not provided, email verification disabled." }
```

---

### 杂项（Misc）

> 源码：`packages/hydrooj/src/handler/misc.ts`（路由注册见 misc.ts:158-166）。

#### 切换界面语言 · `switch_language`

```http
GET /language/:lang
```

| 项 | 值 |
|---|---|
| 处理器 | `SwitchLanguageHandler`（misc.ts:20），方法 `get`（misc.ts:24）；`noCheckPermView = true` |
| 认证 | 匿名 |
| 域权限 | — |
| 响应 | JSON 对象 + 302 回跳 Referer（`this.back()`） |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `lang` | `Types.Name` | 语言代码，必填（`@param`，合并取值） |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 无 |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 仅 GET |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `url` | string | 跳转地址（`Referer`，缺省为 `/`） |

已登录用户（`PRIV_USER_PROFILE`）会同时写入 `session.viewLang` 与 `user.setById({ viewLang })`；未登录仅写 session。

**示例**

```http
GET /language/zh_CN
Accept: application/json
```

```json
{ "url": "/" }
```

#### 用户文件列表 · `home_files`

```http
GET /file
POST /file
```

| 项 | 值 |
|---|---|
| 处理器 | `FilesHandler`（misc.ts:33）；`prepare`（misc.ts:38）、`get`（misc.ts:47）、`postUploadFile`（misc.ts:58）、`postDeleteFiles`（misc.ts:80）；`noCheckPermView = true` |
| 认证 | 匿名；查看他人文件需 `PRIV_EDIT_SYSTEM`；上传需 `PRIV_CREATE_FILE` |
| 域权限 | — |
| 响应 | `get`：JSON 对象（HTML 模板 `home_files.html`，pjax 片段 `partials/files.html`）；子操作：JSON 对象 + 302 回跳 |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 无 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `uid` | `Types.Int` | 否 | `@param`（合并取值）：指定查看哪个用户的文件；非空时需 `PRIV_EDIT_SYSTEM`，并以 `(await user.getById(domainId, uid)).private()` 作为目标用户；缺省为当前用户 |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `operation` | string | 是 | `upload_file` 或 `delete_files`（该 Handler 没有普通 `post`） |
| `filename` | `Types.Filename` | 是（`postUploadFile`） | 文件名，`@post` 只从请求体取值 |
| `file` | 文件（multipart） | 是（`postUploadFile`） | 通过 `this.request.files.file` 读取，缺失抛 `ValidationError('file')` |
| `files` | `Types.ArrayOf(Types.Filename)` | 是（`postDeleteFiles`） | 待删除文件名数组 |

上传限制：数量上限 `system.get('limit.user_files')`、总大小上限 `system.get('limit.user_files_size')`，超限时若用户有 `PRIV_UNLIMITED_QUOTA` 则放行，否则抛 `FileLimitExceededError`；同名文件抛 `FileExistsError`。存储路径 `user/<uid>/<filename>`。

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `files` | `FileInfo[]` | `sortFiles(udoc._files)` 排序后的文件列表（`_id`/`name`/`size`/`lastModified`/`etag`） |
| `urlForFile` | function | `(filename) => url('fs_download', { uid, filename })`，JSON 序列化时丢弃 |

**POST 子操作（`operation`）**

| operation | 说明 | 额外参数 |
|---|---|---|
| `upload_file` | 上传文件并写入 `user._files`；`back()` | `filename`、`file` |
| `delete_files` | 删除存储对象并更新 `user._files`；`back()` | `files` |

> 无 `operation` 的 POST 返回 405（未实现普通 `post`）。

**示例**

```http
GET /file
Accept: application/json
```

```json
{ "files": [{ "_id": "a.zip", "name": "a.zip", "size": 10240, "etag": "abc" }] }
```

#### 用户文件下载 · `fs_download`

```http
GET /file/:uid/:filename
```

| 项 | 值 |
|---|---|
| 处理器 | `FSDownloadHandler`（misc.ts:89），方法 `get`（misc.ts:95）；`noCheckPermView = true` |
| 认证 | 匿名（凭签名链接访问） |
| 域权限 | — |
| 响应 | 302 重定向到签名下载链接；`Cache-Control: public` |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `uid` | `Types.Int` | 用户 ID，必填 |
| `filename` | `Types.Filename` | 文件名，必填 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `noDisposition` | `Types.Boolean` | 否 | `@param`（合并取值）；为真时不带下载文件名（内联预览），默认 `false` |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 仅 GET |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `url` | string | 302 跳转地址 |

存储路径不存在（`Invalid path`）时抛 `NotFoundError(filename)`；同时记录 `oplog.log(this, 'download.file.user', { target, size })`。

**示例**

```http
GET /file/2/a.zip
Accept: application/json
```

```json
{ "url": "/storage?target=user%2F2%2Fa.zip&expire=1730000000000&secret=..." }
```

#### 存储下载 · `storage`

```http
GET /storage
```

| 项 | 值 |
|---|---|
| 处理器 | `StorageHandler`（misc.ts:114），方法 `get`（misc.ts:122）；`noCheckPermView = true`、`notUsage = true`（跳过全局限流） |
| 认证 | 匿名（凭签名参数校验） |
| 域权限 | — |
| 响应 | 二进制文件内容（`response.body` 为 Buffer/流，`response.type` 按 MIME 推断） |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 无 |

**查询参数（均为 `@param`，合并取值）**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `target` | `Types.Name` | 是 | 存储对象路径，如 `user/2/a.zip` |
| `expire` | `Types.UnsignedInt` | 是 | 过期时间戳（毫秒）；`expire < Date.now()` 抛 `AccessDeniedError` |
| `secret` | `Types.String` | 是 | 签名；`ctx.get('storage').isLinkValid(\`${target}/${expire}/${secret}\`)` 为假时抛 `AccessDeniedError` |
| `filename` | `Types.Filename` | 否 | 提供时设置 `Content-Disposition: attachment; filename="..."`（RFC 5987 编码），默认 `''` |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 仅 GET |

**响应字段（`Accept: application/json`）**

响应为文件流，无 JSON 结构。`Content-Type` 规则：`target` 以 `.out` 或 `.ans` 结尾时为 `text/plain`，否则按 `mime-types` 的 `lookup(target)` 推断，兜底 `application/octet-stream`。

**示例**

```http
GET /storage?target=user/2/a.zip&expire=1730000000000&secret=abcd
```

#### 切换账号（sudo 后） · `switch_account`

```http
GET /account/:uid
```

| 项 | 值 |
|---|---|
| 处理器 | `SwitchAccountHandler`（misc.ts:133），方法 `get`（misc.ts:136） |
| 认证 | 需要 `PRIV_EDIT_SYSTEM`（路由级），且需 sudo |
| 域权限 | — |
| 响应 | JSON 对象 + 302 回跳 Referer |

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| `uid` | `Types.Int` | 要切换到的用户 ID，必填 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 无 |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 仅 GET |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `url` | string | 跳转地址（`Referer`，缺省为 `/`） |

行为：`session.sudoUid = this.user._id`（保留原身份以便退出），`session.uid = uid`。

**示例**

```http
GET /account/2
Accept: application/json
```

```json
{ "url": "/manage/dashboard" }
```

#### 堆快照 · `heap_snapshot`

```http
POST /heap-snapshot
```

| 项 | 值 |
|---|---|
| 处理器 | `HeapSnapshotHandler`（misc.ts:143），方法 `post`（misc.ts:145） |
| 认证 | 需要 `PRIV_EDIT_SYSTEM`（路由级 `PRIV.PRIV_EDIT_SYSTEM`，方法内再 `checkPriv` 一次） |
| 域权限 | — |
| 响应 | JSON 对象（未设置模板，**始终返回原始 JSON**） |

**注册条件**：仅当进程启动参数包含 `--enable-heap-snapshot` 时注册（misc.ts:164-166）。

**路径参数**

| 参数 | 类型 | 说明 |
|---|---|---|
| — | — | 无 |

**查询参数**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| — | — | — | 无（参数为 `@param`，合并取值） |

**请求体**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `worker` | `Types.Int` | 是 | 目标 worker 编号；非 `0` 且与 `process.env.NODE_APP_INSTANCE` 不一致时返回 `{ "error": "Not current worker" }` 且不生成快照 |

**响应字段（`Accept: application/json`）**

| 字段 | 类型 | 说明 |
|---|---|---|
| `worker` | string | 当前进程的 `process.env.NODE_APP_INSTANCE` |
| `filename` | string | `v8.writeHeapSnapshot()` 返回的快照文件路径 |
| `error` | string | 仅在 worker 不匹配时出现，值为 `Not current worker` |

**示例**

```http
POST /heap-snapshot
Content-Type: application/json

{ "worker": 0 }
```

```json
{ "worker": "0", "filename": "D:\\project\\Hydro-master\\Heap.20260917.120000.0.001.heapsnapshot" }
```

---

### 域（Domain）与权限模型

#### `DomainDoc` 字段

> 定义：`packages/hydrooj/src/interface.ts:310-318`；创建时的默认值见 `packages/hydrooj/src/model/domain.ts:61-79`（`DomainModel.add`）。
> `DomainDoc extends Record<string, any>`，因此除下表外还可通过域设置（`DOMAIN_SETTINGS`）写入任意键（如 `share`、`langs`、`bulletin`、`showBulletin` 等）。

| 字段 | 类型 | 说明 |
|---|---|---|
| `_id` | string | 域 ID（唯一标识，同时也是集合主键） |
| `lower` | string | `_id` 的小写形式，用于大小写不敏感查询（唯一索引，`model/domain.ts:406-411`） |
| `owner` | number | 域主 UID；创建域时会被自动设为 `root` 角色并 `join: true` |
| `name` | string | 域名称（默认 `New domain`） |
| `avatar` | string | 域头像路径（默认 `''`） |
| `bulletin` | string | 域公告（Markdown） |
| `roles` | `Record<string, string>` | 角色名 → `BigInt` 权限值的**字符串**形式（如 `{ "root": "-1", "default": "1370624076369558733505" }`） |
| `_join` | any | 加入设置，见「加入设置（`_join`）」 |
| `host` | string[] | 绑定的自定义域名列表（`host` 索引为 sparse） |

域成员数据存放在 `domain.user` 集合（`DomainModel.collUser`，字段为 `Record<string, any>`）：

| 字段 | 类型 | 说明 |
|---|---|---|
| `domainId` | string | 域 ID |
| `uid` | number | 用户 ID（`{ domainId, uid }` 为唯一索引） |
| `role` | string | 域内角色名，默认 `default` |
| `join` | boolean | 是否已加入该域 |
| `displayName` | string | 域内昵称（`setUserInDomain` 可写） |
| `rp` | number | 域内 RP（排名用；`{ domainId, rp: -1, uid: 1 }` 稀疏索引） |

#### 角色（role）与权限位

内置角色（`packages/hydrooj/src/model/builtin.ts:89-93`）：

| 角色 | 权限值 | 说明 |
|---|---|---|
| `guest` | `PERM_BASIC` = `37483536664552833153` | 未登录/未加入/被封禁用户 |
| `default` | `PERM_DEFAULT` = `1370624076369558733505` | 已加入域的普通用户 |
| `root` | `PERM_ALL` = `-1` | 域主/超级管理员，拥有全部权限 |

`domain.getRoles(domainId, count?)` 会把 `ddoc.roles` 中的自定义角色与内置角色合并返回，元素形如 `{ _id, perm, count? }`（`model/domain.ts:203-227`）。

**角色解析规则**（`DomainModel.getDomainUser`，`model/domain.ts:263-277`）：

| 条件 | 结果角色 |
|---|---|
| 无 `PRIV_USER_PROFILE` | 强制 `guest` |
| 未 `join` 且无 `PRIV_VIEW_ALL_DOMAIN` | 强制 `guest` |
| 有 `PRIV_MANAGE_ALL_DOMAIN` | 强制 `root` |
| 以上均不满足 | 使用 `dudoc.role`，为空时取 `default` |

最终权限 `dudoc.perm = ddoc.roles[role] ? BigInt(ddoc.roles[role]) : BUILTIN_ROLES[role]`，即**域自定义角色优先，否则回退到内置角色权限**。

#### 加入设置（`_join`）与 `getJoinSettings`

常量（`packages/hydrooj/src/model/domain.ts:44-60`）：

| 常量 | 值 | 说明 |
|---|---|---|
| `JOIN_METHOD_NONE` | `0` | 不允许任何人加入 |
| `JOIN_METHOD_ALL` | `1` | 任何人可加入 |
| `JOIN_METHOD_CODE` | `2` | 需要邀请码 |
| `JOIN_EXPIRATION_KEEP_CURRENT` | `0` | 保持当前过期时间 |
| `JOIN_EXPIRATION_UNLIMITED` | `-1` | 永不过期 |

`JOIN_METHOD_RANGE`（方法 → 描述）：

| 值 | 描述 |
|---|---|
| `0` | `No user is allowed to join this domain` |
| `1` | `Any user is allowed to join this domain` |
| `2` | `Any user is allowed to join this domain with an invitation code` |

`JOIN_EXPIRATION_RANGE`（小时数 → 描述）：

| 键 | 描述 |
|---|---|
| `0` | `Keep current expiration` |
| `3` | `In 3 hours` |
| `24` | `In 1 day` |
| `72` | `In 3 days` |
| `168` | `In 1 week` |
| `720` | `In 1 month` |
| `-1` | `Never expire` |

`_join` 结构：

| 字段 | 类型 | 说明 |
|---|---|---|
| `method` | number | `0` / `1` / `2` |
| `role` | string | 加入后分配的角色 |
| `group` | string | 加入后自动进入的用户组（可为空） |
| `expire` | `Date \| null` | `null` 表示永不过期；由 `moment().add(expire, 'hours').toDate()` 计算 |
| `code` | string | 仅 `method === 2` 时存在 |

`getJoinSettings(ddoc, roles)` 的返回规则（`model/domain.ts:339-347`）：`_join` 不存在 → `null`；`method === JOIN_METHOD_NONE` → `null`；`role` 不在当前域角色列表中 → `null`；`expire` 已过期（`expire < new Date()`）→ `null`；否则返回 `_join` 本身。

#### `PERM` 常量完整对照表

> 定义：`packages/common/permission.ts`（`PERM` 全部为 `BigInt`，`1n << n` 表示第 n 位）。

| 常量 | 位 | 十进制值 | 分组 |
|---|---|---|---|
| `PERM_NONE` | — | `0` | 占位 |
| `PERM_VIEW` | 0 | `1` | Domain Settings |
| `PERM_EDIT_DOMAIN` | 1 | `2` | Domain Settings |
| `PERM_MOD_BADGE` | 2 | `4` | Domain Settings |
| `PERM_VIEW_DISPLAYNAME` | 67 | `147573952589676412928` | Domain Settings（**已废弃**，等价 `PERM_VIEW_USER_PRIVATE_INFO`） |
| `PERM_VIEW_USER_PRIVATE_INFO` | 67 | `147573952589676412928` | Domain Settings |
| `PERM_CREATE_PROBLEM` | 4 | `16` | Problem |
| `PERM_EDIT_PROBLEM` | 5 | `32` | Problem |
| `PERM_EDIT_PROBLEM_SELF` | 6 | `64` | Problem |
| `PERM_VIEW_PROBLEM` | 7 | `128` | Problem |
| `PERM_VIEW_PROBLEM_HIDDEN` | 8 | `256` | Problem |
| `PERM_SUBMIT_PROBLEM` | 9 | `512` | Problem |
| `PERM_READ_PROBLEM_DATA` | 10 | `1024` | Problem |
| `PERM_VIEW_RECORD` | 70 | `1180591620717411303424` | Record |
| `PERM_READ_RECORD_CODE` | 12 | `4096` | Record |
| `PERM_READ_RECORD_CODE_ACCEPT` | 66 | `73786976294838206464` | Record |
| `PERM_REJUDGE_PROBLEM` | 13 | `8192` | Record |
| `PERM_REJUDGE` | 14 | `16384` | Record |
| `PERM_VIEW_PROBLEM_SOLUTION` | 15 | `32768` | Problem Solution |
| `PERM_VIEW_PROBLEM_SOLUTION_ACCEPT` | 65 | `36893488147419103232` | Problem Solution |
| `PERM_CREATE_PROBLEM_SOLUTION` | 16 | `65536` | Problem Solution |
| `PERM_VOTE_PROBLEM_SOLUTION` | 17 | `131072` | Problem Solution |
| `PERM_EDIT_PROBLEM_SOLUTION` | 18 | `262144` | Problem Solution |
| `PERM_EDIT_PROBLEM_SOLUTION_SELF` | 19 | `524288` | Problem Solution |
| `PERM_DELETE_PROBLEM_SOLUTION` | 20 | `1048576` | Problem Solution |
| `PERM_DELETE_PROBLEM_SOLUTION_SELF` | 21 | `2097152` | Problem Solution |
| `PERM_REPLY_PROBLEM_SOLUTION` | 22 | `4194304` | Problem Solution |
| `PERM_EDIT_PROBLEM_SOLUTION_REPLY_SELF` | 24 | `16777216` | Problem Solution |
| `PERM_DELETE_PROBLEM_SOLUTION_REPLY` | 25 | `33554432` | Problem Solution |
| `PERM_DELETE_PROBLEM_SOLUTION_REPLY_SELF` | 26 | `67108864` | Problem Solution |
| `PERM_VIEW_DISCUSSION` | 27 | `134217728` | Discussion |
| `PERM_CREATE_DISCUSSION` | 28 | `268435456` | Discussion |
| `PERM_HIGHLIGHT_DISCUSSION` | 29 | `536870912` | Discussion |
| `PERM_EDIT_DISCUSSION` | 30 | `1073741824` | Discussion |
| `PERM_EDIT_DISCUSSION_SELF` | 31 | `2147483648` | Discussion |
| `PERM_DELETE_DISCUSSION` | 32 | `4294967296` | Discussion |
| `PERM_DELETE_DISCUSSION_SELF` | 33 | `8589934592` | Discussion |
| `PERM_REPLY_DISCUSSION` | 34 | `17179869184` | Discussion |
| `PERM_EDIT_DISCUSSION_REPLY_SELF` | 36 | `68719476736` | Discussion |
| `PERM_DELETE_DISCUSSION_REPLY` | 38 | `274877906944` | Discussion |
| `PERM_DELETE_DISCUSSION_REPLY_SELF` | 39 | `549755813888` | Discussion |
| `PERM_DELETE_DISCUSSION_REPLY_SELF_DISCUSSION` | 40 | `1099511627776` | Discussion |
| `PERM_PIN_DISCUSSION` | 61 | `2305843009213693952` | Discussion |
| `PERM_ADD_REACTION` | 62 | `4611686018427387904` | Discussion |
| `PERM_LOCK_DISCUSSION` | 64 | `18446744073709551616` | Discussion |
| `PERM_VIEW_CONTEST` | 41 | `2199023255552` | Contest |
| `PERM_VIEW_CONTEST_SCOREBOARD` | 42 | `4398046511104` | Contest |
| `PERM_VIEW_CONTEST_HIDDEN_SCOREBOARD` | 43 | `8796093022208` | Contest |
| `PERM_CREATE_CONTEST` | 44 | `17592186044416` | Contest |
| `PERM_ATTEND_CONTEST` | 45 | `35184372088832` | Contest |
| `PERM_EDIT_CONTEST` | 50 | `1125899906842624` | Contest |
| `PERM_EDIT_CONTEST_SELF` | 51 | `2251799813685248` | Contest |
| `PERM_VIEW_HIDDEN_CONTEST` | 68 | `295147905179352825856` | Contest |
| `PERM_VIEW_HOMEWORK` | 52 | `4503599627370496` | Homework |
| `PERM_VIEW_HOMEWORK_SCOREBOARD` | 53 | `9007199254740992` | Homework |
| `PERM_VIEW_HOMEWORK_HIDDEN_SCOREBOARD` | 54 | `18014398509481984` | Homework |
| `PERM_CREATE_HOMEWORK` | 55 | `36028797018963968` | Homework |
| `PERM_ATTEND_HOMEWORK` | 56 | `72057594037927936` | Homework |
| `PERM_EDIT_HOMEWORK` | 57 | `144115188075855872` | Homework |
| `PERM_EDIT_HOMEWORK_SELF` | 58 | `288230376151711744` | Homework |
| `PERM_VIEW_HIDDEN_HOMEWORK` | 69 | `590295810358705651712` | Homework |
| `PERM_VIEW_TRAINING` | 46 | `70368744177664` | Training |
| `PERM_CREATE_TRAINING` | 47 | `140737488355328` | Training |
| `PERM_EDIT_TRAINING` | 48 | `281474976710656` | Training |
| `PERM_EDIT_TRAINING_SELF` | 49 | `562949953421312` | Training |
| `PERM_PIN_TRAINING` | 63 | `9223372036854775808` | Training |
| `PERM_VIEW_RANKING` | 59 | `576460752303423488` | Ranking |
| `PERM_NEVER` | 60 | `1152921504606846976` | Placeholder（永不允许） |
| `PERM_ALL` | — | `-1` | Placeholder（全部权限） |
| `PERM_ADMIN` | — | `-1` | Placeholder（= `PERM_ALL`） |

组合常量（在 `permission.ts` 末尾计算，`packages/common/permission.ts:104-170`）：

| 常量 | 值 | 组成 |
|---|---|---|
| `PERM_BASIC` | `37483536664552833153` | `PERM_VIEW \| PERM_VIEW_PROBLEM \| PERM_VIEW_PROBLEM_SOLUTION \| PERM_VIEW_PROBLEM_SOLUTION_ACCEPT \| PERM_VIEW_DISCUSSION \| PERM_VIEW_CONTEST \| PERM_VIEW_CONTEST_SCOREBOARD \| PERM_VIEW_HOMEWORK \| PERM_VIEW_HOMEWORK_SCOREBOARD \| PERM_VIEW_TRAINING \| PERM_VIEW_RANKING` |
| `PERM_DEFAULT` | `1370624076369558733505` | `PERM_VIEW \| PERM_VIEW_USER_PRIVATE_INFO \| PERM_VIEW_PROBLEM \| PERM_EDIT_PROBLEM_SELF \| PERM_SUBMIT_PROBLEM \| PERM_VIEW_PROBLEM_SOLUTION \| PERM_VIEW_PROBLEM_SOLUTION_ACCEPT \| PERM_CREATE_PROBLEM_SOLUTION \| PERM_VOTE_PROBLEM_SOLUTION \| PERM_EDIT_PROBLEM_SOLUTION_SELF \| PERM_DELETE_PROBLEM_SOLUTION_SELF \| PERM_REPLY_PROBLEM_SOLUTION \| PERM_EDIT_PROBLEM_SOLUTION_REPLY_SELF \| PERM_DELETE_PROBLEM_SOLUTION_REPLY_SELF \| PERM_VIEW_DISCUSSION \| PERM_CREATE_DISCUSSION \| PERM_EDIT_DISCUSSION_SELF \| PERM_REPLY_DISCUSSION \| PERM_ADD_REACTION \| PERM_EDIT_DISCUSSION_REPLY_SELF \| PERM_DELETE_DISCUSSION_REPLY_SELF \| PERM_DELETE_DISCUSSION_REPLY_SELF_DISCUSSION \| PERM_VIEW_CONTEST \| PERM_VIEW_CONTEST_SCOREBOARD \| PERM_ATTEND_CONTEST \| PERM_EDIT_CONTEST_SELF \| PERM_VIEW_HOMEWORK \| PERM_VIEW_HOMEWORK_SCOREBOARD \| PERM_ATTEND_HOMEWORK \| PERM_EDIT_HOMEWORK_SELF \| PERM_VIEW_TRAINING \| PERM_CREATE_TRAINING \| PERM_EDIT_TRAINING_SELF \| PERM_VIEW_RANKING \| PERM_VIEW_RECORD` |

> 注意：`PERM_BASIC`、`PERM_DEFAULT`、`PERM_ADMIN`、`PERM_ALL`、`PERM_NEVER` 在 `PERM` 对象中初始为占位值，随后被重新赋值（`PERM_BASIC`/`PERM_DEFAULT` 为并集，`PERM_ADMIN = PERM_ALL = -1n`）。

#### `PRIV` 常量完整对照表

> 定义：`packages/common/permission.ts:172-202`（`PRIV` 为普通 number，`1 << n` 表示第 n 位）。

| 常量 | 位 | 值 | 说明 |
|---|---|---|---|
| `PRIV_NONE` | — | `0` | 无任何特权 |
| `PRIV_EDIT_SYSTEM` | 0 | `1` | 编辑系统设置（原 `PRIV_SET_PRIV`），`/manage/*` 全部依赖它 |
| `PRIV_SET_PERM` | 1 | `2` | 设置域内权限 |
| `PRIV_USER_PROFILE` | 2 | `4` | 已注册且可登录（`PRIV_DEFAULT` 的核心位） |
| `PRIV_REGISTER_USER` | 3 | `8` | 注册用户 |
| `PRIV_READ_PROBLEM_DATA` | 4 | `16` | 读取题目数据 |
| `PRIV_READ_RECORD_CODE` | 7 | `128` | 读取评测记录代码 |
| `PRIV_VIEW_HIDDEN_RECORD` | 8 | `256` | 查看隐藏记录 |
| `PRIV_JUDGE` | 9 | `512` | 评测权限（judge） |
| `PRIV_CREATE_DOMAIN` | 10 | `1024` | 创建域 |
| `PRIV_VIEW_ALL_DOMAIN` | 11 | `2048` | 查看所有域（跳过 `PERM_VIEW` 检查） |
| `PRIV_MANAGE_ALL_DOMAIN` | 12 | `4096` | 管理所有域（域内角色被视为 `root`） |
| `PRIV_REJUDGE` | 13 | `8192` | 重测 |
| `PRIV_VIEW_USER_SECRET` | 14 | `16384` | 查看用户机密信息 |
| `PRIV_VIEW_JUDGE_STATISTICS` | 15 | `32768` | 查看评测统计 |
| `PRIV_CREATE_FILE` | 16 | `65536` | 创建用户文件 |
| `PRIV_UNLIMITED_QUOTA` | 17 | `131072` | 不限文件配额 |
| `PRIV_DELETE_FILE` | 18 | `262144` | 删除用户文件 |
| `PRIV_NEVER` | 20 | `1048576` | 永不允许 |
| `PRIV_UNLIMITED_ACCESS` | 22 | `4194304` | 不限访问频率（跳过 `limitRate`） |
| `PRIV_VIEW_SYSTEM_NOTIFICATION` | 23 | `8388608` | 查看系统通知 |
| `PRIV_SEND_MESSAGE` | 24 | `16777216` | 发送站内信 |
| `PRIV_MOD_BADGE` | 25 | `33554432` | 显示管理徽章（`discussion_main` 的 `all` 参数依赖它） |
| `PRIV_ALL` | — | `-1` | 全部特权 |
| `PRIV_DEFAULT` | — | `16842756` | `PRIV_USER_PROFILE + PRIV_CREATE_FILE + PRIV_SEND_MESSAGE`（源码使用 `+` 相加，位不重叠故等价于按位或） |

#### `Setting` 结构

> 定义：`packages/hydrooj/src/interface.ts:49-60`；构造见 `packages/hydrooj/src/model/setting.ts:46-71`（`Setting()`）。
> `domain_edit` 返回的 `settings` 为 `DOMAIN_SETTINGS`，`manage_setting` 返回的 `settings` 为 `SYSTEM_SETTINGS`，元素结构相同。

| 字段 | 类型 | 说明 |
|---|---|---|
| `family` | string | 分组名（如 `setting_domain`、`setting_limits`、`setting_server`） |
| `key` | string | 键名（可为 `a.b` 形式的嵌套键） |
| `value` | any | 默认值 |
| `type` | string | 控件类型：`text` / `yaml` / `number` / `float` / `markdown` / `password` / `boolean` / `textarea` / `select` / `json` |
| `subType` | string | 子类型，目前仅 `yaml` |
| `name` | string | 显示名 |
| `desc` | string | 描述 |
| `flag` | number | 标志位：`FLAG_HIDDEN = 1`、`FLAG_DISABLED = 2`、`FLAG_SECRET = 4`、`FLAG_PRO = 8`、`FLAG_PUBLIC = 16`、`FLAG_PRIVATE = 32`（`packages/hydrooj/src/model/setting.ts:29-34`） |
| `range` | `[string, string][] \| Record<string, string>` | `select` 类型的选项 |
| `validation` | function | 可选的额外校验函数 |

---

## 6. JSON-RPC 接口与 UI 辅助接口

### JSON-RPC 接口体系

Hydro 在传统「页面路由」（`ctx.Route`）之外，另有一套以「操作（operation）」为单位的 RPC 式接口层，实现在 `framework/framework/api.ts`。它统一提供 **HTTP 单次调用** 与 **WebSocket 长连接（订阅 + RPC）** 两种传输方式，并内置了基于 schemastery 的输入校验与基于 `projection` 的响应裁剪。

#### 设计概览

- 所有操作集中注册在模块级全局表 `APIS`（`Record<string, ApiCall>`）中，以操作名（key）索引。
- 每个操作由四部分组成：**类型**（`Query` / `Mutation` / `Subscription`）、**输入 schema**（schemastery `Schema`）、**执行函数** `func`、**前置钩子** `hooks`。
- 传输层由框架自动注册两条路由：HTTP 路由与 WebSocket 路由，业务方只需注册操作本身。
- 参数校验、`args` 合并、`projection` 裁剪、错误包装全部由框架在 `ApiService.execute()` 中完成。

```ts
export type ApiType = 'Query' | 'Mutation' | 'Subscription';

export interface ApiCall<Type extends ApiType, Arg, Res, Progress = void> {
    readonly type: Type;
    readonly input: Schema<Arg>;
    readonly func: (Type extends 'Subscription'
        ? (context: any, args: Arg, emit: (payload: Res) => void) => (() => MaybePromise<void>)
        : (context: any, args: Arg) => MaybePromise<Res | AsyncGenerator<Progress, Res, never>>);
    readonly hooks: ApiCall<'Query', Arg, void>[];
}
```

#### 三种操作类型

三种类型由同一个工厂 `_get` 生成，仅 `type` 字段不同：

```ts
export const _get = <Type extends ApiType>(type: Type) => <Arg, Res, Progress = void>(
    schema: Schema<Arg>,
    func: ApiCall<Type, Arg, Res, Progress>['func'],
    hooks: ApiCall<'Query', Arg, void, void>[] = [],
): ApiCall<Type, Arg, Res, Progress> => ({ input: schema, func, hooks, type } as const);

export const Query = _get('Query');
export const Mutation = _get('Mutation');
export const Subscription = _get('Subscription');
```

| 类型 | 语义 | HTTP | WebSocket 直连（`op=<op>`） | WebSocket RPC（`op=rpc`） | 返回值形态 |
| --- | --- | --- | --- | --- | --- |
| `Query` | 只读查询，无副作用 | ✅ GET / POST | ❌（仅允许 `Subscription`） | ❌（仅允许 `Subscription`） | `Res` |
| `Mutation` | 有副作用写操作 | ⚠️ 仅 POST（GET 报 `BadRequestError`） | ❌ | ❌ | `Res` 或 `AsyncGenerator<Progress, Res>` |
| `Subscription` | 订阅推送 | ❌（直接 `BadRequestError`） | ✅ 通过 `emit` 持续推送 | ⚠️ 见下文 | 返回清理函数 `() => void` |

要点：

- **`Query`**：`func(context, args)` 返回结果；`context` 是发起调用的 handler 实例（HTTP 下为 `ApiHandler`），因此可访问 `context.user`、`context.domain`、`context.checkPerm()` 等。
- **`Mutation`**：语义上必须有副作用，框架强制要求 POST，避免被 GET 缓存/预取误触发。若 `func` 是异步生成器（`async function*`），框架会自动 drain：每次 `yield` 的值作为「进度」推送给 `sendPayload`（仅订阅场景存在该回调），`return` 的值作为最终结果。
- **`Subscription`**：`func(context, args, emit)` 返回一个清理函数；`emit(payload)` 每调用一次就向客户端推送一条消息。清理函数被保存为 `ApiConnectionHandler.dispose`，在连接关闭时由 `cleanup()` 调用。
- **`hooks`**：`_get` 的可选第三个参数，类型为 `ApiCall<'Query', Arg, void>[]`。`ApiService.execute()` 会在主体执行之前逐个 `await this.execute(context, hook, rawArgs)`（`ApiService` 源码注释为 `// eslint-disable-next-line no-await-in-loop`）。当前仓库内置操作均未使用该参数。

#### 注册方式：`ctx.api.provide`

`ApiService` 以 `'api'` 为服务名注册到 cordis 上下文：

```ts
export class ApiService extends Service {
    constructor(ctx: Context) {
        super(ctx, 'api');
    }

    provide(calls: Partial<FlattenedApis>, atNameSpace = '') {
        this.ctx.effect(() => {
            for (const key in calls) {
                const target = `${atNameSpace ? `${atNameSpace}.` : ''}${key}`;
                if (APIS[target]) console.warn(`API ${target} already exists, will be overridden`);
                APIS[target] = calls[key];
            }
            return () => {
                for (const key in calls) {
                    delete APIS[`${atNameSpace ? `${atNameSpace}.` : ''}${key}`];
                }
            };
        });
    }
}

declare module 'cordis' {
    interface Context {
        api: ApiService;
    }
}
```

- **命名空间拼接规则**：`target = (atNameSpace ? atNameSpace + '.' : '') + key`。即 `api.provide(X, 'user')` 会把 `X` 中的 `foo` 注册为 `user.foo`；`atNameSpace` 为空串（默认）时 key 即最终操作名。
- **重复注册**：若 `APIS[target]` 已存在，只 `console.warn('API ... already exists, will be overridden')`，随后直接覆盖，不抛错。
- **生命周期**：注册写在 `ctx.effect()` 内，返回的清理函数会在插件卸载时按同一命名空间规则删除对应 key。
- **调用范式**（本仓库三处注册点全部使用该写法，且**都不传命名空间**）：

```ts
await ctx.inject(['api'], ({ api }) => {
    api.provide(UserApi);
});
```

- **类型扩展**：通过 TS 声明合并把操作表挂到 `Apis` 接口上，以获得 `Partial<FlattenedApis>` 的类型检查：

```ts
declare module '@hydrooj/framework' {
    interface Apis {
        user: typeof UserApi;
    }
}
```

#### 内置操作 `query.batch` / `mutation.batch`

```ts
export const APIS = {
    'query.batch': Query(Schema.array(Schema.object({ op: Schema.string(), args: Schema.any() })), () => ({})),
    'mutation.batch': Mutation(Schema.array(Schema.object({ op: Schema.string(), args: Schema.any() })), () => ({})),
} as const;

export interface Apis {
    builtin: {
        'query.batch': ApiCall<'Query', { op: string, args: any }[], { [key: string]: any }>;
        'mutation.batch': ApiCall<'Mutation', { op: string, args: any }[], { [key: string]: any }>;
    };
    test: typeof TestApis;
}
```

| 项目 | 值 |
| --- | --- |
| 操作名 | `query.batch` / `mutation.batch` |
| 类型 | `Query` / `Mutation` |
| 输入 | `Array<{ op: string, args: any }>` |
| 输出 | `{ [key: string]: any }` |
| 实现 | 占位函数 `() => ({})`，**当前仓库中没有任何 `api.provide` 覆盖它们** |

注意：这两个内置操作的 `func` 恒返回空对象 `{}`，仅作为「批量调用」的协议占位与类型声明。仓库内不存在覆盖实现，因此直接调用会得到 `{}`。要真正实现批量语义，需要由插件通过 `api.provide({ 'query.batch': ... })` 覆盖（会触发上述「already exists」告警）。

#### HTTP 入口

框架提供统一的挂载函数：

```ts
export async function applyApiHandler(ctx: Context, name: string, path: string) {
    ctx.plugin(ApiService);
    await ctx.inject(['server', 'api'], ({ Route, Connection }) => {
        Route(name, path, ApiHandler);
        Connection(`${name}_conn`, `${path}/conn`, ApiConnectionHandler);
    });
}
```

Hydro 在 `packages/hydrooj/src/service/server.ts` 约 111 行调用：

```ts
applyApiHandler(childContext, 'api', '/api/:op');
```

由此注册：

| 路由名 | 路径 | 处理器 | 说明 |
| --- | --- | --- | --- |
| `api` | `/api/:op` | `ApiHandler` | HTTP 调用入口，`op` 为路径参数 |
| `api_conn` | `/api/:op/conn` | `ApiConnectionHandler` | WebSocket 连接入口；**`op` 是路径参数**（**不是** `/api/conn`）；`op=rpc` 时为通用 RPC 通道 |

**完整 URL（含域前缀）**

`packages/hydrooj/src/service/layers/domain.ts` 会在路由匹配前剥离 `/d/:domainId/` 前缀，并把域 ID 写入 `ctx.domainId` / `ctx.HydroContext.args.domainId`：

```ts
const forceDomain = /^\/d\/([^/]+)\//.exec(ctx.request.path);
ctx.originalPath = ctx.request.path;
ctx.path = ctx.request.path = ctx.request.path.replace(/^\/d\/[^/]+\//, '/');
// ...
ctx.domainId = inferDomain?._id || domainId;
```

因此可用的 URL 形式为：

| 形式 | URL | 域解析方式 |
| --- | --- | --- |
| 系统域（显式） | `POST /d/system/api/:op` | 路径中的 `system` |
| 指定域 | `POST /d/:domainId/api/:op` | 路径中的 `:domainId` |
| 省略前缀 | `POST /api/:op` | `DomainModel.getByHost(request.host)` 反查，默认 `system` |

WebSocket 同理：`ws(s)://<host>/d/:domainId/api/:op/conn` 或 `ws(s)://<host>/api/:op/conn`。

> ⚠️ **WebSocket 路径中 `op` 是路径参数，不是查询参数。** 连接路径是 `/api/:op/conn`（**不是** `/api/conn`，也不是 `/api?op=...`），因此订阅操作的正确写法是 `/d/:domainId/api/<操作名>/conn`，例如 `wss://oj.example.com/d/system/api/test.subscription/conn`。RPC 通道则写成 `/d/:domainId/api/rpc/conn`（即 `op` 取值为字符串 `rpc`）。连接建立后，客户端只能通过 WebSocket 消息体（而非 URL）继续指定操作，见下文 `message()`。

> 补充：当 `WebService.Config.enableSSE` 为 `true` 时，`WebService.register('conn', ...)` 会为同一条连接路径额外注册一条 **GET** 路由（路由名同为 `api_conn`，路径同为 `/api/:op/conn`），走 Server-Sent Events 通道；此时通过 `?sse=on` 让每条消息带上 `data: ` 前缀。相关实现见 `framework/framework/server.ts` 的 `register()` 与 `handleWS()`。

#### `ApiHandler.all` 调用协议

HTTP 入口由 `ApiHandler` 处理，方法 `all` 响应所有 HTTP 方法，再自行判断合法性：

```ts
export class ApiHandler extends Handler {
    @param('op', Types.String)
    async all({ }, op: string) {
        if (!['get', 'post'].includes(this.request.method.toLowerCase())) {
            throw new MethodNotAllowedError(this.request.method);
        }
        if (!APIS[op]) throw new BadRequestError(`Invalid API operation: ${op}`);
        if (APIS[op].type === 'Subscription') {
            throw new BadRequestError('Subscription operation cannot be called in HTTP handler');
        }
        if (APIS[op].type === 'Mutation' && this.request.method.toLowerCase() === 'get') {
            throw new BadRequestError('Mutation operation cannot be called with GET method');
        }
        handleArguments(this.args);
        await this.ctx.parallel('handler/api/before', this);
        await this.ctx.parallel(`handler/api/before/${op}`, this);
        const result = await this.ctx.api.execute(
            this, op, { domainId: this.args.domainId, ...this.args, ...(this.args.args || {}) },
            (m, args) => (this.ctx.parallel as any)(m, args), this.args.projection,
        );
        if (BinaryResponse.check(result)) {
            this.binary(result.data, result.filename);
        } else if (RedirectResponse.check(result)) {
            this.response.redirect = result.url;
        } else {
            this.response.body = result;
        }
    }
}
```

**1）参数来源**

`this.args` 由 `packages/hydrooj/src/service/layers/base.ts` 构造，`handleHttp()` 再补一次路径参数：

```ts
const args = { domainId, ...ctx.params, ...ctx.query, ...ctx.request.body, __start: Date.now() };
// framework/framework/server.ts → handleHttp()
Object.assign(args, ctx.params);
```

即 `this.args` 最终包含：域 ID、路径参数（`op`）、查询串、请求体（含 JSON body）、以及框架内部时间戳 `__start`。

- `op` 通过装饰器 `@param('op', Types.String)` 从合并后的 `args` 中读取（`@param` 的 source 为 `'all'`，即从整体 args 取），因此既可以来自路径 `:op`，也可以来自查询串/请求体（路径参数优先级最高）。
- `args` 与 `projection` 是两个约定的「信封」字段（见下）。

**2）`args` / `projection` 的解析**

```ts
function handleArguments(args: any) {
    try {
        if (typeof args.args === 'string') {
            args.args = JSON.parse(args.args);
        }
        if (typeof args.projection === 'string') {
            args.projection = '{['.includes(args.projection[0])
                ? JSON.parse(args.projection)
                : args.projection.split(',').map((i) => i.trim()).filter((i) => i);
        }
    } catch (e) {
        throw new BadRequestError('Invalid arguments');
    }
}
```

- `args`：字符串时自动 `JSON.parse`；解析失败抛 `BadRequestError('Invalid arguments')`。已是对象/数组时原样保留。
- `projection`：字符串时，若首字符是 `{` 或 `[` 则按 JSON 解析，否则按逗号分隔为字符串数组；解析失败抛 `BadRequestError('Invalid arguments')`。

**3）业务参数合并规则**

```ts
{ domainId: this.args.domainId, ...this.args, ...(this.args.args || {}) }
```

优先级（后者覆盖前者）：

1. `domainId`（来自域层解析）
2. 顶层 query / body 参数（含 `args`、`projection` 这两个信封字段本身）
3. `this.args.args` 中的键（**最高优先级**）

合并后的对象交给 schemastery `input()` 校验。注意 schemastery 的 `object()` 在非 strict 模式下会**保留 schema 未声明的键**：

```ts
Schema.extend('object', (data, { dict }, options, strict) => {
  if (!isPlainObject(data)) throw new ValidationError(`expected object but got ${data}`, options)
  const result: any = {}
  for (const key in dict) { /* 逐个校验声明的键 */ }
  if (!strict) merge(result, data)   // merge: 把 data 中 result 没有的键浅拷贝进去
  return [result]
})
```

`Schema.prototype` 的调用形式是 `Schema.resolve(data, schema, options)`，`strict` 为 `undefined`（falsy），因此 `merge(result, data)` 会执行。也就是说业务函数最终收到的参数对象 = **已校验/已转换的声明字段** + **原样保留的未声明字段**（如 `args`、`projection`、`op`、`domainId`、`__start`）。框架自身依赖这一点：`DomainApi['domain.current']` 使用 `Schema.object({})`，仍能通过 `handler` 访问上下文。

校验失败时抛 `BadRequestError(e.message)`（schemastery 的 `ValidationError` 消息，形如 `$ 参数名 expected string but got ...`）。

**4）方法限制与错误触发条件**

| 顺序 | 条件 | 抛出 | HTTP 状态 |
| --- | --- | --- | --- |
| 1 | 方法不是 `GET`/`POST` | `MethodNotAllowedError(this.request.method)` | 405 |
| 2 | `op` 未注册（`!APIS[op]`） | `BadRequestError('Invalid API operation: ' + op)` | 400 |
| 3 | `op` 类型为 `Subscription` | `BadRequestError('Subscription operation cannot be called in HTTP handler')` | 400 |
| 4 | `op` 类型为 `Mutation` 且方法为 GET | `BadRequestError('Mutation operation cannot be called with GET method')` | 400 |
| 5 | `args`/`projection` 解析失败 | `BadRequestError('Invalid arguments')` | 400 |
| 6 | 输入 schema 校验失败 | `BadRequestError(<schemastery 消息>)` | 400 |
| 7 | 未登录 / 无权限 / 找不到 | `PrivilegeError` / `PermissionError` / `NotFoundError` | 403 / 403 / 404 |

> 注意：`ApiHandler` 没有声明 `noCheckPermView`，因此 `handler/create/http` 钩子会对其执行 `checkPerm(PERM.PERM_VIEW)`（拥有 `PRIV.PRIV_VIEW_ALL_DOMAIN` 的用户跳过），并且默认走 `limitRate('global', 5, 100)` 限流。也就是说 **`/api/:op` 的所有操作都要求当前域的 `PERM.PERM_VIEW`**，此外还会执行 `Handler.init()` 的 CSRF 校验（POST 时 referer host 必须与 request host 一致，否则抛 `CsrfTokenError`，403）。

**5）钩子**

- `handler/api/before`、`handler/api/before/${op}`：在参数校验之后、业务函数执行之前，通过 `ctx.parallel()` 触发，参数为 handler 实例（`this`）。用于审计、计数、埋点等。

**6）返回值类型**

`ApiService.execute()` 的返回值会被 `ApiHandler.all` 分派：

| 返回值 | 处理 | 响应 |
| --- | --- | --- |
| 普通对象 / 数组 / 标量 / `null` | `this.response.body = result` | JSON（`Accept: application/json` 时）或渲染模板 |
| `BinaryResponse` | `this.binary(result.data, result.filename)` | `application/octet-stream`，若给了 `filename` 则附加 `Content-Disposition: attachment; filename="..."` |
| `RedirectResponse` | `this.response.redirect = result.url` | JSON 请求：`{ "url": "<url>" }`；非 JSON：HTTP 302 跳转 |

```ts
export class BinaryResponse {
    [BINARY] = true;
    constructor(public readonly data: Buffer, public filename: string) { }
    static check(value: any): value is BinaryResponse {
        return value && typeof value === 'object' && BINARY in value && value[BINARY] === true;
    }
}

export class RedirectResponse {
    [REDIRECT] = true;
    constructor(public readonly url: string) { }
    static check(value: any): value is RedirectResponse {
        return value && typeof value === 'object' && REDIRECT in value && value[REDIRECT] === true;
    }
}
```

两个类分别以 `Symbol.for('hydro.api.response.binary')` 和 `Symbol.for('hydro.api.response.redirect')` 作为标记，使用全局 Symbol 注册表，因此跨包、跨 bundle 副本也能被 `check()` 正确识别。

**7）JSON 错误响应结构**

JSON 请求（`Accept: application/json`）下，错误由 `packages/hydrooj/src/service/server.ts` 的 `httpHandlerMixin.onerror` 构造：

```ts
this.response.status = error instanceof UserFacingError ? error.code : 500;
this.response.template = error instanceof UserFacingError ? 'error.html' : 'bsod.html';
this.response.body = {
    UserFacingError,
    error: {
        message: error.msg(), stack: errorMessage(error.stack || ''),
        params: error.params, name: error.name, code: error.code,
    },
    _rawError: error,
};
```

经 `serializer` 处理（剔除 `_` 前缀键，因此 `_rawError` 被移除；类/函数值被 JSON 丢弃）后，客户端实际收到：

```json
{
  "error": {
    "message": "BadRequestError",
    "stack": "",
    "params": ["Invalid API operation: foo"],
    "name": "BadRequestError",
    "code": 400
  }
}
```

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `error.name` | string | 错误类名，如 `BadRequestError` / `NotFoundError` / `MethodNotAllowedError` / `ValidationError` / `PermissionError` / `PrivilegeError` / `CsrfTokenError` |
| `error.message` | string | `error.msg()` 的返回值 |
| `error.params` | any[] | 构造错误时传入的参数，用于填充 `{0}` / `{1}` / `{2}` 占位符 |
| `error.code` | number | `UserFacingError` 及其子类的 HTTP 状态码（400 / 403 / 404 / 405 …）；非用户可见错误为 500 |
| `error.stack` | string | 已清理过的堆栈；生产环境下用户可见错误的 `stack` 会被清空 |

关于 `message` 与 `params` 的分工（见 `framework/framework/error.ts` 的 `Err()` 工厂）：

- 固定文案错误（如 `BadRequestError`、`NotFoundError`、`MethodNotAllowedError`）的 `msg()` 返回**错误名本身**，具体信息放在 `params` 里。例如 `new BadRequestError('Invalid arguments')` → `message: "BadRequestError"`、`params: ["Invalid arguments"]`。
- 带占位符的错误（如 `ValidationError`、`PermissionError`）的 `msg()` 返回模板字符串，如 `'Field {0} validation failed.'`，由前端用 `i18n(message, ...params)` 做替换。
- 前端 `request` 封装的兜底逻辑：当 i18n 未命中且 `params.length` 非零时，显示为 `"<message>: <params.join(' ')>"`。

#### `projection` 响应裁剪机制

**输入格式**（`handleArguments` 归一化后统一为「对象」或「字符串数组」）：

| 客户端写法 | 归一化结果 | 说明 |
| --- | --- | --- |
| `"1"` / `1`（作为 schema 叶子值） | `1` | 表示「取该键的完整值」 |
| `"docId,pid,title"` | `['docId', 'pid', 'title']` | 逗号分隔字符串，自动 `trim()` 并过滤空项 |
| `'["docId","pid"]'` | `['docId', 'pid']` | 以 `[` 开头的字符串按 JSON 解析 |
| `'{"docId":1,"data":{"name":1}}'` | `{ docId: 1, data: { name: 1 } }` | 以 `{` 开头的字符串按 JSON 解析 |
| 原生数组 / 对象（JSON body 中） | 原样使用 | 推荐写法 |

**裁剪函数**：

```ts
export const projection = <T, S extends ProjectionSchema<T>>(input: T, schema: S, serializeCtx?: any): Projection<T, S> => {
    if (typeof input !== 'object' || input === null) throw new Error('Input must be an object.');
    type R = Projection<T, S>;
    if ('serialize' in input && typeof (input as any).serialize === 'function') {
        input = (input as any).serialize(serializeCtx);
    }
    if (Array.isArray(schema)) schema = Object.fromEntries(schema.map((s) => [s, 1])) as S;
    if (Array.isArray(input)) {
        return input.map((item) => projection(item, schema, serializeCtx)) as R;
    }
    const result = {} as R;
    for (const key of Reflect.ownKeys(input as any)) {
        const schemaIt = schema[key];
        if (!schemaIt) continue;
        if (schemaIt === 1 || !input[key]) result[key] = input[key];
        else result[key] = projection(input[key], schemaIt, serializeCtx);
    }
    return result;
};
```

裁剪规则逐条说明：

1. 输入必须是对象（`typeof input !== 'object' || input === null` 时直接抛 `Error('Input must be an object.')`）。
2. 若输入对象自身带有 `serialize` 方法，**先调用 `input.serialize(serializeCtx)`**，再对返回值裁剪。`serializeCtx` 由 `ApiService.execute()` 传入执行上下文（HTTP 下即 `ApiHandler` 实例），因此 `User.serialize(h)` 可以依据 `h.user.hasPerm(...)` 决定是否输出私有字段。
3. schema 为数组时转换为 `{ key: 1 }` 形式（即「只取这些键，值全量保留」）。
4. 输入为数组时，对**每个元素**递归裁剪（schema 保持不变），返回新数组。
5. 遍历 `Reflect.ownKeys(input)`：**只保留输入对象实际存在的键**，schema 中声明但输入中不存在的键不会出现在结果里（测试用例 `non-exist` 覆盖此行为）。使用 `Reflect.ownKeys` 也意味着原型链上的属性不参与裁剪，且 `__proto__` / `prototype` 之类的恶意键不会被读取（`framework/framework/tests/projection.spec.ts` 的 `safety` 用例覆盖）。
6. `schemaIt === 1` **或** `input[key]` 为 falsy（`false` / `0` / `''` / `null` / `undefined`）时，直接赋原值，不再下钻。
7. 否则递归下钻（支持嵌套对象、对象数组）。
8. 返回全新对象，不修改入参。

**调用时机**（`ApiService.execute()` 末尾）：

```ts
return (project && typeof result === 'object' && result !== null) ? projection(result, project, context) : result;
```

注意：

- `projection` 在业务函数返回**之后**执行，因此业务侧始终拿到完整数据。
- 顶层结果不是对象（字符串 / 数字 / `null` / `undefined`）时 `projection` 不生效，原值直接返回。
- 数组结果会被裁剪（对每个元素应用同一 schema）。
- `Subscription` 通过 `emit` 推送的 payload **不经过** `projection`；只有 `execute` 的最终返回值会走裁剪。
- 未传 `projection` 时结果原样返回；但最终写入 HTTP 响应体时仍会经过 `serializer`（剔除 `_` 前缀键、`bigint` 转 `"BigInt::<n>"`、调用对象自身的 `serialize()`）。

**`serialize()` 的调用**：

- `projection()` 内部调用：仅当输入对象带 `serialize` 方法时。
- HTTP 响应序列化时：`framework/framework/serializer.ts`

```ts
export default function serializer(ignoreSerializeFunction = false, h?: HandlerCommon) {
    return (k: string, v: any) => {
        if (k.startsWith('_') && k !== '_id') return undefined;
        if (typeof v === 'bigint') return `BigInt::${v.toString()}`;
        if (!ignoreSerializeFunction && v && typeof v === 'object'
            && 'serialize' in v && typeof v.serialize === 'function') return v.serialize(h);
        return v;
    };
}
```

即：键名以 `_` 开头（`_id` 除外）的属性会被丢弃；`bigint` 序列化为字符串 `BigInt::<n>`；带 `serialize` 方法的对象会以 handler 实例为参数调用 `serialize(h)`。

#### 订阅与 WebSocket 连接

```ts
export class ApiConnectionHandler extends ConnectionHandler {
    dispose: () => Promise<void> | void;
    isRpc: boolean;

    @param('op', Types.String)
    async prepare({ }, op: string) {
        if (op === 'rpc') {
            this.isRpc = true;
            return;
        }
        if (!APIS[op]) throw new BadRequestError(`Invalid API operation: ${op}`);
        if (APIS[op].type !== 'Subscription') {
            throw new BadRequestError('Only subscription operations are supported');
        }
        handleArguments(this.args);
        await this.ctx.parallel('handler/api/before', this);
        await this.ctx.parallel(`handler/api/before/${op}`, this);
        this.dispose = await this.ctx.api.execute(
            this, op, { domainId: this.args.domainId, ...this.args, ...(this.args.args || {}) },
            (m, args) => (this.ctx.parallel as any)(m, args), this.args.projection, (p) => this.send(p),
        );
    }

    async message(message) {
        if (!this.isRpc) throw new BadRequestError('Only RPC operations are supported');
        if (typeof message === 'string') {
            try {
                message = JSON.parse(message);
            } catch (e) {
                throw new BadRequestError('Invalid message');
            }
        }
        if (!APIS[message.op]) throw new BadRequestError(`Invalid API operation: ${message.op}`);
        if (APIS[message.op].type !== 'Subscription') {
            throw new BadRequestError('Only subscription operations are supported');
        }
        handleArguments(message);
        const result = await this.ctx.api.execute(
            this, message.op, message.args, (m, args) => (this.ctx.parallel as any)(m, args), message.projection,
        );
        this.send(result);
    }

    async cleanup() {
        await this.dispose?.();
    }
}
```

**`prepare()`：连接建立时**

- **连接 URL 形如 `/d/:domainId/api/:op/conn`（省略域前缀则为 `/api/:op/conn`）**，其中 `:op` 是**路径参数**，由路由 `Connection('api_conn', '/api/:op/conn', ApiConnectionHandler)` 定义。订阅操作必须写成 `/d/:domainId/api/<操作名>/conn`，例如 `/d/system/api/test.subscription/conn`；**不存在 `/api/conn?op=...` 这种形式**。
- `op` 通过 `@param('op', Types.String)` 从 `args`（含路径参数 `:op`）读取。
- `op === 'rpc'` 是**特例**：直接置 `isRpc = true` 并返回，不建立任何订阅，该连接退化为通用 RPC 通道（URL 为 `/d/:domainId/api/rpc/conn`）。
- 非 `rpc` 时：`op` 必须已注册，且类型必须是 `Subscription`，否则抛 `BadRequestError`（连接随后以 code 4000 关闭）。
- 参数解析、`handler/api/before` 钩子与 HTTP 路径一致。
- 业务参数合并规则与 HTTP 完全一致：`{ domainId: this.args.domainId, ...this.args, ...(this.args.args || {}) }`。
- **订阅函数的 `emit` 即 `(p) => this.send(p)`**：每次 `emit` 立即向该连接写出一条 JSON 消息。
- 订阅函数返回的清理函数被赋给 `this.dispose`。

**`message()`：收到客户端消息时（仅 RPC 模式）**

- 非 RPC 连接收到消息 → `BadRequestError('Only RPC operations are supported')`。
- 消息可以是字符串（会 `JSON.parse`，失败抛 `BadRequestError('Invalid message')`）或已解析对象。
- **消息格式**：

```json
{
  "op": "<操作名>",
  "args": { "键": "值" },
  "projection": ["字段A", "字段B"]
}
```

- `message()` 同样只接受 `Subscription` 类型（否则 `BadRequestError('Only subscription operations are supported')`）。
- `handleArguments(message)` 会就地解析 `message.args`（字符串 → JSON）与 `message.projection`（字符串 → JSON / 逗号数组）。
- 调用 `execute()` 时传入的是 `message.args` **原样**（不做 `{domainId, ...}` 合并，也不传 `sendPayload`），随后把 `execute` 的返回值直接 `this.send(result)` 回给客户端。
- ⚠️ 由于 RPC 分支不传 `sendPayload`，订阅函数的第三个参数 `emit` 为 `undefined`，且返回的清理函数不会被保存。因此依赖推送语义的订阅操作（例如 `test.subscription`）**必须**用直连模式 `op=<操作名>`，不能用 `op=rpc`。

**`cleanup()`：连接关闭时**

- `await this.dispose?.()`，释放订阅函数申请的定时器 / 监听器 / 数据库游标等资源。

**连接层的通用行为**（`framework/framework/server.ts` 的 `handleWS()` 与 `packages/ui-default/components/socket/index.ts`）

| 行为 | 说明 |
| --- | --- |
| 心跳 | 服务端每 40s 检查：30s 无活动则发送字符串 `ping`；80s 无活动则主动断开。客户端收到 `ping` 应回 `pong`；客户端也可每 30s 发 `ping`，服务端回 `pong` |
| 压缩 | URL 带 `?shorty=on` 时，服务端先发送字符串 `shorty`，之后所有 payload 用 `shorty.js` 压缩（`ConnectionHandler.send()` 中 `compression.deflate`，计数超过 1000 会重发 `shorty` 重置）。客户端收到 `shorty` 后启用 `inflate` |
| 会话 | 跨域/子域连接时通过 `?sid=<session id>` 传递会话（`components/socket/index.ts` 在 `i.host !== window.location.host` 时自动附加） |
| 错误 | `ConnectionHandler.onerror()` 发送 `{ "error": { "name": "<错误名>", "params": [...] } }`，随后以 close code **4000** 关闭连接 |
| 客户端 | `packages/ui-default/components/socket/index.ts` 导出的 `Sock` 类封装了重连（`reconnecting-websocket`）、心跳、`shorty` 解压；收到 `PermissionError` / `PrivilegeError` 时自动关闭 |

#### `ApiService.serialize()` 自省输出

返回 `{ [操作名]: { type, input } }`，可用于自动生成 API 文档 / 客户端 SDK / 前端表单：

```ts
serialize() {
    const result = {};
    for (const key in APIS) {
        result[key] = {
            type: APIS[key].type,
            input: APIS[key].input.toJSON(),
        };
    }
    return result;
}
```

**顶层结构**：

```jsonc
{
  "<操作名>": {
    "type": "Query" | "Mutation" | "Subscription",
    "input": { "uid": 12, "refs": { /* ... */ } }
  }
}
```

> 说明：`input` 是 **schemastery 自身的 `toJSON()` 结果**，形如 `{ uid, refs }` 的「引用表（ref map）」，**不是标准 JSON Schema**。若要产出 JSON Schema，需要像 `packages/ui-default` 的 `SystemConfigSchemaHandler` 那样再用 `schemastery-jsonschema` 的 `convert()` 转换（见下文 `config_schema` 路由）。

**`input` 结构**：schemastery 3.18 的 `Schema.prototype.toJSON()` 返回的是「引用表（ref map）」而非内联树：

```ts
Schema.prototype.toJSON = function toJSON() {
    if (globalThis.__schemastery_refs__) {
        globalThis.__schemastery_refs__[this.uid] ??= JSON.parse(JSON.stringify({ ...this }));
        return this.uid as any;
    }
    globalThis.__schemastery_refs__ = { [this.uid]: { ...this } as Schema };
    globalThis.__schemastery_refs__[this.uid] = JSON.parse(JSON.stringify({ ...this }));
    const result = { uid: this.uid, refs: globalThis.__schemastery_refs__ };
    globalThis.__schemastery_refs__ = undefined;
    return result;
};
```

- `uid`：根 schema 的唯一编号。
- `refs`：`{ [uid]: node }` 的扁平映射；每个 `node` 是该 schema 自身的可枚举属性（`JSON.parse(JSON.stringify({ ...this }))` 的结果），**子 schema 在 `refs` 中以数字 uid 引用**，从而对共享子 schema 去重。

`node` 常见字段：

| 字段 | 出现于 | 说明 |
| --- | --- | --- |
| `type` | 全部 | schema 类型：`string` / `number` / `boolean` / `object` / `array` / `union` / `intersect` / `transform` / `is` / `any` / `const` / `tuple` / `dict` / `bitset` / `function` 等 |
| `meta` | 全部 | 元信息对象，常见键：`required`、`default`、`min`、`max`、`step`、`role`、`description`、`hidden`、`disabled`、`comment`、`deprecated`、`experimental` 等 |
| `dict` | `object` | `{ [键名]: <子 schema 的 uid> }` |
| `inner` | `array` / `dict` / `transform` | 子 schema 的 uid |
| `sKey` | `dict` | 键 schema 的 uid |
| `list` | `union` / `intersect` / `tuple` | 子 schema uid 数组 |
| `bits` | `bitset` | `{ [位名]: number }` |
| `value` | `const` | 常量值 |
| `constructor` | `is` | 构造函数名（字符串） |
| `callback` | `transform` | 回调函数的字符串形式 |
| `preserve` | `transform` | 布尔值 |

调用方式：`ctx.api.serialize()`（`ApiService` 是 cordis 服务，`ctx.api` 可直接使用）。

#### 客户端调用示例

**HTTP（POST，推荐）**

```bash
curl -X POST 'https://oj.example.com/d/system/api/users' \
  -H 'Accept: application/json' \
  -H 'Content-Type: application/json' \
  --data '{"args":{"search":"admin","limit":5},"projection":["_id","uname","avatarUrl"]}'
```

响应（`users` 返回数组，直接作为响应体）：

```json
[
  { "_id": 2, "uname": "admin", "avatarUrl": "gravatar:..." }
]
```

**HTTP（GET，仅 `Query` 类型可用）**

```bash
curl -G 'https://oj.example.com/d/system/api/problem' \
  -H 'Accept: application/json' \
  --data-urlencode 'args={"id":1000}' \
  --data-urlencode 'projection=docId,pid,title'
```

响应：

```json
{ "docId": 1000, "pid": "P1000", "title": "A + B Problem" }
```

> 用 `Mutation` 走 GET 会返回 400：`{ "error": { "name": "BadRequestError", "message": "BadRequestError", "params": ["Mutation operation cannot be called with GET method"], "code": 400 } }`。

**WebSocket 订阅（直连模式）**

```js
// URL：/d/:domainId/api/:op/conn，op 即操作名
const sock = new WebSocket(`wss://oj.example.com/d/system/api/test.subscription/conn`);

sock.onopen = () => console.log('connected');

sock.onmessage = (ev) => {
  if (ev.data === 'ping') { sock.send('pong'); return; }
  if (ev.data === 'shorty') { /* 后续 payload 需要 shorty.js inflate（仅 ?shorty=on 时） */ return; }
  const payload = JSON.parse(ev.data);
  if (payload.error) {
    console.error(payload.error.name, payload.error.params);
    return;
  }
  console.log('pushed:', payload); // 例如 { count: 1 }、{ count: 2 } ...
};
```

> 订阅参数也可以走查询串：`.../api/test.subscription/conn?args={"initial":10}`（`handleArguments` 会把字符串 `args` 解析成对象）。

**WebSocket RPC（`op=rpc`）**

```js
const sock = new WebSocket(`wss://oj.example.com/d/system/api/rpc/conn`);

sock.onmessage = (ev) => {
  const payload = JSON.parse(ev.data);
  console.log('rpc result:', payload);
};

sock.onopen = () => {
  sock.send(JSON.stringify({
    op: 'test.subscription',   // 注意：RPC 模式只接受 Subscription 类型的操作
    args: { initial: 0 },
    projection: ['count'],
  }));
};
```

**HTTP 中的错误示例**

```json
{
  "error": {
    "message": "BadRequestError",
    "stack": "",
    "params": ["Invalid API operation: not.exist"],
    "name": "BadRequestError",
    "code": 400
  }
}
```

### 已注册的 API 操作参考

下列操作来自 `framework/framework/api.ts` 的 `TestApis` 与 `packages/hydrooj` 的三个 handler 文件。**三处 `api.provide()` 调用均未传命名空间**，因此操作名就是对象字面量里的 key（`user`、`users`、`domain`、`domain.current`、`groups`、`domain.group`、`problem`、`problems`），不存在额外前缀。

所有操作的 HTTP 访问形式统一为：

- `POST /d/:domainId/api/<操作名>`（全部类型，`Subscription` 除外）
- `GET /d/:domainId/api/<操作名>`（仅 `Query`）
- `WS /d/:domainId/api/<操作名>/conn`（仅 `Subscription`）
- `WS /d/:domainId/api/rpc/conn`（RPC 通道）

所有操作在 **HTTP 调用**下都受「域 `PERM.PERM_VIEW` + 全局限流 + CSRF 校验」约束（见前文 `ApiHandler` 说明）；WebSocket 连接只经过 `handler/create/ws`，不做域权限检查。下表中的「域权限」列仅列**操作内部额外检查**的权限。

#### `test.query`

| 项目 | 值 |
| --- | --- |
| 操作名 | `test.query` |
| 类型 | `Query` |
| 注册点 | `framework/framework/api.ts` → `TestApis` → `applyTestApis(ctx)` |
| 处理器 | `ApiHandler.all`（`POST/GET /d/:domainId/api/test.query`） |
| 认证 | 匿名可调用 |
| 域权限 | 无额外检查（仅框架默认的 `PERM.PERM_VIEW`） |
| 响应 | `{ ok: true, name: string }` |

**启用方式**：`applyTestApis` 不会自动执行，需要显式调用：

```ts
export function applyTestApis(ctx: Context) {
    ctx.inject(['api'], ({ api }) => {
        api.provide(TestApis);
    });
}
```

仓库内没有任何调用点，仅用于测试与示例；未启用时访问会返回 `BadRequestError: Invalid API operation: test.query`。

**参数表**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `name` | string | ✅ | — | `Schema.string()`，任意字符串 |

**响应字段表**

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `ok` | boolean | 恒为 `true` |
| `name` | string | 回显入参 `name` |

**示例**

```bash
curl -X POST 'https://oj.example.com/d/system/api/test.query' \
  -H 'Accept: application/json' -H 'Content-Type: application/json' \
  --data '{"args":{"name":"hydro"}}'
# => {"ok":true,"name":"hydro"}
```

#### `test.mutation`

| 项目 | 值 |
| --- | --- |
| 操作名 | `test.mutation` |
| 类型 | `Mutation` |
| 注册点 | `framework/framework/api.ts` → `TestApis` |
| 处理器 | `ApiHandler.all`（仅 `POST /d/:domainId/api/test.mutation`） |
| 认证 | 匿名可调用 |
| 域权限 | 无额外检查 |
| 响应 | `{ ok: true, name: string }` |

**参数表**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `name` | string | ✅ | — | `Schema.string().required()` |

**响应字段表**

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `ok` | boolean | 恒为 `true` |
| `name` | string | 回显入参 `name` |

**示例**

```bash
curl -X POST 'https://oj.example.com/d/system/api/test.mutation' \
  -H 'Accept: application/json' -H 'Content-Type: application/json' \
  --data '{"args":{"name":"hydro"}}'
# => {"ok":true,"name":"hydro"}

# 用 GET 调用会失败：
curl -G 'https://oj.example.com/d/system/api/test.mutation' -H 'Accept: application/json'
# => 400 {"error":{"name":"BadRequestError","params":["Mutation operation cannot be called with GET method"],...}}
```

#### `test.mutation_progress`

| 项目 | 值 |
| --- | --- |
| 操作名 | `test.mutation_progress` |
| 类型 | `Mutation`（异步生成器） |
| 注册点 | `framework/framework/api.ts` → `TestApis` |
| 处理器 | `ApiHandler.all`（仅 `POST`） |
| 认证 | 匿名可调用 |
| 域权限 | 无额外检查 |
| 响应 | 最终值 `{ ok: true, count: number }`；过程中的 `{ progress: number }` 仅在有 `sendPayload` 时推送 |

**参数表**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `count` | number | ❌ | `10` | `Schema.number().step(1).min(1).default(10)`，步长 1、最小值 1 |

**响应字段表**

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `progress` | number | 进度值（`yield` 产出，`1..count`）；HTTP 与 RPC 路径下没有 `sendPayload`，会被丢弃 |
| `ok` | boolean | 最终值，恒为 `true` |
| `count` | number | 最终值，回显入参 `count` |

**实现**

```ts
'test.mutation_progress': Mutation(Schema.object({
    count: Schema.number().step(1).min(1).default(10),
}), async function* (c, { count }) {
    for (let i = 1; i <= count; i++) {
        yield { progress: i };
    }
    return {
        ok: true,
        count,
    };
}),
```

**示例**

```bash
curl -X POST 'https://oj.example.com/d/system/api/test.mutation_progress' \
  -H 'Accept: application/json' -H 'Content-Type: application/json' \
  --data '{"args":{"count":3}}'
# => {"ok":true,"count":3}
```

#### `test.subscription`

| 项目 | 值 |
| --- | --- |
| 操作名 | `test.subscription` |
| 类型 | `Subscription` |
| 注册点 | `framework/framework/api.ts` → `TestApis` |
| 处理器 | `ApiConnectionHandler.prepare`（`WS /d/:domainId/api/test.subscription/conn`） |
| 认证 | 匿名可连接 |
| 域权限 | 无额外检查 |
| 响应 | 每秒推送 `{ count: number }`；HTTP 访问会返回 400 |

**参数表**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `initial` | number | ❌ | `0` | `Schema.number().step(1).min(0).default(0)`，推送计数的起始值 |

**推送字段表**

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `count` | number | 从 `initial + 1` 开始，每秒递增 1 |

**实现**

```ts
'test.subscription': Subscription(Schema.object({
    initial: Schema.number().step(1).min(0).default(0),
}), (c, { initial }, send) => {
    let count = initial;
    const interval = setInterval(() => {
        count++;
        send({ count });
    }, 1000);
    return () => {
        clearInterval(interval);
    };
}),
```

**示例**

```js
const sock = new WebSocket(`wss://oj.example.com/d/system/api/test.subscription/conn?args={"initial":5}`);
sock.onmessage = (ev) => {
  if (ev.data === 'ping') return sock.send('pong');
  console.log(JSON.parse(ev.data)); // { count: 6 }、{ count: 7 } ...
};
```

#### `user`

| 项目 | 值 |
| --- | --- |
| 操作名 | `user` |
| 类型 | `Query` |
| 注册点 | `packages/hydrooj/src/handler/user.ts`（约 573 行 `UserApi`；`apply()` 中 `api.provide(UserApi)`，**无命名空间**） |
| 处理器 | `ApiHandler.all` |
| 认证 | 匿名可调用（不带任何标识参数时返回当前登录用户；未登录则为 `_id = 0` 的匿名用户） |
| 域权限 | 无额外检查（仅框架默认的 `PERM.PERM_VIEW`） |
| 响应 | `User`（经 `User.serialize()` 序列化）或 `null` |

**参数表**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `id` | number | ❌ | — | `Schema.number().step(1)`，用户 UID |
| `uname` | string | ❌ | — | 用户名 |
| `mail` | string | ❌ | — | 邮箱 |
| `domainId` | string | ✅ | — | `Schema.string().required()`，查询所属域 |

**查找优先级**：`id` → `mail` → `uname` → 当前登录用户（`c.user._id`）。

```ts
user: Query(Schema.object({
    id: Schema.number().step(1),
    uname: Schema.string(),
    mail: Schema.string(),
    domainId: Schema.string().required(),
}), (c, arg) => {
    if (arg.id) return user.getById(arg.domainId, arg.id);
    if (arg.mail) return user.getByEmail(arg.domainId, arg.mail);
    if (arg.uname) return user.getByUname(arg.domainId, arg.uname);
    return user.getById(arg.domainId, c.user._id);
}),
```

**响应字段表**（`packages/hydrooj/src/model/user.ts` 的 `User.serialize()` / `getFields()`）

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `_id` | number | UID；匿名用户为 `0` |
| `uname` | string | 用户名 |
| `mail` | string | 邮箱（公开字段，注意隐私） |
| `perm` | string | 域内权限位（bigint），序列化为 `"BigInt::<十进制>"` |
| `role` | string | 角色名，默认 `default` |
| `priv` | number | 全局特权位 |
| `regat` | string(Date) | 注册时间（ISO 字符串） |
| `loginat` | string(Date) | 最后登录时间 |
| `avatar` | string | 头像标识（如 `gravatar:xxx@yyy`）；本操作不计算 `avatarUrl`，因此 `avatarUrl` 通常为 `undefined` 而不出现在 JSON 中 |
| `avatarUrl` | string? | 头像 URL；`getFields()` 会列出，但 `getById` 不赋值，故通常缺省 |
| 其他 | any | 所有 `FLAG_PUBLIC` 的偏好设置字段；若调用者拥有 `PERM.PERM_VIEW_USER_PRIVATE_INFO`，额外附加 `FLAG_PRIVATE` 字段（如 `school`、`studentId`、`phone`） |

`serialize()` 源码：

```ts
getFields(type: 'public' | 'private' = 'public') {
    const fields = ['_id', 'uname', 'mail', 'perm', 'role', 'priv', 'regat', 'loginat', 'avatar', 'avatarUrl'].concat(this._publicFields);
    return type === 'public' ? fields : fields.concat(this._privateFields);
}

serialize(h?) {
    if (this._isPrivate) { /* 私有上下文：输出除 _ 前缀外的所有字段 */ }
    return pick(this, this.getFields(h?.user?.hasPerm(PERM.PERM_VIEW_USER_PRIVATE_INFO) ? 'private' : 'public'));
}
```

> `User.getById` / `getByEmail` / `getByUname` 在找不到时返回 `null`，此时响应体为 `null`。

**示例**

```bash
# 按 UID 查询
curl -X POST 'https://oj.example.com/d/system/api/user' \
  -H 'Accept: application/json' -H 'Content-Type: application/json' \
  --data '{"args":{"id":2,"domainId":"system"},"projection":["_id","uname","role"]}'
# => {"_id":2,"uname":"admin","role":"root"}

# 查询当前登录用户
curl -X POST 'https://oj.example.com/d/system/api/user' \
  -H 'Accept: application/json' -H 'Content-Type: application/json' \
  --data '{"args":{"domainId":"system"}}'
```

#### `users`

| 项目 | 值 |
| --- | --- |
| 操作名 | `users` |
| 类型 | `Query` |
| 注册点 | `packages/hydrooj/src/handler/user.ts` → `UserApi`（无命名空间） |
| 处理器 | `ApiHandler.all` |
| 认证 | 匿名可调用 |
| 域权限 | 无额外检查 |
| 响应 | `User[]`（空结果返回 `[]`） |

**参数表**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `ids` | number[] | ❌ | — | `Schema.array(Schema.number().step(1))`，按 UID 批量查询 |
| `auto` | string[] | ❌ | — | 混合标识（UID 字符串 / 用户名 / 邮箱）批量查询 |
| `search` | string | ❌ | — | 模糊搜索关键字 |
| `limit` | number | ❌ | `10` | 前缀搜索返回条数，内部会被裁剪为 `Math.min(limit, 10)` |
| `exact` | boolean | ❌ | `false` | 为 `true` 时只做精确匹配，不做前缀搜索 |

**行为**

1. `const auto = (arg.ids?.length && arg.ids) || arg.auto || [];` —— `ids` 非空时优先。
2. `auto` 非空时：
   - 过滤出可转数字的项，调用 `user.getList(domainId, maybeId)` 批量取回，并为每条结果补 `avatarUrl = avatar(udoc.avatar)`；
   - 剩余未命中的项依次按 `uname`、`mail` 查找；若未命中数量超过 50，直接返回已找到的部分（拒绝过多逐条查询）。
3. `auto` 为空且 `search` 为空 → 返回 `[]`。
4. 有 `search` 时：先按 `id`（数字）/ `uname` / `mail` 精确匹配一个用户；`exact` 为假时再调用 `user.getPrefixList(domainId, search, Math.min(limit || 10, 10))` 做前缀搜索；若精确命中的用户不在前缀结果中，则替换掉最后一项并插入到首位。
5. 所有返回项都会补 `avatarUrl`。

**响应字段表**：与 `user` 相同（`User.serialize()` 输出），额外保证 `avatarUrl` 有值。

**示例**

```bash
# 前缀搜索（下拉框常用）
curl -X POST 'https://oj.example.com/d/system/api/users' \
  -H 'Accept: application/json' -H 'Content-Type: application/json' \
  --data '{"args":{"search":"ad","limit":5},"projection":["_id","uname","displayName","avatarUrl"]}'
# => [{"_id":2,"uname":"admin","avatarUrl":"..."}]

# 按 ID / 用户名 / 邮箱混合批量查询
curl -X POST 'https://oj.example.com/d/system/api/users' \
  -H 'Accept: application/json' -H 'Content-Type: application/json' \
  --data '{"args":{"auto":["2","admin","a@b.c"]},"projection":["_id","uname"]}'
```

#### `domain`

| 项目 | 值 |
| --- | --- |
| 操作名 | `domain` |
| 类型 | `Query` |
| 注册点 | `packages/hydrooj/src/handler/domain.ts`（约 426 行 `DomainApi`；`api.provide(DomainApi)`，**无命名空间**） |
| 处理器 | `ApiHandler.all` |
| 认证 | 匿名可调用 |
| 域权限 | 目标域内 `PERM.PERM_VIEW` 或 `PRIV.PRIV_VIEW_ALL_DOMAIN`（不满足返回 `null`） |
| 响应 | `DomainDoc` 或 `null` |

**参数表**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `id` | string | ❌ | — | 目标域 ID；缺省时使用当前请求所在域（`ctx.domain`） |

**实现**

```ts
domain: Query(
    Schema.object({
        id: Schema.string(),
    }),
    async (ctx, args) => {
        const ddoc = args.id ? await domain.get(args.id) : ctx.domain;
        if (!ddoc) return null;
        const udoc = await user.getById(ddoc._id, ctx.user._id);
        if (!udoc.hasPerm(PERM.PERM_VIEW) && !udoc.hasPriv(PRIV.PRIV_VIEW_ALL_DOMAIN)) return null;
        return ddoc;
    },
),
```

**响应字段表**（`DomainDoc`，见 `packages/hydrooj/src/interface.ts`；`serializer` 会剔除 `_` 前缀键，`_id` 除外）

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `_id` | string | 域 ID |
| `owner` | number | 域所有者 UID |
| `roles` | Record<string, string> | 角色定义表（角色名 → 权限位串） |
| `avatar` | string | 域图标 |
| `bulletin` | string | 公告（markdown） |
| `host` | string[]? | 自定义域名列表 |
| `lower` | string | 小写域 ID（内部字段） |
| `namespaces` | any | 题号命名空间配置 |
| 其他 | any | 所有通过 `DomainSetting` 声明的域设置字段，如 `name`、`langs`、`share`，以及各插件注册的域设置 |

> `DomainDoc` 的声明为 `interface DomainDoc extends Record<string, any>`，因此字段集合由「数据库文档 + `domain/get` 钩子 + 各插件 `DomainSetting`」共同决定。

**示例**

```bash
curl -X POST 'https://oj.example.com/d/system/api/domain' \
  -H 'Accept: application/json' -H 'Content-Type: application/json' \
  --data '{"args":{"id":"system"},"projection":["_id","name","avatar","bulletin"]}'
# => {"_id":"system","name":"Hydro","avatar":"","bulletin":"..."}
```

#### `domain.current`

| 项目 | 值 |
| --- | --- |
| 操作名 | `domain.current` |
| 类型 | `Query` |
| 注册点 | `packages/hydrooj/src/handler/domain.ts` → `DomainApi`（key 本身含点号，无额外命名空间） |
| 处理器 | `ApiHandler.all` |
| 认证 | 匿名可调用 |
| 域权限 | 无额外检查 |
| 响应 | `{ domain: DomainDoc }` |

**参数表**：`Schema.object({})` —— 不声明任何业务参数；受 schemastery 非 strict 模式影响，调用方传入的额外键会被原样保留在参数对象中，但处理器只使用 `handler`（上下文），不使用参数。

**实现**

```ts
'domain.current': Query(
    Schema.object({}),
    async (handler) => ({ domain: handler.domain as DomainDoc }),
),
```

`handler` 即 `ApiHandler` 实例，`handler.domain` 来自 `handler/create` 钩子写入的 `ctx.HydroContext.domain`（由域层解析得到）。

**响应字段表**

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `domain` | DomainDoc | 当前请求所属域的完整文档 |

**示例**

```bash
curl -X POST 'https://oj.example.com/d/system/api/domain.current' \
  -H 'Accept: application/json' -H 'Content-Type: application/json' \
  --data '{"args":{}}'
# => {"domain":{"_id":"system","name":"Hydro", ...}}
```

前端 `packages/ui-default/utils/index.ts` 的 `loadDomainInfo()` 正是用它做域信息缓存（配合 IndexedDB 的 `domain-info` 存储与 `UiContext.domainVersion` 版本比对）。

#### `groups`

| 项目 | 值 |
| --- | --- |
| 操作名 | `groups` |
| 类型 | `Query` |
| 注册点 | `packages/hydrooj/src/handler/domain.ts` → `DomainApi` |
| 处理器 | `ApiHandler.all` |
| 认证 | 匿名可调用（但通常需要域内权限） |
| 域权限 | `PERM.PERM_VIEW` 或 `PRIV.PRIV_VIEW_ALL_DOMAIN`，否则抛 `PermissionError(PERM.PERM_VIEW)` |
| 响应 | `GDoc[]` |

**参数表**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `domainId` | string | ✅ | — | `Schema.string().required()`，查询域 |
| `uid` | number | ❌ | — | 只返回包含该用户的组；同时会追加一个以 UID 为名的虚拟组 |
| `names` | string[] | ❌ | — | 组名白名单（`$in` 过滤） |
| `search` | string | ❌ | — | 组名正则搜索（大小写不敏感） |
| `limit` | number | ❌ | — | `Schema.number().step(1).max(100)`，最大 100 |

**实现**

```ts
groups: Query(
    Schema.object({
        domainId: Schema.string().required(),
        uid: Schema.number().step(1),
        names: Schema.array(Schema.string()),
        search: Schema.string(),
        limit: Schema.number().step(1).max(100),
    }),
    async (ctx, args) => {
        if (!ctx.user.hasPerm(PERM.PERM_VIEW) && !ctx.user.hasPriv(PRIV.PRIV_VIEW_ALL_DOMAIN)) throw new PermissionError(PERM.PERM_VIEW);
        return user.listGroup(args.domainId, args.uid, args.names, args.search, args.limit || (args.search ? 20 : undefined));
    },
),
```

**响应字段表**（`packages/hydrooj/src/model/user.ts` 的 `listGroup()` 返回的 `GDoc[]`）

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `_id` | string | ObjectId 的十六进制字符串 |
| `domainId` | string | 所属域 |
| `uids` | number[] | 组成员 UID 列表 |
| `name` | string | 组名；当传入 `uid` 时，追加的虚拟组 `name` 为该 UID 的字符串形式 |

**示例**

```bash
curl -X POST 'https://oj.example.com/d/system/api/groups' \
  -H 'Accept: application/json' -H 'Content-Type: application/json' \
  --data '{"args":{"domainId":"system","search":"tea","limit":20},"projection":["name","uids"]}'
# => [{"name":"teachers","uids":[2,3]}]
```

#### `domain.group`

| 项目 | 值 |
| --- | --- |
| 操作名 | `domain.group` |
| 类型 | `Mutation` |
| 注册点 | `packages/hydrooj/src/handler/domain.ts` → `DomainApi` |
| 处理器 | `ApiHandler.all`（**仅 POST**） |
| 认证 | 匿名可调用（但需要域内编辑权限） |
| 域权限 | `PERM.PERM_EDIT_DOMAIN`（作用于当前域 `ctx.domain._id`），否则抛 `PermissionError` |
| 响应 | `boolean` |

**参数表**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `name` | string | ✅ | — | `Schema.string().required()`，组名 |
| `uids` | number[] | ❌ | — | 组成员 UID 列表；**传空数组或省略表示删除该组** |

**实现**

```ts
'domain.group': Mutation(
    Schema.object({
        name: Schema.string().required(),
        uids: Schema.array(Schema.number()),
    }),
    async (ctx, args) => {
        if (!ctx.user.hasPerm(PERM.PERM_EDIT_DOMAIN)) throw new PermissionError(PERM.PERM_EDIT_DOMAIN);
        if (args.uids?.length) {
            const res = await user.updateGroup(ctx.domain._id, args.name, args.uids);
            return res.upsertedCount > 0;
        }
        const res = await user.delGroup(ctx.domain._id, args.name);
        return res.deletedCount > 0;
    },
),
```

**响应字段表**

| 值 | 说明 |
| --- | --- |
| `true` | 新建了组（`upsertedCount > 0`）或删除了组（`deletedCount > 0`） |
| `false` | 仅更新了已有组（未 upsert），或删除时未匹配到任何组 |

> 注意：**该操作使用当前域**（`ctx.domain._id`），不读取 `domainId` 参数；域由 URL 前缀 `/d/:domainId/` 决定。

**示例**

```bash
# 创建/更新组
curl -X POST 'https://oj.example.com/d/system/api/domain.group' \
  -H 'Accept: application/json' -H 'Content-Type: application/json' \
  --data '{"args":{"name":"teachers","uids":[2,3,4]}}'
# => true

# 删除组
curl -X POST 'https://oj.example.com/d/system/api/domain.group' \
  -H 'Accept: application/json' -H 'Content-Type: application/json' \
  --data '{"args":{"name":"teachers"}}'
# => true
```

#### `problem`

| 项目 | 值 |
| --- | --- |
| 操作名 | `problem` |
| 类型 | `Query` |
| 注册点 | `packages/hydrooj/src/handler/problem.ts`（约 1037 行 `ProblemApi`；`api.provide(ProblemApi)`，**无命名空间**） |
| 处理器 | `ApiHandler.all` |
| 认证 | 匿名可调用 |
| 域权限 | 仅当题目 `hidden` 时检查 `PERM.PERM_VIEW_PROBLEM_HIDDEN`（`ctx.checkPerm`） |
| 响应 | `ProblemDoc` 或 `null` |

**参数表**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `id` | number \| string | ✅ | — | `Schema.union([Schema.number().step(1), Schema.string()]).required()`，题目 `docId`（数字）或 `pid`（字符串） |
| `domainId` | string | ✅ | — | `Schema.string().required()`，题目所属域 |

**实现**

```ts
problem: Query(
    Schema.object({
        id: Schema.union([Schema.number().step(1), Schema.string()]).required(),
        domainId: Schema.string().required(),
    }),
    async (ctx, args) => {
        const pdoc = await problem.get(args.domainId, args.id);
        if (!pdoc) return null;
        if (pdoc.hidden) ctx.checkPerm(PERM.PERM_VIEW_PROBLEM_HIDDEN);
        return pdoc;
    },
),
```

**响应字段表**（`ProblemModel.PROJECTION_PUBLIC`，见 `packages/hydrooj/src/interface.ts` 的 `ProblemDoc`）

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `_id` | string | ObjectId 十六进制字符串 |
| `domainId` | string | 所属域 |
| `docType` | number | 文档类型（`TYPE_PROBLEM` = 10） |
| `docId` | number | 域内题目数字 ID |
| `pid` | string | 题目显示 ID（如 `P1000`） |
| `owner` | number | 题目所有者 UID |
| `title` | string | 题目标题 |
| `content` | string | 题面（原始 markdown） |
| `html` | boolean? | 题面是否为 HTML |
| `data` | FileInfo[] | 测试数据文件列表 `{ name, size, ... }` |
| `config` | ProblemConfig \| string | 已解析的题目配置（`problem.get` 会调用 `parseConfig`；解析失败时为 `"Cannot parse: ..."` 字符串） |
| `additional_file` | FileInfo[] | 附加文件列表 |
| `reference` | { domainId, pid }? | 引用来源题目 |
| `maintainer` | number[]? | 维护者 UID 列表 |
| `nSubmit` | number | 提交次数 |
| `nAccept` | number | 通过次数 |
| `difficulty` | number? | 难度 |
| `tag` | string[] | 标签 |
| `hidden` | boolean? | 是否隐藏 |
| `stats` | any? | 统计数据 |
| `sort` | string? | 排序字段 |

**示例**

```bash
curl -X POST 'https://oj.example.com/d/system/api/problem' \
  -H 'Accept: application/json' -H 'Content-Type: application/json' \
  --data '{"args":{"id":1000,"domainId":"system"},"projection":["docId","pid","title","tag"]}'
# => {"docId":1000,"pid":"P1000","title":"A + B Problem","tag":["入门"]}

# 用 pid 字符串查询
curl -X POST 'https://oj.example.com/d/system/api/problem' \
  -H 'Accept: application/json' -H 'Content-Type: application/json' \
  --data '{"args":{"id":"P1000","domainId":"system"},"projection":["docId","title"]}'
```

#### `problems`

| 项目 | 值 |
| --- | --- |
| 操作名 | `problems` |
| 类型 | `Query` |
| 注册点 | `packages/hydrooj/src/handler/problem.ts` → `ProblemApi` |
| 处理器 | `ApiHandler.all` |
| 认证 | 匿名可调用 |
| 域权限 | 隐藏题目是否可见取决于 `PERM.PERM_VIEW_PROBLEM_HIDDEN` 或「自己拥有/维护」 |
| 响应 | `ProblemDoc[]`（按请求顺序，未找到的项被跳过） |

**参数表**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| `ids` | number[] | ✅ | — | `Schema.array(Schema.number().step(1)).required()`，题目 `docId` 列表 |
| `domainId` | string | ✅ | — | `Schema.string().required()`，题目所属域 |

**实现**

```ts
problems: Query(
    Schema.object({
        ids: Schema.array(Schema.number().step(1)).required(),
        domainId: Schema.string().required(),
    }),
    async (ctx, args) => {
        const pdocs = await problem.getList(args.domainId, args.ids, ctx.user.hasPerm(PERM.PERM_VIEW_PROBLEM_HIDDEN) || ctx.user._id,
            undefined, undefined, true);
        return args.ids.map((id) => pdocs[+id]).filter((i) => i);
    },
),
```

说明：

- 第三个参数 `canViewHidden` 传入 `hasPerm(PERM.PERM_VIEW_PROBLEM_HIDDEN) || ctx.user._id`，即「有隐藏题权限」或「自己拥有/维护的题目」都可见。
- 第四个参数 `doThrow` 为 `undefined`（falsy），因此找不到的 ID **不会**抛 `ProblemNotFoundError`，而是被 `filter((i) => i)` 过滤掉。
- 第五个参数 `projection` 为 `undefined` → 使用默认的 `PROJECTION_PUBLIC`。
- 第六个参数 `indexByDocIdOnly = true`，返回值以 `docId` 为键，再由 `args.ids.map(...)` 还原成数组，因此**输出顺序与请求的 `ids` 顺序一致**。

**响应字段表**：与 `problem` 相同（`ProblemDoc`，`PROJECTION_PUBLIC` 字段集），类型为数组。

**示例**

```bash
curl -X POST 'https://oj.example.com/d/system/api/problems' \
  -H 'Accept: application/json' -H 'Content-Type: application/json' \
  --data '{"args":{"ids":[1000,1001],"domainId":"system"},"projection":["docId","pid","title"]}'
# => [{"docId":1000,"pid":"P1000","title":"A + B Problem"},{"docId":1001,"pid":"P1001","title":"..."}]
```

### UI 路由（ui-default / ui-next）

除 API 层外，`packages/ui-default` 与 `packages/ui-next` 还注册了一批「前端资源 / 工具」类路由。它们不挂在 `/api/` 下，遵循普通页面路由规则（同样支持 `/d/:domainId/` 前缀）。

#### `config_schema`

| 项目 | 值 |
| --- | --- |
| 路由名 | `config_schema` |
| 注册点 | `packages/ui-default/index.ts`（位于 `ctx.inject(['setting'], ...)` 内） |
| 处理器 | `SystemConfigSchemaHandler`（同文件） |
| 方法 | `GET` |
| 路径 | `/manage/config/schema.json`（含域前缀则为 `/d/:domainId/manage/config/schema.json`） |
| 认证 | 需登录 |
| 权限 | `PRIV.PRIV_EDIT_SYSTEM`（全局特权，注册时通过 `ctx.Route(..., PRIV.PRIV_EDIT_SYSTEM)` 传入） |
| 响应 | JSON Schema（`schemastery-jsonschema` 转换结果），`application/json` |

**实现**

```ts
class SystemConfigSchemaHandler extends Handler {
  async get() {
    const schema = convert(Schema.intersect(this.ctx.setting.settings) as any, true);
    this.response.body = schema;
  }
}
```

`this.ctx.setting.settings` 是所有已注册系统设置的 schema 集合（`Schema.intersect` 合并），`convert(..., true)` 来自 `schemastery-jsonschema`。响应 `response.template` 为空，因此始终以原始 JSON 返回（无需 `Accept: application/json`）。

**参数表**：无。

**响应字段表**：标准 JSON Schema 文档，结构由 `schemastery-jsonschema` 决定；顶层为 `{ type: 'object', properties: { '<设置键>': {...} }, required: [...] }` 形式的对象。

**示例**

```bash
curl 'https://oj.example.com/manage/config/schema.json' -H 'Accept: application/json'
```

#### `wiki_help`

| 项目 | 值 |
| --- | --- |
| 路由名 | `wiki_help` |
| 注册点 | `packages/ui-default/index.ts` |
| 处理器 | `WikiHelpHandler`（同文件） |
| 方法 | `GET` |
| 路径 | `/wiki/help` |
| 认证 | 无需登录（`noCheckPermView = true`） |
| 权限 | 无 |
| 响应 | HTML（模板 `wiki_help.html`） |

**实现**

```ts
class WikiHelpHandler extends Handler {
  noCheckPermView = true;

  async get() {
    this.response.template = 'wiki_help.html';
  }
}
```

**参数表**：无。

**响应字段表**：无业务字段；`Accept: application/json` 时返回渲染前传入模板的 `args` 对象（空对象 `{}`，因为未设置 `response.body`）。

#### `wiki_about`

| 项目 | 值 |
| --- | --- |
| 路由名 | `wiki_about` |
| 注册点 | `packages/ui-default/index.ts` |
| 处理器 | `WikiAboutHandler`（同文件） |
| 方法 | `GET` |
| 路径 | `/wiki/about` |
| 认证 | 无需登录（`noCheckPermView = true`） |
| 权限 | 无 |
| 响应 | HTML（模板 `about.html`）；JSON 时为 `{ sections }` |

**实现**

```ts
class WikiAboutHandler extends Handler {
  noCheckPermView = true;

  async get() {
    let raw = SystemModel.get('ui-default.about') || '';
    raw = raw.replace(/\{\{ name \}\}/g, this.domain.ui?.name || SystemModel.get('server.name')).trim();
    const lines = raw.split('\n');
    const sections: { id: string, title: string, content: string }[] = [];
    for (const line of lines) {
      if (line.startsWith('# ')) {
        const id = line.split(' ')[1];
        sections.push({ id, title: line.split(id)[1].trim(), content: '' });
      } else sections[sections.length - 1].content += `${line}\n`;
    }
    this.response.template = 'about.html';
    this.response.body = { sections };
  }
}
```

内容来源为系统设置 `ui-default.about`（默认值取自 `packages/ui-default/setting.yaml` 的 `about.value`），其中的 `{{ name }}` 会被替换为域显示名或站点名；以 `# ` 开头的行被视为小节标题。

**参数表**：无。

**响应字段表**

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `sections` | Array<{ id, title, content }> | 按 `# ` 标题切分出的章节 |
| `sections[].id` | string | 标题行第一个空格后的词 |
| `sections[].title` | string | 去掉 `id` 后的剩余标题文本 |
| `sections[].content` | string | 该章节正文（保留换行） |

**示例**

```bash
curl 'https://oj.example.com/wiki/about' -H 'Accept: application/json'
# => {"sections":[{"id":"Hydro","title":"Hydro","content":"..."}]}
```

#### `set_theme`

| 项目 | 值 |
| --- | --- |
| 路由名 | `set_theme` |
| 注册点 | `packages/ui-default/index.ts` |
| 处理器 | `SetThemeHandler`（同文件） |
| 方法 | `GET` |
| 路径 | `/set_theme/:theme` |
| 认证 | 需登录（处理器内 `this.checkPriv(PRIV.PRIV_USER_PROFILE)`；`noCheckPermView = true` 只跳过域权限检查） |
| 权限 | `PRIV.PRIV_USER_PROFILE` |
| 响应 | 302 重定向回来源页（`this.back()`） |

**实现**

```ts
class SetThemeHandler extends Handler {
  noCheckPermView = true;

  async get({ theme }) {
    this.checkPriv(PRIV.PRIV_USER_PROFILE);
    await UserModel.setById(this.user._id, { theme });
    this.back();
  }
}
```

**参数表**

| 参数 | 位置 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- | --- |
| `theme` | 路径参数 | string | ✅ | 目标主题名（`light` / `dark` 等），直接写入用户文档的 `theme` 字段 |

**响应字段表**

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `url` | string | `this.back()` 设置的跳转地址（`request.headers.referer` 或 `/`）；JSON 请求时以 `{ "url": "..." }` 返回，否则 302 |

**示例**

```bash
curl -L 'https://oj.example.com/set_theme/dark' -H 'Accept: application/json'
# => {"url":"https://oj.example.com/"}
```

#### `set_legacy`

| 项目 | 值 |
| --- | --- |
| 路由名 | `set_legacy` |
| 注册点 | `packages/ui-default/index.ts` |
| 处理器 | `LegacyModeHandler`（同文件） |
| 方法 | `GET` |
| 路径 | `/legacy` |
| 认证 | 无需登录（写入 session，不写数据库） |
| 权限 | 无（`noCheckPermView = true`） |
| 响应 | 302 重定向回来源页（`this.back()`） |

**实现**

```ts
class LegacyModeHandler extends Handler {
  noCheckPermView = true;

  @param('legacy', Types.Boolean)
  @param('nohint', Types.Boolean)
  async get({ }, legacy = false, nohint = false) {
    this.session.legacy = legacy;
    this.session.nohint = nohint;
    this.back();
  }
}
```

**参数表**

| 参数 | 位置 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `legacy` | query / body | boolean | ❌ | `false` | `Types.Boolean`（该类型自带 `isOptional = true`）；真值判定为「非 `false`/`off`/`no`/`0` 且有值」 |
| `nohint` | query / body | boolean | ❌ | `false` | 同上 |

**响应字段表**

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `url` | string | 跳转地址（referer 或 `/`） |

**示例**

```bash
curl -L 'https://oj.example.com/legacy?legacy=on&nohint=1' -H 'Accept: application/json'
# => {"url":"https://oj.example.com/"}
```

#### `markdown`

| 项目 | 值 |
| --- | --- |
| 路由名 | `markdown` |
| 注册点 | `packages/ui-default/index.ts` |
| 处理器 | `MarkdownHandler`（同文件） |
| 方法 | `POST`（**仅 POST**，GET 会得到 405 `MethodNotAllowedError`） |
| 路径 | `/markdown` |
| 认证 | 无需登录（`noCheckPermView = true`） |
| 权限 | 无 |
| 响应 | HTML 片段，`Content-Type: text/html`，HTTP 200 |

**实现**

```ts
class MarkdownHandler extends Handler {
  noCheckPermView = true;

  async post({ text, inline = false }) {
    this.response.body = inline
      ? markdown.renderInline(text)
      : markdown.render(text);
    this.response.type = 'text/html';
    this.response.status = 200;
  }
}
```

`markdown` 为 `./backendlib/markdown`（`markdown.js`），内部使用 markdown-it 并注册了 `markdown-it-katex`、`markdown-it-media`、`markdown-it-imsize`、`markdown-it-xss` 等插件；`render(text)` → `md.render(text)`，`renderInline(text)` → `md.renderInline(text)`。

**参数表**（无装饰器，直接从 `this.args` 解构，不做类型校验）

| 参数 | 位置 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `text` | body（或 query） | string | ✅ | — | 待渲染的 markdown 源码 |
| `inline` | body（或 query） | boolean | ❌ | `false` | 为真时使用 `renderInline`，不包裹 `<p>` |

**响应字段表**：直接返回 HTML 字符串（非 JSON 对象）。

**示例**

```bash
curl -X POST 'https://oj.example.com/markdown' \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data-urlencode 'text=**bold** and $x^2$'
# => <p><strong>bold</strong> and <span class="katex">...</span></p>
```

#### `media`

| 项目 | 值 |
| --- | --- |
| 路由名 | `media` |
| 注册点 | `packages/ui-default/index.ts` |
| 处理器 | `RichMediaHandler`（同文件） |
| 方法 | `POST`（仅 POST） |
| 路径 | `/media` |
| 认证 | 无需登录（未设置 `noCheckPermView`，但受域 `PERM.PERM_VIEW` 约束） |
| 权限 | 域 `PERM.PERM_VIEW`；单个条目内部还会按条目类型做权限判断 |
| 响应 | JSON 数组（每项为渲染好的 HTML 字符串），`application/json` |

**实现**

```ts
class RichMediaHandler extends Handler {
  async renderUser(domainId, payload) { /* -> renderHTML('partials/user.html', { udoc }) */ }
  async renderProblem(domainId, payload) { /* -> renderHTML('partials/problem.html', { pdoc }) */ }
  async renderContest(domainId, payload) { /* -> renderHTML('partials/contest.html', { tdoc }) 或 '' */ }
  async renderHomework(domainId, payload) { /* -> renderHTML('partials/homework.html', { tdoc }) 或 '' */ }

  async post({ domainId, items }) {
    const res: any[] = [];
    for (const item of items || []) {
      if (item.domainId && item.domainId === domainId) delete item.domainId;
      if (item.type === 'user') res.push(this.renderUser(domainId, item).catch(() => ''));
      else if (item.type === 'problem') res.push(this.renderProblem(domainId, item).catch(() => ''));
      else if (item.type === 'contest') res.push(this.renderContest(domainId, item).catch(() => ''));
      else if (item.type === 'homework') res.push(this.renderHomework(domainId, item).catch(() => ''));
      else res.push('');
    }
    this.response.body = await Promise.all(res);
  }
}
```

**参数表**

| 参数 | 位置 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `domainId` | body | string | ❌ | 当前域 | 默认域；`this.args.domainId`（域层写入） |
| `items` | body | Array<{ type, id, domainId? }> | ❌ | `[]` | 待渲染条目列表 |
| `items[].type` | — | `'user' \| 'problem' \| 'contest' \| 'homework'` | ✅ | — | 条目类型；其它值返回空字符串 |
| `items[].id` | — | string \| number | ✅ | — | 目标对象标识：`user` 支持 UID（数字）或用户名；`problem` 支持 `docId` / `pid`；`contest` / `homework` 为 ObjectId 字符串 |
| `items[].domainId` | — | string | ❌ | — | 条目所属域；与顶层 `domainId` 相同则被删除，否则按该域渲染 |

**权限细节**

- `renderUser`：若 `payload.domainId` 存在且当前用户在**该域**拥有 `PERM.PERM_VIEW`，则按该域查询，否则回退到当前域。
- `renderProblem`：当前用户需拥有 `PERM.PERM_VIEW | PERM.PERM_VIEW_PROBLEM`，否则返回 `ProblemModel.default`；隐藏题目在「非本人所有且无 `PERM.PERM_VIEW_PROBLEM_HIDDEN`」时也回退为默认题。
- `renderContest` / `renderHomework`：需要 `PERM.PERM_VIEW | PERM.PERM_VIEW_CONTEST` / `PERM.PERM_VIEW | PERM.PERM_VIEW_HOMEWORK`，否则返回空字符串。
- 任一条目渲染抛错时，该条目被替换为 `''`（`.catch(() => '')`），不会导致整体失败。

**响应字段表**：`string[]`，长度与 `items` 一致，顺序一一对应；每项是渲染后的 HTML 片段（失败为空字符串）。

**示例**

```bash
curl -X POST 'https://oj.example.com/media' \
  -H 'Content-Type: application/json' \
  --data '{"domainId":"system","items":[{"type":"user","id":2},{"type":"problem","id":1000}]}'
# => ["<div class=\"media\">...","<div class=\"media\">..."]
```

#### `constant`（`UiConstantsHandler`）

| 项目 | 值 |
| --- | --- |
| 路由名 | `constant` |
| 注册点 | `packages/ui-default/backendlib/builder.ts` 的 `apply(ctx)` |
| 处理器 | `UiConstantsHandler`（同文件） |
| 方法 | `all`（**任意方法**） |
| 路径 | `GET /lazy/:version/:name` 与 `GET /resource/:version/:name`（两条路由共用同一处理器） |
| 认证 | 无需登录（`noCheckPermView = true`） |
| 权限 | 无 |
| 响应 | JavaScript 源码，`Content-Type: application/javascript`，带 `ETag` 与 `Cache-Control: public, max-age=86400` |

**实现**

```ts
class UiConstantsHandler extends Handler {
  noCheckPermView = true;

  @param('name', Types.Filename)
  async all(domainId: string, name: string) {
    this.response.type = 'application/javascript';
    if (!vfs[name]) throw new NotFoundError(name);
    this.response.addHeader('ETag', hashes[name]);
    this.response.body = vfs[name];
    this.response.addHeader('Cache-Control', 'public, max-age=86400');
  }
}
```

**参数表**

| 参数 | 位置 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- | --- |
| `version` | 路径参数 | string | ✅ | 资源版本号；处理器**不使用**该参数，仅用于缓存失效（客户端换 URL 即绕过缓存） |
| `name` | 路径参数 | string | ✅ | `Types.Filename`，虚拟文件名；不存在时抛 `NotFoundError(name)`（404） |
| `domainId` | 隐式 | string | ✅ | 由装饰器约定隐含注入（`async all(domainId, name)` 的第一个形参被识别为域 ID） |

**可用 `name`**（`vfs` 中的键，由 `buildUI()` 生成）

| 名称 | 内容 |
| --- | --- |
| `entry.js` | 页面入口包（`window._hydroLoad`），`UiContextBase.constantVersion` 即其哈希 |
| `<name>.js` | 每个 `*.lazy.ts(x)` 模块的懒加载包 |
| `lang-<lang>.js` | 各语言包，内容为 `window.LOCALES=...` |

`vfs` / `hashes` 由 `buildUI()` 填充：扫描所有 addon 的 `frontend`（或 `public`）目录，用 esbuild 打包 `*.page.*` 与 `*.lazy.*`，哈希为 `sha1(content).substring(0, 8)`。

**响应字段表**：非 JSON，直接返回 JavaScript 源码字符串。

**示例**

```bash
curl 'https://oj.example.com/resource/8f3a1c22/entry.js'
curl 'https://oj.example.com/lazy/8f3a1c22/lang-zh_CN.js'
```

#### `ui_next_constants`（`UiNextConstantHandler`）

| 项目 | 值 |
| --- | --- |
| 路由名 | `ui_next_constants` |
| 注册点 | `packages/ui-next/index.ts` 的 `apply(ctx)` |
| 处理器 | `UiNextConstantHandler`（同文件） |
| 方法 | `all`（任意方法） |
| 路径 | `GET /plugins/:version/:name` |
| 认证 | 无需登录（`noCheckPermView = true`） |
| 权限 | 无 |
| 响应 | JavaScript 源码，`Content-Type: application/javascript`，带 `ETag` 与 `Cache-Control: public, max-age=86400` |

**实现**

```ts
class UiNextConstantHandler extends Handler {
    noCheckPermView = true;

    @param('name', Types.Filename)
    async all(domainId: string, name: string) {
        if (!(name in vfs)) throw new NotFoundError(name);
        this.response.type = 'application/javascript';
        this.response.body = vfs[name];
        this.response.addHeader('ETag', hashes[name]);
        this.response.addHeader('Cache-Control', 'public, max-age=86400');
    }
}
```

**参数表**

| 参数 | 位置 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- | --- |
| `version` | 路径参数 | string | ✅ | 资源版本号，处理器不使用；开发模式下固定为 `0`，生产模式下为文件哈希 |
| `name` | 路径参数 | string | ✅ | `Types.Filename`，虚拟文件名；不存在时抛 `NotFoundError(name)` |
| `domainId` | 隐式 | string | ✅ | 由装饰器约定隐含注入 |

**可用 `name`**（`vfs` 中的键，由 `buildPlugins()` / `buildI18n()` / `buildCodeLangs()` / `buildVersions()` 生成）

| 名称 | 内容 |
| --- | --- |
| `plugins.js` | 所有 addon 的 `ui/index.ts(x)` 聚合包，导出 `window.__hydroPlugins` |
| `chunk-<hash>.js` | esbuild code-splitting 产物 |
| `lang-<lang>.js` | 语言包，内容为 `window.HydroLocale=...` |
| `locale-list.js` | 语言列表，内容为 `window.HydroLocaleList=...` |
| `code-langs.js` | 代码语言配置，内容为 `window.HydroCodeLangs=...` |
| `versions.js` | 各 addon 的 git 版本号，内容为 `window.HydroVersions=...` |

> 生产模式下 `hashes['plugins.js']` 会作为 `plugins_url` 注入到 HTML；未构建完成时回退为 `00000000`，并返回一段提示页面（`PENDING_HTML`）。

**响应字段表**：非 JSON，直接返回 JavaScript 源码字符串。

**示例**

```bash
curl 'https://oj.example.com/plugins/1a2b3c4d/plugins.js'
curl 'https://oj.example.com/plugins/0/lang-zh.js'
```

**ui-next 是否提供额外的 JSON API？**

不提供。`packages/ui-next/` 的目录结构为：

```
packages/ui-next/
├── index.ts          # 后端插件：仅注册 ui_next_constants 路由 + 注册 'next' 渲染器
├── api.ts            # 前端插件 API 的入口（export * from './src/api'）
├── src/
│   ├── api.ts        # 前端对外导出的组件/hooks/类型（Link、usePageData、useNavigate、defineSlot ...）
│   ├── app.tsx, main.tsx, globals.ts
│   ├── components/   # layout.tsx, link.tsx
│   ├── context/      # page-data.tsx, router.tsx
│   ├── hooks/        # use-build-url.ts, use-route-map.ts
│   ├── pages/        # homepage.tsx, problem_main.tsx ...
│   └── registry/     # plugin.ts, slot.tsx, page.tsx, store.ts ...
```

- **没有** `api/` 或 `handler/` 目录，也没有任何 `ctx.api.provide(...)` 调用。
- `index.ts` 除 `ui_next_constants` 外只做两件事：注册名为 `next` 的渲染器（`ctx.server.registerRenderer('next', { accept: [], output: 'html', asFallback: true, priority: 100, ... })`），以及在开发模式挂载 Vite 中间件（`/@vite/`、`/src/`、`/node_modules/`、`/@react-refresh`、`/@fs`、`/@id/` 前缀走 `ctx.server.addCaptureRoute`）。
- 这里出现的 `api.ts` 是**前端**模块（构建产物通过 `window.__hydroExports` 暴露给 addon 的 `ui/index.ts`，见 `src/main.tsx` 与 `index.ts` 中的 `federationPlugin`），与后端的 JSON-RPC `ApiService` 无关。
- ui-next 页面数据加载走的是普通页面路由 + `x-hydro-inject` 头（见下节），不使用 `/api/*`。

### 前端如何调用后端接口

#### `request` 封装（`packages/ui-default/utils/base.ts`）

前端统一通过 `request` 对象发起 HTTP 请求，底层是 jQuery `$.ajax`：

```ts
export const request = {
  ajax(options: Record<string, any>) {
    const stack = new Error().stack;
    return new Promise<any>((resolve, reject) => {
      $
        .ajax({
          dataType: 'json',
          headers: {
            Accept: 'application/json',
          },
          ...options,
        })
        .fail((jqXHR, textStatus, errorThrown: any) => {
          if (textStatus === 'abort') { /* err.aborted = true; err.isUserFacingError = true */ }
          else if (jqXHR.readyState === 0) { /* new Error(i18n('Network error')) */ }
          else if (typeof jqXHR.responseJSON === 'object' && jqXHR.responseJSON.error) {
            const { error } = jqXHR.responseJSON;
            let err: any;
            if (error.params) {
              const message = i18n(error.message, ...error.params);
              err = new Error(message === error.message && error.params.length
                ? `${error.message}: ${error.params.join(' ')}`
                : message);
              err.rawMessage = error.message;
              err.params = error.params;
            } else err = new Error(error.message);
            if (jqXHR.status >= 400 && jqXHR.status < 500) err.isUserFacingError = true;
            reject(err);
          } else if (errorThrown instanceof Error) reject(errorThrown);
          else reject(new Error(textStatus));
        })
        .done(resolve);
    }).catch((e: Error) => { e.stack = stack; throw e; });
  },

  postFile(url: string, form: FormData, options: any = {}) { /* ... */ },
  post(url: string, dataOrForm: JQueryStatic | Node | string | Record<string, any> = {}, options: any = {}) { /* ... */ },
  get(url: string, qs: Record<string, any> = {}, options: Record<string, any> = {}) { /* ... */ },
};
```

**默认请求头**（`request.ajax` 的默认值，可被 `options` 覆盖）

| 头 | 值 | 说明 |
| --- | --- | --- |
| `Accept` | `application/json` | **关键**。服务端 `base.ts` 通过 `(ctx.request.headers.accept \|\| '').includes('application/json')` 判定 `request.json`，从而决定返回原始 JSON 而不是渲染 HTML 模板 |
| `Content-Type` | `application/json` | 仅当 `request.post` 的 `dataOrForm` 是普通对象时由 `post` 自动设置 |
| `X-Requested-With` | — | `request` **不会**主动发送该头；它只出现在服务端 CORS 允许头列表 `x-requested-with, accept, origin, content-type, upgrade-insecure-requests` 中（`framework/framework/server.ts`） |
| `x-hydro-inject` | — | 由 ui-next 的 router 手动设置（见下） |

**方法签名**

| 方法 | 签名 | 默认行为 |
| --- | --- | --- |
| `request.ajax` | `(options: Record<string, any>) => Promise<any>` | `dataType: 'json'`、`headers.Accept = 'application/json'`，其余透传 |
| `request.get` | `(url: string, qs: Record<string, any> = {}, options = {}) => Promise<any>` | `{ url, data: qs, method: 'get', ...options }` |
| `request.post` | `(url: string, dataOrForm = {}, options = {}) => Promise<any>` | 见下方分支 |
| `request.postFile` | `(url: string, form: FormData, options = {}) => Promise<any>` | `{ url, data: form, processData: false, contentType: false, type: 'POST', dataType: undefined, ...options }` |

`request.post` 的 `dataOrForm` 分支处理：

```ts
let postData;
if (dataOrForm instanceof $ && dataOrForm.is('form')) postData = (dataOrForm as any).serialize();          // jQuery 表单对象
else if (dataOrForm instanceof Node && $(dataOrForm).is('form')) postData = $(dataOrForm).serialize();      // DOM 表单
else if (typeof dataOrForm === 'string') postData = dataOrForm;                                             // 'foo=bar&box=boz'
else { postData = JSON.stringify(dataOrForm); options.contentType = 'application/json'; }                  // 普通对象 -> JSON
return request.ajax({ url, method: 'post', data: postData, ...options });
```

**错误处理**

| 场景 | 行为 |
| --- | --- |
| 请求被 abort | reject `Error(i18n('Aborted'))`，并设置 `err.aborted = true`、`err.isUserFacingError = true` |
| 网络失败（`readyState === 0`） | reject `Error(i18n('Network error'))`，`err.isUserFacingError = true` |
| 响应体含 `error` 字段 | 用 `i18n(error.message, ...error.params)` 渲染消息；i18n 未命中且有 `params` 时拼成 `"<message>: <params.join(' ')>"`；附加 `err.rawMessage`、`err.params`；HTTP 4xx 时 `err.isUserFacingError = true` |
| 其它 | 透传 `errorThrown` 或 `Error(textStatus)` |

**CSRF**

服务端 `framework/framework/server.ts` 的 `Handler.init()` 对 POST 做同源校验：

```ts
async init() {
    if (this.request.method === 'post' && this.request.headers.referer && !this.context.cors && !this.allowCors) {
        try {
            const host = new URL(this.request.headers.referer).host;
            if (host !== this.request.host) throw new CsrfTokenError(host);
        } catch (e) {
            throw e instanceof CsrfTokenError ? e : new CsrfTokenError();
        }
    }
}
```

因此：

- 同源页面发起的 POST 会携带浏览器自动附加的 `Referer`，host 匹配即通过。
- 跨站伪造请求会抛 `CsrfTokenError`（403，`ForbiddenError` 子类）。
- 无 `Referer` 的请求（如某些原生客户端）不会被拦截。
- 需要在 CORS 场景下跳过校验时，服务端需设置 `WebService.Config.cors` 并在中间件中把 `ctx.cors` 置为 `true`，或由 handler 声明 `allowCors = true`。

**`x-hydro-inject`**

该头用于让服务端把上下文信息「注入」到 JSON 响应体中（`framework/framework/base.ts`）：

```ts
if (request.headers['x-hydro-inject']) {
    const inject = request.headers['x-hydro-inject'].toString().toLowerCase().split(',').map((i) => i.trim());
    if (inject.includes('pagename')) {
        ctx.set('x-hydro-page', ctx._matchedRouteName || '');
        ctx.set('x-hydro-template', response.template || '');
    }
    if (response.body !== null && typeof response.body === 'object') {
        if (inject.includes('uicontext')) response.body.UiContext = UiContext;
        if (inject.includes('usercontext')) response.body.UserContext = user;
        if (inject.includes('routemap')) response.body.routeMap = handler.ctx.server.routeMap;
    }
}
```

| 取值 | 效果 |
| --- | --- |
| `uicontext` | 响应体附加 `UiContext` |
| `usercontext` | 响应体附加 `UserContext` |
| `routemap` | 响应体附加 `routeMap`（全部路由名 → 路径的映射） |
| `pagename` | 不修改响应体，改为设置响应头 `x-hydro-page`（匹配到的路由名）与 `x-hydro-template`（模板名） |

ui-next 的 router（`packages/ui-next/src/context/router.tsx`）就是用它做页面数据预取的：

```ts
const res = await fetch(reqUrl, {
  signal,
  headers: {
    Accept: 'application/json',
    'x-hydro-inject': [
      'uicontext', 'usercontext', 'pagename',
      ...(init ? ['routemap'] : []),
    ].join(','),
  },
});
const body = await res.json();
const pageName = res.headers.get('x-hydro-page') || '';
const template = res.headers.get('x-hydro-template') || '';
```

> 注意：`x-hydro-inject` 只对**页面路由**生效（服务端在渲染 JSON 时注入）；`/api/:op` 的响应体是 `ApiHandler.all` 直接赋值的业务结果，不会被注入。

#### `api()` 客户端（`packages/ui-default/utils/index.ts`）

对 JSON-RPC 层的轻量封装：

```ts
export async function api(method: string, args: Record<string, any>, projection?: any) {
  const res = await request.post(`/d/${UiContext.domainId}/api/${encodeURIComponent(method)}`, { args, projection });
  if (res.error) throw new Error(res.error);
  return res;
}
```

调用链：

1. `request.post(url, { args, projection })` —— 因第二个参数是普通对象，jQuery 会以 `Content-Type: application/json` 发送 `{"args":{...},"projection":...}`。
2. URL 形如 `/d/{UiContext.domainId}/api/{encodeURIComponent(op)}` —— **固定带域前缀**，域取自页面注入的 `UiContext.domainId`。
3. 服务端 `ApiHandler.all` 解析 `this.args.args` / `this.args.projection`，合并 `{ domainId, ...this.args, ...(this.args.args || {}) }`，交给操作自身的 schema 校验后执行。
4. 响应体就是操作的返回值（对象/数组/标量）；`api()` 原样返回，仅在响应体含 `error` 字段时抛错（正常情况下 4xx 会走 `request` 的错误分支，此判断是双保险）。

该函数从 `packages/ui-default/utils/index.ts` 导出，经 `packages/ui-default/api.ts` 的 `export * from './utils'` 聚合到 `window.HydroExports`，页面/组件通过别名 `vj/utils` 引用：

```ts
import { api, request } from 'vj/utils';
```

**可复用示例**

```ts
import { api, request } from 'vj/utils';

// 1) 查询当前域信息（无 projection，返回完整对象）
const { domain } = await api('domain.current', {});

// 2) 前缀搜索用户（projection 用字符串数组，只取需要的字段）
const users = await api('users', { search: 'ad', limit: 10 }, ['_id', 'uname', 'displayName', 'avatarUrl']);

// 3) 按 ID / 用户名 / 邮箱混合批量取用户
const list = await api('users', { auto: ['2', 'admin'] }, ['_id', 'uname']);

// 4) 批量取题目（嵌套 projection：只取 data 的 name 字段）
const pdocs = await api('problems', { ids: [1000, 1001], domainId: UiContext.domainId }, {
  docId: 1,
  pid: 1,
  title: 1,
  data: { name: 1 },
});

// 5) Mutation：修改域内用户组（自动 POST）
await api('domain.group', { name: 'teachers', uids: [2, 3] });

// 6) 非 API 的页面级 JSON 请求（保留渲染器的数据接口）
const { pdocs: searchResult } = await request.get(`/d/${UiContext.domainId}/p`, {
  q: 'a+b', limit: 10, quick: true,
});

// 7) 带文件上传的 POST
const form = new FormData();
form.append('file', fileInput.files[0]);
const uploaded = await request.postFile(`/d/${UiContext.domainId}/file`, form);

// 8) 错误处理：4xx 时 err.isUserFacingError 为 true，可直接展示 err.message
try {
  await api('domain.group', { name: 'teachers', uids: [2, 3] });
} catch (e: any) {
  if (e.isUserFacingError) console.warn(e.message, e.params);
  else throw e;
}
```

仓库中的真实用法（可作为最佳实践参考）：

| 位置 | 用法 |
| --- | --- |
| `utils/index.ts` | `api('domain.current', {})` —— 加载并缓存域信息 |
| `components/autocomplete/components/UserSelectAutoComplete.tsx` | `api('users', { search: query }, ['_id', 'uname', 'displayName', 'avatarUrl'])`、`api('users', { auto: ids }, ['_id', 'uname', 'displayName'])` |
| `components/autocomplete/components/AssignSelectAutoComplete.tsx` | 并行调用 `api('users', ...)` 与 `api('groups', { search }, ['name', 'uids'])` |
| `components/autocomplete/components/ProblemSelectAutoComplete.tsx` | `api('problems', { ids: ids.map((i) => +i) }, ['docId', 'pid', 'title'])` |
| `components/zipDownloader/index.ts` | `api('problem', { id: +pid }, { pid: 1, owner: 1, title: 1, content: 1, tag: 1, nSubmit: 1, nAccept: 1, data: { name: 1 }, additional_file: { name: 1 } })` |
| `components/monaco/languages/markdown.ts` | `api('users', { ids: [...] }, ['_id', 'uname'])` |
| `components/omnisearch/index.page.tsx` | `api('users', { search: query }, ['_id', 'uname', 'displayName', 'avatarUrl'])` |

**WebSocket 客户端**

`packages/ui-default/components/socket/index.ts` 导出的 `Sock` 类封装了重连、心跳与 `shorty` 压缩，可配合 API 订阅使用：

```ts
import Socket from 'vj/components/socket';

const sock = new Socket(`${UiContext.ws_prefix}d/${UiContext.domainId}/api/test.subscription/conn`, false, true);
sock.onopen = () => console.log('connected');
sock.onmessage = (message, data) => {
  const payload = JSON.parse(data);          // 已自动 shorty 解压
  if (payload.error) return console.error(payload.error);
  console.log(payload);                      // { count: 1 } ...
};
```

| 构造参数 | 类型 | 说明 |
| --- | --- | --- |
| `url` | string | WebSocket 路径，可用 `UiContext.ws_prefix` 拼接 |
| `nocookie` | boolean | 为 `false` 且目标 host 与当前 host 不同时，自动附加 `?sid=<session id>` |
| `shorty` | boolean | 为 `true` 时附加 `?shorty=on`，启用 payload 压缩（客户端自动 `inflate`） |

`Sock` 内部行为：`onopen` 后每 30s 发送字符串 `ping`；收到 `ping` 回 `pong`；收到 `shorty` 后启用解压；收到 `error.name` 为 `PermissionError` / `PrivilegeError` 的消息时自动关闭连接（对应服务端 `close(4000, ...)`）。
