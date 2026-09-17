# authStore（认证状态机）

> 源文件：`src/stores/authStore.ts`

## 职责

持有当前用户与会话确认标记，编排登录 / 登出 / 会话恢复 / 三态校验四条链路，是全局会话守卫与路由守卫的唯一认证状态源；登出时联动 `stores/session` 清理全部领域状态。

## 核心类型/函数

常量：`SESSION_INVALID_MESSAGE` — 会话失效统一文案（点明最常见原因「同账号在其他设备登录」并给出下一步「重新登录」，而不是只报"未授权"）。

| 名称 | 签名 | 用途 |
|------|------|------|
| state.`user` / `isLoading` / `error` | — | 认证态与请求状态 |
| state.`sessionResolved` | `boolean` | **是否已与后端确认过会话状态**（登录/登出/会话恢复均视为已确认）；路由守卫据此决定是否发起 `get_session`，避免每次导航重复 IPC |
| `isLoggedIn` / `username` | getters | `user !== null` / 用户名 |
| `login` | `(username, password, ojType?) => Promise<void>` | 成功写入 user 并置 `sessionResolved = true`；失败记录 error 并**抛出**（登录表单需要感知失败） |
| `logout` | `() => Promise<void>` | **永不 reject**：后端登出失败也一律清理本地（user=null + `clearDomainState()`），保证 UI 不停留在需认证页面；失败时 `sessionResolved` 复位 false（下次进受保护路由重新校验） |
| `validateSession` | `() => Promise<SessionValidity>` | 三态校验；`invalid` 且已登录 → 就地 `invalidateSession(SESSION_INVALID_MESSAGE)`；**`unknown` 保持登录态不变**（赛前误踢回登录页的代价远大于多等一轮校验） |
| `invalidateSession` | `(reason: string) => Promise<void>` | 复用 logout 清理链路，随后 `error = reason`（供登录页展示）+ `sessionResolved = false`（强制下次重新校验）；由全局会话守卫调用 |
| `checkSession` | `() => Promise<boolean>` | 启动/首导航时恢复后端会话；异常吞掉返回 false；`finally` 中**无论成败都置 `sessionResolved = true`**（守卫无需重复 IPC） |

## 直接依赖

- `pinia`
- `@/types/user`（仅类型）
- `@/services/auth.service`（`authService`）
- `@/stores/session`（`clearDomainState`）
- `@/utils/error`（`errorMessage` —— 错误文案收敛）
- `@/utils/logger`（`createLogger` —— 作用域日志）

## 被依赖

- `router/index.ts` — `beforeEach` 守卫（`sessionResolved` / `isLoggedIn` / `checkSession`）
- `stores/sessionGuard.ts` — 认证类 IPC 失败 → `invalidateSession`
- `views/LoginView.vue`（登录/切换账号/预检）、`views/RankView.vue` / `ProblemSetView.vue`（uid）、`components/layout/TopBar.vue`（用户名/登出）
- `stores/__tests__/authStore.spec.ts`

## 逻辑流程

```
login → authService.login → user + sessionResolved=true（失败：error + rethrow）

logout（永不 reject）
  → authService.logout()（后端 + localStorage；失败仅记录 backendCleared=false）
  → user = null；sessionResolved = backendCleared
  → clearDomainState()（workspace 防抖取消 → submission/rank 轮询回收 → contest 定向清理 → problem 重置）

validateSession → authService.validateSession()
  ├─ valid   → 原样返回（登录态不动）
  ├─ unknown → 原样返回（登录态不动，调用方决定重试）
  └─ invalid → 已登录时 invalidateSession(统一文案) → 返回

invalidateSession(reason) → logout() → error=reason → sessionResolved=false
```

设计要点：

- **logout 永不 reject**：调用方（TopBar、LoginView、sessionGuard）无需各自捕获，
  `finally` 里的跳转/清理必然执行；后端失败只影响 `sessionResolved`（保守复位，
  下次导航重新校验），本地清理不打折。
- **`sessionResolved` 与 `isLoggedIn` 分离**：「确认过没有会话」也是有效结论——
  checkSession 失败/无会话同样置 true，守卫不会在每次导航都重发 `get_session`。
- 三态语义与 Rust `SessionValidity` 严格对齐（见 `types/user.md`）；invalid 的清理
  与主动登出共用链路，差异只在 error 文案与 sessionResolved 复位。
- 依赖方向单向：`authStore → stores/session → 各领域 store`，领域 store 不反向依赖
  authStore，避免循环（见 `stores/session.md`）。

## 测试

`src/stores/__tests__/authStore.spec.ts` 锁定：login 成功/失败的状态与 sessionResolved 语义、logout「后端失败也不 reject 且本地一律清理」「保留匿名比赛简报」「清理前取消工作区防抖同步」、checkSession 三路径（恢复/无会话仍置已确认/异常不抛出）、validateSession 状态机（valid 不动、**unknown 必须保留登录态**、invalid 清理+复位、未登录时 invalid 不覆盖已有 error）、invalidateSession 以指定原因清理。
