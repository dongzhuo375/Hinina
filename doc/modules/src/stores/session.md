# session（会话级状态清理）

> 源文件：`src/stores/session.ts`

## 职责

登出、切换账号或**切换 OJ** 时统一清空会话级领域状态，避免上一位选手（或上一个 OJ）的比赛、题面、提交记录、榜单与编辑器代码残留到下一个会话。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `clearDomainState` | `() => void` | **先 `bumpSessionGeneration()`**（使所有在途 IPC 的迟到响应失效，见 [sessionGeneration.md](sessionGeneration.md)），再重置 contest / problem / submission / rank / announcement / workspace 六个 store；重置工作区前先取消其防抖同步定时器，重置榜单/公告前先停止各自的实时刷新轮询 |
| `resetSessionForOjSwitch` | `() => void` | OJ 切换的会话上下文重置（调用点：`useSettingsForm.onSwitchOj` 切换成功后）。= `clearDomainState()` + **匿名简报作废**（`contestStore.invalidateBrief()`：递增简报失效世代，在途旧 OJ brief 的回写被丢弃 + brief/briefBaseUrl/briefError 清空、briefState 置 idle —— 简报属旧 OJ，且 LoginView 的 canEnter 侦听器会按简报时间窗计算阶段，陈旧值会在新 OJ 有会话时立即自动推进赛场，评审八轮）+ 认证态清零（`user = null` / `error = null` / **`sessionResolved = false`**，下次导航由路由守卫 `checkSession` 按新 OJ 的 `sessions/{id}.json` 自动恢复会话）。**刻意不调用 `authStore.logout()`**：Registry 已切到新 OJ，后端 logout 会拿新 OJ 的无凭证会话打它的登出端点、并误删新 OJ 自己的会话文件；各 OJ 会话文件按 id 隔离，**旧 OJ 登录态保留**（切回免登录） |

## 直接依赖

- `@/stores/contestStore`（`clearSessionData()`，保留匿名比赛简报）
- `@/stores/problemStore`
- `@/stores/rankStore`（`stopLive()` + `$reset()`）
- `@/stores/announcementStore`（`stopLive()` + `$reset()`）
- `@/stores/submissionStore`（`stopAllPolling()` + `$reset()`）
- `@/stores/workspaceStore`（`cancelPendingSync()` + `$reset()`）

## 被依赖

- `@/stores/authStore` — `logout()` 在清理认证态后调用
- `@/composables/useSettingsForm` — 「当前 OJ」切换成功后调用 `resetSessionForOjSwitch()`

## 逻辑流程

```
authStore.logout()
  → authService.logout()（后端会话 + localStorage，失败不阻断）
  → user = null
  → clearDomainState()
      → bumpSessionGeneration()              // 在途请求的迟到响应全部失效
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

useSettingsForm.onSwitchOj()（切换成功分支）
  → resetSessionForOjSwitch()
      → clearDomainState()                   // 同上：旧 OJ 的领域状态全部失效
      → contestStore.invalidateBrief()      // 作废旧 OJ 简报：递增世代（丢弃在途
                                             // 旧请求回写）+ brief 清空 + idle：
                                             // 简报属旧 OJ，登录页挂载时按新 OJ
                                             // 重拉（canEnter 决策不再被旧时间窗驱动）
      → authStore: user/error 清零 + sessionResolved = false
  → router.replace({ name: 'Login' })        // 守卫据此 checkSession：新 OJ 有
                                             // 会话则无感续用，否则落在登录表单
                                             // 注意：切换即离开 = 放弃设置页所有
                                             // 未保存的修改（下拉提示已明示）
```

设计要点：

- Pinia store 生命周期与组件无关，路由跳转不会自动清空状态，必须显式重置。
- **重置不等于取消在途请求**：清理只重置 state、停止定时器，已发出的 IPC 仍会返回。
  因此所有「请求发起 → await → 写回」的路径都要比对会话代际（`bumpSessionGeneration`
  由本模块触发，读取方见 [sessionGeneration.md](sessionGeneration.md)；已接入
  submissionStore 的提交/轮询/历史、workspaceStore 的加载、problemStore 的题面/
  limits/我的状态、contestStore 的比赛、rankStore 的榜单/全量快照、announcementStore
  的公告与已读；失败分支同样不写错误/加载态，避免旧会话污染新会话）。
- 认证状态由 `authStore` 自行清理，本模块只负责领域状态。**注意：与 `authStore` 存在函数级循环**（`authStore → clearDomainState`；`resetSessionForOjSwitch` 内读取 `authStore` 做认证态复位）——运行时安全（只在函数内调用、无模块初始化期求值），属既有基线；文档如实记录，不宣称「无反向依赖」（评审六轮）。
- `contestStore` 同时持有登录页的匿名比赛简报（不依赖会话），因此走定向清理
  `clearSessionData()` 而非 `$reset()`，避免切换账号时右侧氛围区与倒计时被清空。
- `rankStore` 的轮询器句柄在模块作用域（不在 state 里），`$reset()` 清不掉它，
  必须先 `stopLive()` 回收定时器，否则登出后轮询继续带着旧 `contestId` 打请求。
- 竞赛场景下机位账号常被复用，残留数据既是误操作风险也是信息泄露风险。
