# main（前端组合根）

> 源文件：`src/main.ts`

## 职责

前端应用入口与组合根：创建 Vue 应用，装配 Pinia 与 Router，注入四个全局观察者/握手（会话守卫、工作区落盘事件、新公告事件、关窗落盘握手），挂载到 `#app`。所有「底层不感知上层」的装配关系集中在此 —— 入口自身不含业务逻辑。

## 核心类型/函数

| 语句 | 用途 |
|------|------|
| `createApp(App)` | 创建根应用 |
| `app.use(createPinia())` | 装配 Pinia（全部 store 的前提） |
| `app.use(router)` | 装配路由（含 `beforeEach` 会话守卫，见 `router/index.md`） |
| `installSessionGuard(router)` | **组合根注入**：把「认证类 IPC 失败 → 判定会话失效 → 清理并回登录页」的观察者挂到 Bridge 层，随后传入 router 供失效后导航 |
| `installWorkspacePersistenceListener()` | **组合根注入**：订阅后端 `workspace-saved`（显式保存 / 后台 auto-save 成功）→ `workspaceStore.markPersisted()`，驱动「已自动备份」指示；订阅失败只降级指示器 |
| `installAnnouncementListener()` | **组合根注入**：订阅 `onAnnouncementsPublished`（Rust 比对公告基线后发布的新公告事件）→ 事件的 `contestId` 与 store 当前 `contestId` 一致时调 `announcementStore.refresh()`，红点随即点亮，不必等下一个 60s 轮询周期。**只认当前比赛**：切比赛瞬间可能有在途事件（旧比赛的新公告），误刷新会把旧比赛的公告拉进当前列表；订阅失败只 `console.error` 记录，红点退化为轮询发现（功能降级而非失效） |
| `installCloseFlushGuard()` | **组合根注入**：把真实窗口操作注入 `createCloseGuard({ flush, close, destroy })`（`flush` = `workspaceStore.saveWorkspace()`），`onCloseRequested` 里 `guard.handleRequest()` 返回 `true` 才 `event.preventDefault()`。状态机与双层时间上界（落盘 3s / 硬超时 5s）全部收敛在 `utils/close-guard`，此处**只做装配**；收尾用 `destroy()` 而非 `close()` —— 落盘已完成，不需要再走一遍 `close-requested` 往返（那个往返正是原内联实现「窗口关不掉」的失效点）。**本函数不再持有任何 `proceedClose` 布尔量**：布尔量在 `close()` 抛错时会被永久卡在 `true`，此后每次点关闭都命中拦截，窗口再也关不掉（详见 `utils/close-guard.md` 失效模式分析） |
| `import '@/styles/global.css'` | 全局样式（CSS 变量主题、榜单状态色等） |
| `import 'katex/dist/katex.min.css'` | KaTeX 公式样式与字体（题面/公告/简介的 LaTeX 公式）；字体由 katex 包本地打包、不经 CDN，符合离线客户端约束 |
| `app.mount('#app')` | 挂载 |

注：Naive UI **不在此处 `app.use`**——`App.vue` 直接按需引入 `NConfigProvider` / `NDialogProvider` 组件（tree-shaking 友好，无全量插件安装）。

## 直接依赖

- `vue`（`createApp`）/ `pinia`（`createPinia`）
- `@/router` / `@/App.vue`
- `@/guards/sessionGuard`（`installSessionGuard`）
- `@/stores/workspaceStore`（`installWorkspacePersistenceListener` / `useWorkspaceStore` —— 关窗落盘握手注入的 `flush`）
- `@/stores/announcementStore`（`useAnnouncementStore` —— 新公告事件到达即 `refresh()`）
- `@/bridge/announcement.bridge`（`onAnnouncementsPublished` —— 新公告事件订阅；组合根可直接用 Bridge 层的订阅原语，装配链「Bridge ← 观察者 ← store」不反向）
- `@/utils/close-guard`（`createCloseGuard` —— 关窗守卫状态机与时间上界，见 `utils/close-guard.md`）
- `@tauri-apps/api/window`（`getCurrentWindow().onCloseRequested` / `close` / `destroy`，权限 `core:window:allow-close` 与 `core:window:allow-destroy` 已在 capabilities）
- `@/styles/global.css`
- `katex/dist/katex.min.css`（公式渲染的消费前提，与 `utils/markdown.ts` 的 KaTeX 输出配套）

## 被依赖

- 无 — 前端顶层入口（`index.html` 加载）
- 反向关系：本文件是 `utils/close-guard.ts` 的**唯一生产消费方**（其余被依赖方为单测，见 `utils/close-guard.md` 的「被依赖」）；守卫本身零 import，不依赖窗口或 store，窗口操作由本文件注入

## 逻辑流程

```
createApp → use(pinia) → use(router)
  → installSessionGuard(router)              // 认证类 IPC 失败 → 清会话 + 回登录页
  → installWorkspacePersistenceListener()    // 后端落盘事件 → 清「编辑中…」指示
  → installAnnouncementListener()            // 新公告事件 → 刷新列表（红点不等 60s 周期）
  → installCloseFlushGuard()                 // 关窗前落盘握手（守卫实现在 utils/close-guard）
  → mount('#app')

装配顺序含义：
  Pinia 先于 Router —— router.beforeEach 中调用 useAuthStore() 需要活动 Pinia 实例
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
  登录页/评测页关窗时同样不丢代码（相关未处理项见 `doc/problem.md` 的「遗留（归档自已清除的修复记录）」段）。
- **关窗策略为何不在组合根**：组合根只应回答「用哪个窗口、哪个 store」，不该回答
  「拦截几次、等多久、失败了怎么收尾」。策略一旦内联在此，失败路径就无法被单测穷尽
  —— 上一版正是这么丢掉「窗口关不掉」这条死路的（`utils/close-guard.md`）。
- **公告刷新为何是「事件 + 轮询」双通道**：拉取动作必须有人周期性发起（Rust 侧不做无源
  轮询），故 60s 节拍仍留在前端 store；而「有新公告」是状态变更，走 EventBus 立即下发。
  组合根是唯一知道「事件通道名 ↔ store」的地方，因此订阅在此装配。
- 入口保持零业务逻辑：不读配置、不发请求，会话恢复由路由守卫在首次导航时统一触发；
  落盘钩子只调用 store 的既有动作，不自带保存策略。
