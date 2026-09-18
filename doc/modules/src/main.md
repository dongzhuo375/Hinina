# main（前端组合根）

> 源文件：`src/main.ts`

## 职责

前端应用入口与组合根：创建 Vue 应用，装配 Pinia 与 Router，注入全局会话守卫观察者，挂载到 `#app`。所有「底层不感知上层」的装配关系集中在此。

## 核心类型/函数

| 语句 | 用途 |
|------|------|
| `createApp(App)` | 创建根应用 |
| `app.use(createPinia())` | 装配 Pinia（全部 store 的前提） |
| `app.use(router)` | 装配路由（含 `beforeEach` 会话守卫，见 `router/index.md`） |
| `installSessionGuard(router)` | **组合根注入**：把「认证类 IPC 失败 → 判定会话失效 → 清理并回登录页」的观察者挂到 Bridge 层，随后传入 router 供失效后导航 |
| `installWorkspacePersistenceListener()` | **组合根注入**：订阅后端 `workspace-saved`（显式保存 / 后台 auto-save 成功）→ `workspaceStore.markPersisted()`，驱动「已自动备份」指示；订阅失败只降级指示器 |
| `installCloseFlushGuard()` | **组合根注入**：`onCloseRequested` 握手 —— 首次拦截 `preventDefault` → 落盘工作区 → 再次 `close()` 放行（一次性标志防重入）；落盘等待上限 `CLOSE_FLUSH_TIMEOUT_MS = 3000`，超时即放行，避免 IPC 无响应时窗口关不掉 |
| `import '@/styles/global.css'` | 全局样式（CSS 变量主题、榜单状态色等） |
| `import 'katex/dist/katex.min.css'` | KaTeX 公式样式与字体（题面/公告/简介的 LaTeX 公式）；字体由 katex 包本地打包、不经 CDN，符合离线客户端约束 |
| `app.mount('#app')` | 挂载 |

注：Naive UI **不在此处 `app.use`**——`App.vue` 直接按需引入 `NConfigProvider` / `NDialogProvider` 组件（tree-shaking 友好，无全量插件安装）。

## 直接依赖

- `vue`（`createApp`）/ `pinia`（`createPinia`）
- `@/router` / `@/App.vue`
- `@/stores/sessionGuard`（`installSessionGuard`）
- `@/stores/workspaceStore`（`installWorkspacePersistenceListener` / `useWorkspaceStore` —— 关窗落盘握手）
- `@tauri-apps/api/window`（`getCurrentWindow().onCloseRequested` / `close`，权限 `core:window:allow-close` 已在 capabilities）
- `@/styles/global.css`
- `katex/dist/katex.min.css`（公式渲染的消费前提，与 `utils/markdown.ts` 的 KaTeX 输出配套）

## 被依赖

- 无 — 前端顶层入口（`index.html` 加载）

## 逻辑流程

```
createApp → use(pinia) → use(router)
  → installSessionGuard(router)              // 认证类 IPC 失败 → 清会话 + 回登录页
  → installWorkspacePersistenceListener()    // 后端落盘事件 → 清「编辑中…」指示
  → installCloseFlushGuard()                 // 关窗前落盘握手
  → mount('#app')

装配顺序含义：
  Pinia 先于 Router —— router.beforeEach 中调用 useAuthStore() 需要活动 Pinia 实例
  sessionGuard 在 mount 前注入 —— 首屏任何 IPC 失败都能被观察者捕获
  落盘订阅与关窗钩子同样在 mount 前 —— 首屏编辑与立即关窗都不漏
```

设计要点：

- **依赖倒置的落地点**：`bridge/index.ts` 只回调注入的观察者、不 import store/router；
  sessionGuard 依赖 authStore 与 router。这条「Bridge ← 观察者 ← sessionGuard → store/router」
  的装配链只能在组合根完成，避免底层反向依赖上层形成循环（详见 `stores/sessionGuard.md`）。
- **关窗落盘为何在组合根**：窗口事件是应用级关注点（不是某个视图的职责），而
  `workspaceStore` 的落盘动作由解题页与组合根共用；放在 `main.ts` 也保证用户停留在
  登录页/评测页关窗时同样不丢代码（详见 `doc/problem.md` P74）。
- 入口保持零业务逻辑：不读配置、不发请求，会话恢复由路由守卫在首次导航时统一触发；
  落盘钩子只调用 store 的既有动作，不自带保存策略。
