# sessionGuard（全局会话守卫）

> 源文件：`src/stores/sessionGuard.ts`

## 职责

把"认证类 IPC 失败"统一解释为会话失效：清理本地会话并回到登录页，避免用户停留在受保护页面反复重试失败。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `installSessionGuard` | `(router: Router) => void` | 在组合根（`main.ts`）注册 IPC 错误观察者，仅调用一次 |
| `handleAuthFailure` | `(error: IpcError, router: Router) => Promise<void>`（模块内私有） | 认证错误的实际处理：判定 → 清理 → 必要时导航 |
| `invalidating` | 模块内私有标记 | 并发请求同时 401 时只处理一次（含清理过程中再次触发 401 的重入） |

## 直接依赖

- `@/bridge`（`setIpcErrorObserver`、`IpcError` 类型）
- `@/stores/authStore`（`useAuthStore`、`SESSION_INVALID_MESSAGE`）
- `vue-router`（仅 `Router` 类型 + 导航）
- `@/utils/logger`（`createLogger` —— 作用域日志）

## 被依赖

- `main.ts` — 组合根装配

## 逻辑流程

```
ipcInvoke 捕获错误 → IpcError（含 AppError variant）→ 通知观察者
  └─ handleAuthFailure
       ├─ variant !== 'Auth' 或正在处理中 → 忽略
       ├─ 本地无会话（如登录表单密码错误）→ 忽略，错误由调用方展示
       └─ 判定会话失效
            → log.warn 记录命令名与原因（不含参数；utils/logger 作用域日志）
            → authStore.invalidateSession(SESSION_INVALID_MESSAGE)
                 （复用 logout 清理链路：后端尽力清理 + 本地认证态 + 领域状态）
            → 当前路由 requiresAuth 时 replace 到登录页
```

设计要点：

- **依赖倒置**：Bridge 层只回调注入的观察者，不 import store/router；装配关系集中在组合根，
  避免"底层反向依赖上层"的循环依赖（`authStore → stores/session → 各领域 store` 仍为单向）。
- **只认 `Auth` 变体**：Rust 端登录失败同样映射为 `AppError::Auth`，故额外要求"本地存在会话"
  才判定失效，密码错误不会被误解释为会话被撤销。
- **重入保护**：`invalidateSession` 内部会再发一次 `logout` IPC，若它也返回认证错误，
  由 `invalidating` 标记拦截，不会递归。
- 与赛前预检（`utils/session-check`）互补：预检负责"提前发现"，本守卫负责"随时兜底"
  （含比赛进行中的 token 过期、服务端重启清空 Redis 等场景）。
