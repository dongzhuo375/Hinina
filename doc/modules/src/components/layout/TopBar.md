# TopBar（顶栏）

> 源文件：`src/components/layout/TopBar.vue`

## 职责

工作台顶栏：Logo + 比赛阶段徽章 + 标题、剩余时间胶囊与进度条、比赛简介弹层（Markdown 渲染）、用户菜单（登出）、无边框窗口的自绘窗口控制（最小化/最大化/关闭），并作为 Tauri 拖拽区域。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `now` | `ref<number>`（秒） | 每秒 setInterval 刷新，驱动倒计时与阶段跃迁；onUnmounted 清理 |
| `phase` / `phaseBadge` | computed | `utils/contest.getContestPhase`（与登录页共用判据）→ 徽章文案与配色（进行中=绿脉冲 / 未开始=amber / 已结束=灰 / 未加载） |
| `timing` / `progressPercent` / `formatHMS` | computed/fn | 总时长 = end−start；已用钳制在 [0, total]（未开始为 0、结束后封顶）；amber 进度条占比 |
| `toggleBrief` | `() => Promise<void>` | 打开简介弹层时异步读 `configService.getOjBaseUrl()`，把描述中相对图片地址改写为绝对地址后 `renderMarkdown` |
| `handleLogout` | `() => Promise<void>` | `isLoggingOut` 防重复点击；`authStore.logout()` 不会 reject（后端失败也清理本地），`finally` 中必然 `router.replace(Login)`，不会滞留比赛页 |
| `minimize` / `toggleMaximize` / `close` | fn | `getCurrentWindow()`（@tauri-apps/api/window）自绘窗口控制 |

## 直接依赖

- `vue` / `vue-router`
- `@tauri-apps/api/window`（`getCurrentWindow`，窗口控制）
- `@/stores/authStore` / `@/stores/contestStore`
- `@/utils/contest`（`getContestPhase`）、`@/utils/markdown`（`renderMarkdown`）
- `@/services/config.service`（OJ 基址，用于简介图片改写）

## 被依赖

- `views/ContestLayout.vue` — 外壳顶栏

## 逻辑流程

```
每秒 tick → now → phase/timing 重算 → 徽章、倒计时、进度条响应式更新
简介按钮 → toggleBrief → 读 OJ 基址 → renderMarkdown(description, baseUrl) → v-html 弹层
用户菜单 → handleLogout → authStore.logout()（含 clearDomainState 领域清理）→ replace 登录页
```

设计要点：

- **drag region 与可交互元素严格分离**：`data-tauri-drag-region` 只加在 header 与纯展示
  容器上，按钮/菜单一律不带——否则点击会被拖拽区域吞掉。
- 阶段判据集中在 `utils/contest`，与登录页/榜单页共用，避免同一规则多处实现。
- 登出跳转放 `finally`：`authStore.logout()` 设计上不 reject，但即便未来行为变化，
  用户也不会被滞留在比赛页面。
- 图标全部内联 SVG（离线客户端不引外部字体/CDN，全项目统一约定）。
