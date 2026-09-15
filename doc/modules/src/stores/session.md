# session（会话级状态清理）

> 源文件：`src/stores/session.ts`

## 职责

登出或切换账号时统一清空会话级领域状态，避免上一位选手的比赛、题面、提交记录、榜单与编辑器代码残留到下一个会话。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `clearDomainState` | `() => void` | 重置 contest / problem / submission / rank / workspace 五个 store；重置工作区前先取消其防抖同步定时器，重置榜单前先停止实时刷新轮询 |

## 直接依赖

- `@/stores/contestStore`（`clearSessionData()`，保留匿名比赛简报）
- `@/stores/problemStore`
- `@/stores/rankStore`（`stopLive()` + `$reset()`）
- `@/stores/submissionStore`（`stopAllPolling()` + `$reset()`）
- `@/stores/workspaceStore`（`cancelPendingSync()` + `$reset()`）

## 被依赖

- `@/stores/authStore` — `logout()` 在清理认证态后调用

## 逻辑流程

```
authStore.logout()
  → authService.logout()（后端会话 + localStorage，失败不阻断）
  → user = null
  → clearDomainState()
      → workspaceStore.cancelPendingSync()   // 防止登出后仍向后端写入代码
      → workspaceStore.$reset()
      → submissionStore.stopAllPolling()     // 回收评测轮询定时器
      → submissionStore.$reset()
      → rankStore.stopLive()                 // 回收榜单轮询定时器
      → rankStore.$reset()                   // 「我的行」属会话数据，一并清空
      → contestStore.clearSessionData()      // 保留登录页匿名比赛简报
      → problemStore.$reset()
```

设计要点：

- Pinia store 生命周期与组件无关，路由跳转不会自动清空状态，必须显式重置。
- 认证状态由 `authStore` 自行清理，本模块只负责领域状态，避免循环依赖
  （`authStore → session → 各领域 store`，各领域 store 不反向依赖 `authStore`）。
- `contestStore` 同时持有登录页的匿名比赛简报（不依赖会话），因此走定向清理
  `clearSessionData()` 而非 `$reset()`，避免切换账号时右侧氛围区与倒计时被清空。
- `rankStore` 的轮询器句柄在模块作用域（不在 state 里），`$reset()` 清不掉它，
  必须先 `stopLive()` 回收定时器，否则登出后轮询继续带着旧 `contestId` 打请求。
- 竞赛场景下机位账号常被复用，残留数据既是误操作风险也是信息泄露风险。
