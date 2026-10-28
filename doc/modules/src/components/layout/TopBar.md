# TopBar（顶栏）

> 源文件：`src/components/layout/TopBar.vue`

## 职责

工作台顶栏，**两行式**：第一行窗口条（品牌 + 比赛标题、比赛简介按钮、账号菜单、无边框窗口自绘控制，整体为 Tauri 拖拽区）；第二行导航/计时区（内嵌 `ActivityBar` 横向导航 + 阶段徽章 + 剩余时间 + 总时长 + 进度条）。比赛简介以浮层展示（Markdown 渲染）。

## 核心类型/函数

**props**：`navigationDisabled?: boolean` —— 透传到导航区的 `:inert`（比赛 `ended` / `upcoming` 时禁用导航）。

| 名称 | 签名 | 用途 |
|------|------|------|
| `nowMs` / `now` | `useNowMs()` / computed（秒） | 共享毫秒墙钟派生秒，驱动倒计时与阶段跃迁；scope dispose 清理 interval |
| `contest` | computed | `contestStore.contest` |
| `phase` | computed | `getContestPhase(contest, now)`（与登录页共用判据） |
| `phaseBadge` | computed | 四态徽章：进行中 / 未开始 / 已结束 / 未加载，配色全部取 CSS 令牌（`--color-ac-*`、`--color-pending-*`、`--border-color`），不再硬编码色值 |
| `timing` | computed | `total = max(1, end - start)`；`elapsed = clamp(0, total, now - start)`（未开始为 0、结束后封顶）；`remaining` |
| `formatHMS` | `(secs: number) => string` | 补零 `HH:MM:SS` |
| `progressPercent` | computed | `elapsed / total * 100`，驱动进度条宽度 |
| `toggleBrief` | `() => Promise<void>` | 打开简介浮层时异步读 `configService.getOjBaseUrl()`，把描述中相对图片地址改写为绝对地址后 `renderMarkdown` |
| `handleLogout` | `() => Promise<void>` | `isLoggingOut` 防重复点击；`authStore.logout()` 不会 reject（后端失败也清理本地），`finally` 中必然 `router.replace(Login)`，不会滞留比赛页 |
| `userInitial` | computed | 账号首字母（大写），账号菜单头像位 |
| `minimize` / `toggleMaximize` / `close` | fn | `getCurrentWindow()`（@tauri-apps/api/window）自绘窗口控制 |

## 直接依赖

- `vue` / `vue-router`
- `@tauri-apps/api/window`（`getCurrentWindow`，窗口控制）
- `@/components/layout/ActivityBar.vue`（内嵌横向导航）
- `@/stores/authStore` / `@/stores/contestStore`
- `@/utils/contest`（`getContestPhase`）、`@/utils/markdown`（`renderMarkdown`）、`@/utils/focus-scope`（`vFocusScope` —— 简介浮层/账号菜单的焦点陷阱与 Esc 关闭）
- `@/services/config.service`（OJ 基址，用于简介图片改写）
- `@/composables/useNowMs`（毫秒墙钟，派生秒）

## 被依赖

- `views/ContestLayout.vue` — 外壳顶栏（`:navigation-disabled`）

## 逻辑流程

```
每秒 tick → now → phase/timing 重算 → 徽章、剩余时间、进度条响应式更新
行 1 window-bar：品牌（Hinina + 比赛标题）｜比赛简介按钮｜账号菜单（登出，v-focus-scope）｜窗口三键
行 2 navigation-deck：<ActivityBar/> + 赛程仪表（阶段点 + 剩余时间 + 总时长 + 底部 2px 进度条）
简介按钮 → toggleBrief → 读 OJ 基址 → renderMarkdown(description, baseUrl)
        → .surface-overlay 浮层（v-html，正文用 .reading-body）
用户菜单 → handleLogout → authStore.logout()（含 clearDomainState 领域清理）→ replace 登录页
```

设计要点：

- **drag region 与可交互元素严格分离**：`data-tauri-drag-region` 只加在 `.window-bar` 与
  纯展示容器上，按钮/菜单一律不带——否则点击会被拖拽区域吞掉。
- **导航区从左侧竖栏移入顶栏第二行**（PR #36）：导航由竖排图标改为横排文字链接，
  左侧空间全部让给内容；`navigationDisabled` 同时驱动 `:inert`，
  比赛未开始/已结束时导航不可点（`ContestLayout` 会同时叠加结束覆盖层）。
- 阶段判据集中在 `utils/contest`，与登录页/榜单页共用，避免同一规则多处实现。
- 登出跳转放 `finally`：`authStore.logout()` 设计上不 reject，但即便未来行为变化，
  用户也不会被滞留在比赛页面。
- **浮层复用 `utils/focus-scope`**：弹层打开即焦点入内、Tab 循环、Esc 关闭并归还焦点，
  键盘用户不必用鼠标找回位置。
- 图标全部内联 SVG（离线客户端不引外部字体/CDN，全项目统一约定）；
  响应式断点 1100px / 850px（窄屏隐藏总时长说明、品牌分隔线、比赛标题）。
