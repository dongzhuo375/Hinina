# E2E 层（Playwright）

> 对应源码：仓库根 `e2e/` 目录与 `playwright.config.ts`（M3 落地，见 `doc/cicd/m3-integration.md`）。

## 职责

用真实浏览器（Chromium）跑真实前端，验证单元测试覆盖不到的跨层组合（路由守卫 × store × 视图 × Monaco × IPC 全链路）。IPC 由 `fake-tauri.ts` 注入的 fake internals 承接——**零生产代码改动、零构建开关**，fake 只存在于 Playwright 会话。

## 文档索引

| 源文件 | 文档 |
|---|---|
| `playwright.config.ts` | [playwright.config.md](playwright.config.md) |
| `e2e/fake-tauri.ts` | [fake-tauri.md](fake-tauri.md) |
| `e2e/specs/auth.spec.ts` | [specs/auth.md](specs/auth.md) |
| `e2e/specs/workspace.spec.ts` | [specs/workspace.md](specs/workspace.md) |
| `e2e/specs/submission.spec.ts` | [specs/submission.md](specs/submission.md) |
| `e2e/specs/session.spec.ts` | [specs/session.md](specs/session.md) |

## Fake 的保真度规则（修改 fake 前必读）

1. **未实现命令 fail-loud**：console.error + `{Unknown}` 拒绝，禁止静默兜底。
2. **错误形态对齐真实 Tauri**：业务错误以 `{ Variant: message }` 对象拒绝（bridge `parseAppError` 的单键对象路径，sessionGuard 依赖 `variant === 'Auth'`）；参数错误以裸字符串拒绝（Tauri 形态 `invalid args \`key\` for command \`cmd\``）。
3. **必填参数校验先于命令体**（对齐 Tauri 反序列化时机）：按 Rust 命令签名的非 Option 参数建表，缺参/类型错直接拒绝，防止前端漏传参数的回归假绿。
4. **无工作区时拒绝变更**：`update_workspace_file` / `set_workspace_language` / `delete_workspace_file` 作用于「当前已加载工作区」，未加载即拒绝——不得建幻影工作区（真实后端会拒绝的行为测试不能通过）。
5. **fixtures 动态时间**：比赛 `startTime = now-1h`、`endTime = now+8h`、`status=0`，保证恒处 running 赛段（路由守卫按赛段放行）；`oj.contestRef` 必须等于比赛 id。
6. **数据全部虚构**：无真实凭证；调用记录只留内存，不落 artifact。
