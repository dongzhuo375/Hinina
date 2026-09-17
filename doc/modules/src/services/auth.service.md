# auth.service（前端认证服务）

> 源文件：`src/services/auth.service.ts`

## 职责

认证领域的服务层：登录 / 登出 / 会话检查 / 三态校验的业务编排，附带 `localStorage` 用户缓存（key `hinina_user`）；是 authStore 访问认证后端的唯一入口。

## 核心类型/函数

常量：`STORED_USER_KEY = 'hinina_user'`。

| 名称 | 签名 | 用途 |
|------|------|------|
| `AuthService.login` | `(username, password, ojType?) => Promise<User>` | 调 bridge 登录，成功后把 User 写入 localStorage |
| `AuthService.logout` | `() => Promise<void>` | `try { bridge.logout() } finally { localStorage.removeItem }` —— **后端登出失败也清本地缓存**（本地状态必须与"已登出"的 UI 语义一致），异常仍向上抛（由 authStore 决定吞掉） |
| `AuthService.checkSession` | `() => Promise<User \| null>` | 查后端会话；有用户则刷新缓存；**IPC 异常吞掉返回 null**（启动路径不应因会话查询失败而中断） |
| `AuthService.validateSession` | `() => Promise<SessionValidity>` | 透传后端三态；**IPC 自身异常（序列化/通道故障）归一为 `unknown` 而非 `invalid`** —— 调用方只面对三种业务语义，传输层故障不会把用户误踢回登录页 |
| `AuthService.getStoredUser` | `() => User \| null` | 读 localStorage 缓存；JSON 损坏时清除脏数据返回 null |
| `authService` | 单例 | 全局唯一实例 |

## 直接依赖

- `@/bridge/auth.bridge`（login / logout / getSession / validateSession）
- `@/types/user`（仅类型）
- `@/utils/logger`（`createLogger` —— 作用域日志）

## 被依赖

- `stores/authStore.ts` — 唯一调用方

## 逻辑流程

```
login → bridge.login → 成功写 localStorage → 返回 User（失败不写缓存、上抛）
logout → bridge.logout（成败与否）→ finally 清 localStorage → 后端异常继续上抛
checkSession → bridge.getSession → 有用户刷新缓存并返回 / null / 异常 → null
validateSession → bridge.validateSession → 'valid'|'invalid'|'unknown'；IPC 异常 → 'unknown'
```

设计要点：

- **localStorage 缓存只是 UI 快照**：权威会话在后端（Rust 端 `sessions/{oj_type}.json` +
  Provider token），缓存仅供展示层快速取用户名等；损坏即清除，不做修复。
- 错误策略按调用场景分化：login 上抛（表单要展示原因）、logout 上抛但本地必清、
  checkSession/validateSession 吞传输异常（启动与预检路径不能被 IPC 故障打断）——
  三态语义见 `types/user.md` 与 `service/auth/mod.md`（Rust 侧）。

## 测试

`src/services/__tests__/auth.service.spec.ts`（mock auth.bridge）锁定：login 成功写缓存/失败不写且上抛、logout「后端失败也必须清本地缓存且异常上抛」契约、checkSession 三路径（刷新缓存 / null 不写缓存 / IPC 异常吞掉返回 null）、validateSession 透传三态且「IPC 自身异常归一为 unknown 而非 invalid」契约、getStoredUser 解析/损坏清除/无缓存。
