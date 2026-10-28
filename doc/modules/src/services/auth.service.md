# auth.service（前端认证服务）

> 源文件：`src/services/auth.service.ts`

## 职责

认证领域的服务层：登录 / 登出 / 会话检查 / 三态校验的业务编排；是 authStore 访问认证后端的唯一入口。

**刻意不做任何前端持久化**（P66，2026-09-20）：用户信息（含 token）的唯一存放处是后端会话文件，
前端每次经 `checkSession` / `validateSession` 向后端查询。此前这里把含 token 的完整 `User` JSON
写进 localStorage（key `hinina_user`），但唯一的读取方只被测试引用 —— 「只写不读」的死代码，
且是可被同源任意脚本读取的暴露面。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `AuthService.login` | `(username, password) => Promise<User>` | 调 bridge 登录（OJ 切换走显式 `switchOj`，与登录解耦）；失败上抛，无本地写入 |
| `AuthService.logout` | `() => Promise<void>` | 直接透传 `bridge.logout()`；**后端失败时异常向上抛**，由 `authStore.logout` 决定 UI 语义（服务层不吞错误，也没有需要在此清理的前端副本） |
| `AuthService.checkSession` | `() => Promise<User \| null>` | 查后端会话；正常 `null` 专属「后端确认无会话」。**IPC 异常记录日志并上抛**，不得把传输故障伪装成已登出 |
| `AuthService.validateSession` | `() => Promise<SessionValidity>` | 透传后端三态；**IPC 自身异常（序列化/通道故障）归一为 `unknown` 而非 `invalid`** —— 调用方只面对三种业务语义，传输层故障不会把用户误踢回登录页 |
| `authService` | 单例 | 全局唯一实例 |

## 直接依赖

- `@/bridge/auth.bridge`（login / logout / getSession / validateSession）
- `@/types/user`（仅类型）
- `@/utils/logger`（`createLogger` —— 作用域日志）

## 被依赖

- `stores/authStore.ts` — 唯一调用方

## 逻辑流程

```
login → bridge.login → 返回 User（失败上抛，无本地写入）
logout → bridge.logout → 成败均原样透传（异常上抛）
checkSession → bridge.getSession → User / null；IPC 异常 → log.error + rethrow
validateSession → bridge.validateSession → 'valid'|'invalid'|'unknown'；IPC 异常 → 'unknown'
```

设计要点：

- **前端不持有会话副本**：权威会话在后端（Rust 端 `sessions/{oj_id}.json` + Provider token）。
  少一份副本就少一处「与后端不一致」的可能，也少一处凭证暴露面。
- 错误策略按调用场景分化：login/logout 上抛；checkSession 把 IPC 异常记录后交给 authStore，
  由 store 保持 unresolved、保留原用户并向路由/视图返回 false；validateSession 则归一为 `unknown`，
  调用方只面对三种业务语义。两者都避免把传输故障误判为会话失效，见 `types/user.md` 与 `service/auth/mod.md`。

## 测试

`src/services/__tests__/auth.service.spec.ts`（mock auth.bridge）锁定：login 成功返回/失败上抛、
logout「后端失败时异常上抛、不再做任何本地清理」契约、checkSession 三路径（有会话 / 确认 null / IPC 异常记录日志并上抛）、
validateSession 透传三态且「IPC 自身异常归一为 unknown 而非 invalid」契约。
