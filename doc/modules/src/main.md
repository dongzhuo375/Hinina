# main（前端组合根）

> 源文件：`src/main.ts`

## 职责

前端应用入口与组合根：创建 Vue 应用，装配 Pinia 与 Router，注入五个全局观察者/握手（**全局错误转发、会话守卫、工作区落盘事件、新公告事件、关窗落盘握手**），引入全局与赛场两套样式，挂载到 `#app`；DEV 构建另注册插件控制台助手 `window.hininaPlugins`（动态 import `pluginStore`，暴露 list/start/stop/reload/resync 五个控制方法，发布构建不含）。所有「底层不感知上层」的装配关系集中在此 —— 入口自身不含业务逻辑。

## 核心类型/函数

| 语句 | 用途 |
|------|------|
| `createApp(App)` | 创建根应用 |
| `app.use(createPinia())` | 装配 Pinia（全部 store 的前提） |
| `app.use(router)` | 装配路由（含 `beforeEach` 会话守卫，见 `router/index.md`） |
| `installGlobalErrorGuard(app, router, forward)` | **组合根注入**（**第一个装配，先于 mount 与其余 install** —— 首屏渲染/早期 IPC 的错误也要被捕获）：装 `app.config.errorHandler` + `window` `error`/`unhandledrejection` 监听 + `router.onError`，四类全局错误归一化（IpcError → `ipc:{cmd}`）后经注入的 `forward` 走 `bridge/diagnostics.bridge` 的 `forwardFrontendLog` → Rust `frontend_log` 命令落 `logs/hinina.log`（panic 钩子的前端半边）。`forward` 的 Promise 拒绝由组合根 catch 成 `log.warn` —— 报错的通道不允许自己成为新的错误源。见 `guards/globalErrorGuard.md` |
| `installSessionGuard(router)` | **组合根注入**：把「认证类 IPC 失败 → 判定会话失效 → 清理并回登录页」的观察者挂到 Bridge 层，随后传入 router 供失效后导航 |
| `installWorkspacePersistenceListener()` | **组合根注入**：订阅后端 `workspace-saved`（显式保存 / 后台 auto-save 成功）→ `workspaceStore.markPersisted()`，驱动「已自动备份」指示；订阅失败只降级指示器 |
| `installAnnouncementListener()` | **组合根注入**：订阅 `onAnnouncementsPublished`（Rust 比对公告基线后发布的新公告事件）→ 事件的 `contestId` 与 store 当前 `contestId` 一致时调 `announcementStore.refresh()`，红点随即点亮，不必等下一个 60s 轮询周期。**只认当前比赛**：切比赛瞬间可能有在途事件（旧比赛的新公告），误刷新会把旧比赛的公告拉进当前列表；订阅失败只经 `createLogger('main')` 记 `log.error`（F6 吸收：`main.ts` 已不用裸 console），红点退化为轮询发现（功能降级而非失效） |
| `installCloseFlushGuard()` | **组合根注入**：把真实窗口操作注入 `createCloseGuard({ flush, shouldReflush, close, destroy })`（`flush` = `workspaceStore.saveWorkspace()`；`shouldReflush` = `syncPending \|\| isDirty`，PR21-7：每轮落盘完成后复查在途改动，把关窗握手窗口内到达的落键也纳入落盘），`onCloseRequested` 里 `guard.handleRequest()` 返回 `true` 才 `event.preventDefault()`。状态机、双层时间上界（落盘 3s / 硬超时 5s）与有界多轮重刷全部收敛在 `utils/close-guard`，此处**只做装配**；收尾用 `destroy()` 而非 `close()` —— 落盘已完成，不需要再走一遍 `close-requested` 往返（那个往返正是原内联实现「窗口关不掉」的失效点）。**本函数不再持有任何 `proceedClose` 布尔量**：布尔量在 `close()` 抛错时会被永久卡在 `true`，此后每次点关闭都命中拦截，窗口再也关不掉（详见 `utils/close-guard.md` 失效模式分析） |
| `import '@/styles/global.css'` | 全局样式（CSS 变量主题、字体栈、榜单状态色等） |
| `import '@/styles/contest.css'` | 赛场主题样式（`.contest-theme` 作用域下的令牌重写 + 通用类：页头/表格/正文/浮层/响应式密度 `--ui-step`）；在 global.css 之后导入，保证赛场令牌覆盖 `:root` 同名变量 |
| `import 'katex/dist/katex.min.css'` | KaTeX 公式样式与字体（题面/公告/简介的 LaTeX 公式）；字体由 katex 包本地打包、不经 CDN，符合离线客户端约束 |
| `app.mount('#app')` | 挂载 |
| `if (import.meta.env.DEV) …` | **DEV-only 插件控制台助手**：动态 import `pluginStore`，`Object.defineProperty(window, 'hininaPlugins', …)` 冻结暴露 `list/start/stop/reload/resync`；跟随 Store → Service → Bridge 分层，发布构建不含此分支 |

注：Naive UI **不在此处 `app.use`**——`App.vue` 直接按需引入 `NConfigProvider` / `NDialogProvider` 组件（tree-shaking 友好，无全量插件安装）。

## 直接依赖

- `vue`（`createApp`）/ `pinia`（`createPinia`）
- `@/router` / `@/App.vue`
- `@/guards/globalErrorGuard`（`installGlobalErrorGuard`，见 `guards/globalErrorGuard.md`）
- `@/bridge/diagnostics.bridge`（`forwardFrontendLog` —— 错误转发的 Bridge 出口）
- `@/utils/logger`（`createLogger('main')` —— 装配失败的本地 console 告警与转发失败告警）
- `@/guards/sessionGuard`（`installSessionGuard`）
- `@/stores/workspaceStore`（`installWorkspacePersistenceListener` / `useWorkspaceStore` —— 关窗落盘握手注入的 `flush`）
- `@/stores/announcementStore`（`useAnnouncementStore` —— 新公告事件到达即 `refresh()`）
- `@/bridge/announcement.bridge`（`onAnnouncementsPublished` —— 新公告事件订阅；组合根可直接用 Bridge 层的订阅原语，装配链「Bridge ← 观察者 ← store」不反向）
- `@/utils/close-guard`（`createCloseGuard` —— 关窗守卫状态机与时间上界，见 `utils/close-guard.md`）
- `@tauri-apps/api/window`（`getCurrentWindow().onCloseRequested` / `close` / `destroy`，权限 `core:window:allow-close` 与 `core:window:allow-destroy` 已在 capabilities）
- `@/styles/global.css`
- `@/styles/contest.css`（赛场主题令牌与通用类；作用域仍靠 `.contest-theme` 类名限定）
- `katex/dist/katex.min.css`（公式渲染的消费前提，与 `utils/markdown.ts` 的 KaTeX 输出配套）

## 被依赖

- 无 — 前端顶层入口（`index.html` 加载）
- 反向关系：本文件是 `utils/close-guard.ts` 的**唯一生产消费方**（其余被依赖方为单测，见 `utils/close-guard.md` 的「被依赖」）；守卫本身零 import，不依赖窗口或 store，窗口操作由本文件注入

## 逻辑流程

```
createApp → use(pinia) → use(router)
  → installGlobalErrorGuard(app, router, forward)  // 四类全局错误 → Rust 日志（必须最先装）
  → installSessionGuard(router)              // 认证类 IPC 失败 → 清会话 + 回登录页
  → installWorkspacePersistenceListener()    // 后端落盘事件 → 清「编辑中…」指示
  → installAnnouncementListener()            // 新公告事件 → 刷新列表（红点不等 60s 周期）
  → installCloseFlushGuard()                 // 关窗前落盘握手（守卫实现在 utils/close-guard）
  → mount('#app')

装配顺序含义：
  Pinia 先于 Router —— router.beforeEach 中调用 useAuthStore() 需要活动 Pinia 实例
  globalErrorGuard 第一个装 —— errorHandler 不在位时首屏渲染错误直接逃逸
  sessionGuard 在 mount 前注入 —— 首屏任何 IPC 失败都能被观察者捕获
  其余三个订阅/握手同样在 mount 前 —— 首屏事件（落盘完成、新公告）与「刚打开就关窗」都不漏

关窗握手的控制流（本文件只做注入）：
  onCloseRequested(event)
    guard.handleRequest() 返回 true  → event.preventDefault()（拦截，守卫在途负责最终关闭）
                        返回 false → 放行（守卫已进入 allowing / 正在 destroy）
```

设计要点：

- **依赖倒置的落地点**：`bridge/index.ts` 只回调注入的观察者、不 import store/router；
  sessionGuard 依赖 authStore 与 router。这条「Bridge ← 观察者 ← sessionGuard → store/router」
  的装配链只能在组合根完成，避免底层反向依赖上层形成循环（详见 `guards/sessionGuard.md`）。
- **关窗落盘为何在组合根**：窗口事件是应用级关注点（不是某个视图的职责），而
  `workspaceStore` 的落盘动作由解题页与组合根共用；放在 `main.ts` 也保证用户停留在
  登录页/评测页关窗时同样不丢代码（写盘持锁的已知取舍见
  `doc/modules/src-tauri/service/workspace/manager.md` 的「持锁写盘与元数据取舍」）。
- **关窗策略为何不在组合根**：组合根只应回答「用哪个窗口、哪个 store」，不该回答
  「拦截几次、等多久、失败了怎么收尾」。策略一旦内联在此，失败路径就无法被单测穷尽
  —— 上一版正是这么丢掉「窗口关不掉」这条死路的（`utils/close-guard.md`）。
- **公告刷新为何是「事件 + 轮询」双通道**：拉取动作必须有人周期性发起（Rust 侧不做无源
  轮询），故 60s 节拍仍留在前端 store；而「有新公告」是状态变更，由后端事件总线立即下发。
  组合根是唯一知道「事件通道名 ↔ store」的地方，因此订阅在此装配。
- 入口保持零业务逻辑：不读配置、不发请求，会话恢复由路由守卫在首次导航时统一触发；
  落盘钩子只调用 store 的既有动作，不自带保存策略。


## 测试（M2）

`src/__tests__/main.spec.ts`：五个装配副作用（全局错误转发/会话守卫/工作区落盘订阅/公告订阅/关窗握手）注册齐备、全局错误转发**先于**会话守卫装配（`invocationCallOrder` 在 beforeAll 当场快照，规避 clearMocks）、公告事件按 `contestId` 过滤（切比赛瞬间的旧比赛在途事件被丢弃）。
