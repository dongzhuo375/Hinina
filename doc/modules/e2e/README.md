# E2E 层（Playwright）

> 对应源码：仓库根 `e2e/` 目录与 `playwright.config.ts`（M3 落地，见 `doc/cicd/m3-integration.md`）。

## 职责

用真实浏览器（Chromium）跑真实前端，验证单元测试覆盖不到的跨层组合（路由守卫 × store × 视图 × Monaco × IPC 全链路）。IPC 由 `fake-tauri.ts` 注入的 fake internals 承接——**零生产代码改动、零构建开关**，fake 只存在于 Playwright 会话。

## 文档索引

| 源文件 | 文档 |
|---|---|
| `playwright.config.ts` | [playwright.config.md](playwright.config.md) |
| `e2e/fake-tauri.ts` | [fake-tauri.md](fake-tauri.md) |
| `e2e/fixtures.ts` | 共享 test 入口：失败时把 `window.__e2e.calls` 写为 `test-results/**/ipc-calls.json` 附件（施工方案 §6.3 要求的 IPC 日志），随 artifact 上传；四个 spec 统一从这里 import `test`/`expect` |
| `e2e/specs/auth.spec.ts` | [specs/auth.md](specs/auth.md) |
| `e2e/specs/workspace.spec.ts` | [specs/workspace.md](specs/workspace.md) |
| `e2e/specs/submission.spec.ts` | [specs/submission.md](specs/submission.md) |
| `e2e/specs/session.spec.ts` | [specs/session.md](specs/session.md) |

## Fake 的保真度规则（修改 fake 前必读）

1. **未实现命令 fail-loud**：console.error + `{Unknown}` 拒绝，禁止静默兜底。
2. **错误形态对齐真实 Tauri**：业务错误以 `{ Variant: message }` 对象拒绝（bridge `parseAppError` 的单键对象路径，sessionGuard 依赖 `variant === 'Auth'`）；参数错误以裸字符串拒绝（Tauri 形态 `invalid args \`key\` for command \`cmd\``）。
3. **必填参数校验先于命令体**（对齐 Tauri 反序列化时机）：按 Rust 命令签名的非 Option 参数建表，缺参/类型错直接拒绝，防止前端漏传参数的回归假绿。
4. **无工作区时拒绝变更、且与 Rust 语义一致**：`update_workspace_file` / `set_workspace_language` / `delete_workspace_file` 作用于「当前已加载工作区」，未加载即拒绝；`update_workspace_file` 同时把该文件记为 `activeFile` 并置 `isDirty`；`delete_workspace_file` 拒绝删除当前代码文件与 `workspace.json`；新建工作区**无预置文件**（与 Rust `Workspace::new` 一致）。不得建幻影工作区或虚构初始代码（评审六轮）。
5. **命令契约校验**：`submit_code` 校验 `displayId` 为已知展示题号、其真实 pid 与 `problemId` 一致、语言在该题允许列表内（pid 当 displayId 传之类的回归会假绿）；`get_contest_problem_limits` 只返回请求过且已知的 `displayIds`（获取失败的题不出现，不以默认值兜底）。
5. **fixtures 动态时间**：比赛 `startTime = now-1h`、`endTime = now+8h`、`status=0`，保证恒处 running 赛段（路由守卫按赛段放行）；`oj.contestRef` 必须等于比赛 id。
6. **后端状态跨 reload 持久化**：`addInitScript` 每次导航/reload 都重跑——若后端状态只挂文档内存，reload 就重建「后端」，与真实 Tauri（WebView reload 不重建 Rust 进程）相反。工作区/提交/会话/修订计数/配置/**当前工作区 id** 序列化到 **sessionStorage** 并在 bootstrap 恢复；`calls`（IPC 日志）、`failQueue`、事件回调属文档生命周期，不持久化。
7. **契约优先于固定成功**：`switch_oj` 校验实例并切换 `oj.active`；`load_configured_contest` 校验 `oj.contestRef`（空 → Config，不匹配 → Contest）；`submit_code` 要求已登录（HOJ require_token，匿名以 `{Auth}` 拒绝），登录态在命令发起时校验（先于延迟）。未实现的命令不得返回无条件成功。
8. **数据全部虚构**：无真实凭证；`calls` 的 `password` **脱敏为 `***`**；正常情况只留内存，**失败时**经 `fixtures.ts` 附件写入 test-results（内容虚构）。
