# App（根组件）

> 源文件：`src/App.vue`

## 职责

应用根组件：提供 Naive UI 配置/对话框上下文与**按路由分区切换的主题覆盖**，渲染窗口圆角外框与 `<router-view>`，并在**比赛外壳之外的路由**渲染兜底窗口控制条（拖拽区 + 最小化/最大化/关闭）。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `showGlobalTitleBar` | computed | `!route.matched.some(r => r.name === 'Contest')` —— 比赛外壳（ContestLayout）的 TopBar 自带窗口控制，外壳之外的路由（当前只有登录页）由此处提供精简窗口条，避免窗口无法关闭/最小化 |
| `contestThemeOverrides` | `GlobalThemeOverrides` | **赛场主题**：primary `#2e3033`、语义色与 `contest.css` 令牌对齐（info `#5b736d` / success `#50705c` / warning `#8c6a43` / error `#a33b2a`）、`borderRadius: '3px'`、`fontFamily: 'var(--font-sans)'` |
| `themeOverrides` | `GlobalThemeOverrides` | 入口页主题：primary `#7C5CFF` 系 + info/success/warning/error 语义色 |
| `minimize` / `toggleMaximize` / `close` | fn | `getCurrentWindow()`（@tauri-apps/api/window）自绘窗口控制（无边框窗口） |

模板结构：`n-config-provider(:theme="null" + :theme-overrides="showGlobalTitleBar ? themeOverrides : contestThemeOverrides")` → `n-dialog-provider` → **插件通知出口 `<PluginNotifications />`**（纯文本通知渲染，挂在 dialog 层内、根 div 之前）→ 根 div（`:class="{ 'contest-theme': !showGlobalTitleBar }"`）→ 条件渲染的兜底 header（`.entry-titlebar` + `data-tauri-drag-region`）→ `router-view`。

## 直接依赖

- `vue`（`computed`）/ `vue-router`（`useRoute`）
- `naive-ui`（`NConfigProvider` / `NDialogProvider` 组件 + `GlobalThemeOverrides` 类型；按需引入，非插件安装）
- `@tauri-apps/api/window`（`getCurrentWindow`）
- `@/components/PluginNotifications.vue`（插件通知出口组件）

## 被依赖

- `main.ts` — 根组件

## 逻辑流程

```
路由变化 → route.matched 是否含 name='Contest'
  ├─ 是（比赛工作台子树）→ 不渲染兜底窗口条（TopBar 已有窗口控制，避免双份按钮）
  │                        根节点挂 .contest-theme（启用赛场 CSS 令牌）
  │                        Naive UI 用 contestThemeOverrides
  └─ 否（/login 等）→ 渲染 40px 精简窗口条（品牌方块 + 拖拽区 + 三个窗口按钮）
                      Naive UI 用 themeOverrides（紫系）
```

设计要点：

- **判据用 `route.matched` 而非 `route.name`**：Contest 的子路由（ProblemSet/Rank…）
  name 各不相同，但 matched 链上都含外壳记录，一处判断覆盖整棵子树。
- **主题分区在根节点一处收口**：`.contest-theme` 类与 `contestThemeOverrides` 同判据切换，
  保证「CSS 变量」与「组件库语义色」两套主题不会各自漂移（详见 `styles/contest.css` 的令牌表）；
  登录页刻意保留紫系/木色入口气质，不被赛场中性色吞掉。
- `:theme="null"`：不启用 Naive UI 内置暗色主题，明暗由项目自身 CSS 变量驱动，
  themeOverrides 只对齐组件库的语义色。
- 拖拽区与可交互元素分离（`data-tauri-drag-region` 只加在 header 与纯展示容器上），
  与 TopBar 同一约定；图标内联 SVG。
