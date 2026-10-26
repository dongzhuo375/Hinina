# fake-tauri.ts（E2E 的 Tauri IPC fake）

> 源文件：`e2e/fake-tauri.ts`

## 职责

用 `page.addInitScript` 注入**自包含**的 `window.__TAURI_INTERNALS__`（Tauri v2 命令与事件的唯一 IPC 入口）与 `window.__TAURI_EVENT_PLUGIN_INTERNALS__`（`@tauri-apps/api` 2.11 的 `_unlisten` 前置调用），承接前端全部 IPC；并暴露 `window.__e2e` 控制面供用例断言与注入。保真度规则见 [README.md](README.md)。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `FakeOptions` | `{ user?, failLogin?, judgementSequence? }` | 按用例覆盖：预置会话（会话恢复场景）、预置 login 拒绝、评测状态序列（缺省 `Pending→Running→Accepted`，停留在最后一个元素——`['Running']` 即「永不终态」，轮询停止类断言依赖它） |
| `FakeOptions` → 注入脚本 | `fakeTauriBootstrap(options)` | **自包含**引导函数：经 addInitScript 序列化执行，不得闭包模块级运行时值；fixtures 全部在函数体内以动态时间构建 |
| `installFakeTauri(page, options?)` | fn | 在 `page.goto` 前注入（时序：先于应用脚本执行） |
| `loginAndEnterContest(page, options?)` | fn | 四场景共用的进场路径：填登录表单 → 断言跳 `/contest/problems` → 断言题目总览渲染 |
| `callCount(page, cmd)` | fn | 统计某命令的调用次数（去重/轮询次数断言） |
| `fakeControl(page)` | fn | 读取页面内控制面 |
| `window.__e2e` | `{ calls, emitEvent, failNext, backendState }` | `calls` 调用记录（`{cmd, args}`）；`emitEvent(name, payload)` 推事件（如 `workspace-saved`）；`failNext(cmd, variant, message)` 一次性错误注入；`backendState()` 返回后端状态快照（当前 `lastWorkspaceId`，供 reload 后「当前工作区仍在」断言） |

**命令表（26 个，按 Rust 命令签名取证）**：auth 域 4（get_session/login/logout/validate_session，共享 `currentUser`——登录后恢复、登出后 get_session 为空可被真实表达）；config 域 4；contest/announcement 域 5；problem 域 3（get_problem 按**展示题号**查询，返回的 `Problem.id` 是真实 pid）；rank 1；submission 域 3（submit 生成 9001 起的会话内提交，评测序列逐次推进并回填记录状态）；workspace 域 6（按题目真实 pid 内存隔离，`update_workspace_file` 返回递增修订号，`save_workspace` 发 `workspace-saved` 事件）。

**关键内部机制**：
- **后端状态持久化（评审三轮 #4 / 四轮 #5）**：`PersistedBackend`（workspaces/submissions/currentUser/revisionCounter/submitSeq/config/**lastWorkspaceId**）经 `persistBackend()` 在变更命令成功后写入 `sessionStorage`，bootstrap 先 `restoreBackend()` 恢复（恢复成功则忽略 `options.user`/`failLogin` 播种）；`PERSIST_CMDS` 列出需要落库的命令。`lastWorkspaceId` 也持久化——真实后端的 `WorkspaceManager.current` 存活于 Rust 进程，reload 不丢；不持久化会让 reload 后的 `update_workspace_file` 错误地报「无工作区」。文档级 `calls`/`failQueue`/事件回调不持久化。
- `REQUIRED_ARGS`（17 命令，含 `switch_oj.ojId`）+ `ARG_TYPES`（Vec → array，AppConfig → object）+ `OPTIONAL_ARG_TYPES`（currentPage/limit/status → number，removeStar/containsEnd → boolean，keyword/problemDisplayId → string，null 视为未提供）：`assertRequiredArgs` 在命令体**之前**执行（对齐 Tauri 反序列化时机），缺参与类型错都以 Tauri 形态裸字符串拒绝。
- `requireLoadedProblemId()`：工作区变更类命令必须先 `load_workspace`，否则拒绝——不建幻影工作区。
- **契约约束**：`switch_oj` 实例校验（未注册/未启用 → `{ProviderNotFound}`）并更新 `oj.active`；`load_configured_contest` 校验 `oj.contestRef`；`submit_code` 要求 `currentUser`（匿名 → `{Auth}`，对应 HOJ `require_token`）。
- `dispatch`：`plugin:event|listen/unlisten`（回调 id 注册表）与 `plugin:window|*`（no-op）单独分派，其余走命令表；`transformCallback`/`unregisterCallback` 支撑事件订阅。
- 错误双形态：业务错误 `throw { [variant]: message }`；参数错误 `throw \`invalid args ...\``（裸字符串）。

## 直接依赖

- `@playwright/test`（`Page` 类型、`expect`）
- `../src/types/*`（**仅类型**，运行时零依赖——注入脚本必须自包含）

## 被依赖

- `e2e/specs/*.spec.ts`（全部用例的进场与断言入口）

## 逻辑流程

`installFakeTauri` → 页面首次导航前注入 bootstrap → 先从 sessionStorage 恢复后端状态（无则构建 fixtures，动态时间保证 running 赛段）+ 事件注册表 + 控制面 → 应用启动后所有 invoke 进 `dispatch` → 参数校验 → failQueue 命中则注入错误 → 命令表执行 → `PERSIST_CMDS` 成功后写回 sessionStorage（reload 后可恢复）→ 每次调用记入 `calls`（文档级；失败时由 `fixtures.ts` 附件落盘）。
