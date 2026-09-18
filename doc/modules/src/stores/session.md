# session（会话级状态清理）

> 源文件：`src/stores/session.ts`

## 职责

登出、切换账号或**切换 OJ** 时统一清空会话级领域状态，避免上一位选手（或上一个 OJ）的比赛、题面、提交记录、榜单与编辑器代码残留到下一个会话。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `clearDomainState` | `() => void` | 重置 contest / problem / submission / rank / announcement / workspace 六个 store；重置工作区前先取消其防抖同步定时器，重置榜单/公告前先停止各自的实时刷新轮询 |
| `resetSessionForOjSwitch` | `() => void` | OJ 切换的会话上下文重置（调用点：SettingsView 切换成功后）。= `clearDomainState()` + 认证态清零（`user = null` / `error = null` / **`sessionResolved = false`**，下次导航由路由守卫 `checkSession` 按新 OJ 的 `sessions/{id}.json` 自动恢复会话）+ `authService.clearStoredUser()`。**刻意不调用 `authStore.logout()`**：Registry 已切到新 OJ，后端 logout 会拿新 OJ 的无凭证会话打它的登出端点、并误删新 OJ 自己的会话文件；各 OJ 会话文件按 id 隔离，**旧 OJ 登录态保留**（切回免登录） |

## 直接依赖

- `@/stores/contestStore`（`clearSessionData()`，保留匿名比赛简报）
- `@/stores/problemStore`
- `@/stores/rankStore`（`stopLive()` + `$reset()`）
- `@/stores/announcementStore`（`stopLive()` + `$reset()`）
- `@/stores/submissionStore`（`stopAllPolling()` + `$reset()`）
- `@/stores/workspaceStore`（`cancelPendingSync()` + `$reset()`）

## 被依赖

- `@/stores/authStore` — `logout()` 在清理认证态后调用
- `@/views/SettingsView` — 「当前 OJ」切换成功后调用 `resetSessionForOjSwitch()`

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
      → announcementStore.stopLive()         // 回收公告轮询定时器
      → announcementStore.$reset()           // 已读状态按用户隔离，不得跨会话残留
      → contestStore.clearSessionData()      // 保留登录页匿名比赛简报
      → problemStore.$reset()

SettingsView.onSwitchOj()（切换成功分支）
  → resetSessionForOjSwitch()
      → clearDomainState()                   // 同上：旧 OJ 的领域状态全部失效
      → authStore: user/error 清零 + sessionResolved = false
      → authService.clearStoredUser()        // localStorage 旧 OJ 用户缓存
  → router.replace({ name: 'Login' })        // 守卫据此 checkSession：新 OJ 有
                                             // 会话则无感续用，否则落在登录表单
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
