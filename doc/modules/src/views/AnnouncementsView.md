# AnnouncementsView（公告页）

> 源文件：`src/views/AnnouncementsView.vue`

## 职责

比赛公告卡片流页面：Markdown 正文渲染（相对图片 URL 改写为 OJ 绝对地址）、未读圆点与高亮边框、长文折叠/展开（仅对实际溢出的卡片显示切换按钮）、**在页面可见期间把列表标记为已读**（由 `isWatching` 声明「用户正在看」，标记动作归 store）；轮询不在本视图 —— 由外壳 `ContestLayout` 统一持有，保证 ActivityBar 红点在全部页面鲜活。

## 核心类型/函数

普通变量：`alive`（卸载标记）、`bodyEls: Map<string, HTMLElement>`（正文元素引用，非渲染状态）。

| 名称 | 签名 | 用途 |
|------|------|------|
| `baseUrl` | ref | OJ 基址（`configService.getOjBaseUrl()`，失败回退空串），供 `renderMarkdown` 把正文中的相对图片 URL 改写为绝对地址 |
| `headerTitle` | computed | `{比赛标题} 赛事公告` |
| `isContestFailed` / `isBootstrapping` / `isAnnouncementFailed` / `isEmpty` | computed | 四态分流：比赛加载彻底失败（整页 ErrorMessage）→ 首次加载 spinner → 公告失败且**无旧数据**（有旧数据时保留列表，错误交给下一轮轮询自愈）→ 空态 |
| `renderedContents` | computed | 公告 id → 正文 HTML 的 Map 缓存：内容或基址变化才重渲染，避免模板里反复调 `renderMarkdown`（出口已消毒，`v-html` 安全） |
| `formatTime` | `(epochSecs) => string` | createdAt（epoch 秒，与 Contest.startTime 同口径）→ zh-CN 本地化；非正值显示 `—` |
| `overflowIds` / `expandedIds` | `ref<Set<string>>` | 已判定溢出的公告 ID（展开后不再复测，标记保留）/ 用户手动展开的 ID |
| `setBodyRef` | `(id) => (el) => void` | 正文元素的 ref 回调工厂，维护 `bodyEls` |
| `measureOverflow` | fn | 溢出测量：`scrollHeight > clientHeight + 2` 才判定需要折叠；展开状态下二者相等，跳过复测；沿用仍在列表中的既有标记 |
| `toggleExpand` | fn | 展开/收起（替换 Set 触发响应式） |
| `bootstrap` / `retryAnnouncements` / `onRefresh` | — | 引导链 / 重试（无 contestId 时重走 bootstrap）/ 手动刷新按钮（`announcementStore.refresh()`）。`bootstrap` **不再显式调用 `markAllRead`** —— `store.load` 在 `isWatching` 为真时自己标记，避免「加载后标记」与「刷新后标记」两条路径各自维护导致语义漂移 |
| `isWatching` 维护 | `onMounted` / `onUnmounted` | 挂载即置 `announcementStore.isWatching = true`（**先于 bootstrap**：`load` 要据此把落地的那批标为已读），卸载置 `false`（离开页面后到达的新公告必须保持未读，红点才会亮） |

## 直接依赖

- `vue`
- 组件：`ErrorMessage` / `LoadingSpinner`
- `@/services/config.service`（`getOjBaseUrl`）
- stores：`announcementStore`（列表/已读/未读数）、`contestStore`（`whenLoaded` + 标题）
- `@/utils/markdown`（`renderMarkdown`）

## 被依赖

- `router/index.ts` — 路由 `Announcements`（`/contest/announcements`）；ActivityBar 入口带未读红点

## 逻辑流程

```
onMounted:
  announcementStore.isWatching = true           // 先声明「用户正在看」——load 据此标记已读
  baseUrl = await configService.getOjBaseUrl()  // 先于渲染取基址，图片 URL 一步到位
  bootstrap():
    contestStore.whenLoaded()                   // 失败 → isContestFailed 整页报错
    announcementStore.load(contestId)           // 列表 + 已读集合（store 内并行拉取）
                                                // isWatching && 页面可见 → store 内部 markAllRead()
                                                //（产品决策：看着 = 已读，红点随之消失）
  nextTick → measureOverflow()                  // 首测折叠

watch([announcements, baseUrl]) → nextTick → measureOverflow()
  // 轮询落新数据或基址就绪后正文高度变化，需要重测

onUnmounted:
  alive = false                                 // 只标记失效，**不停止轮询**（轮询归外壳 ContestLayout）
  announcementStore.isWatching = false          // 此后到达的新公告保持未读 → 红点亮
```

设计要点：

- **轮询所有权在外壳**：未读红点徽标在所有页面可见，公告轮询必须与工作台同生命周期
  （见 `ContestLayout.md`）；本视图卸载只置 `alive`，防止异步回调在卸载后写状态。
- **`isWatching` 是本页对 store 的唯一「语义输入」**：本页不判断「该不该标记已读」，
  只回答「我还在不在屏幕上」。这样停留期间轮询落地的新公告会被 store 一并标记（否则
  离开页面后会冒出一个内容早已看过的**假红点**），而切走期间落地的仍保持未读
  （`markAllRead` 在 `document.hidden` 时短路 —— 没看到就不该被吞掉）。
- **长文折叠按实测溢出而非字数**：`max-h-36` + 渐变遮罩，只有 `scrollHeight` 真正超出
  的卡片才显示「展开全部」——短公告不出现无意义按钮；已展开卡片不复测（展开后
  scrollHeight === clientHeight 会误判为不溢出）。
- **标记已读只走一条路径**：本页不再调 `markAllRead`，标记由 `store.load` 统一触发 ——
  视图侧调用会与 store 内部的标记逻辑并存，两条路径的判据（`isWatching`/页面可见性）
  迟早漂移，而漂移的后果是红点被吞（选手永远错过公告）。
- 正文样式 scoped `.prose` 与题面同风格；`renderMarkdown` 出口消毒是 `v-html` 的安全前提。
- 有旧数据时公告加载失败不打断阅读（列表保留，错误由下一轮外壳轮询自愈），
  只有「一条公告都没有」才整块换 ErrorMessage。
