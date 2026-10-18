# ActivityBar（赛场导航条）

> 源文件：`src/components/layout/ActivityBar.vue`

## 职责

赛场**横向**导航条（内嵌于 `TopBar` 第二行）：题目 / 榜单 / 评测 / 公告四个主入口 + 末尾「环境设置」（默认隐藏，连点状态栏版本号 5 下才出现），按当前路由高亮并渲染底部指示线；公告入口挂**真实未读红点徽标**（数据源 `announcementStore.unreadCount`）。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `activeKey` | computed | 由 `route.path` 前缀推导当前高亮项：`/contest/problem*`→`problems`、`/contest/rank`→`rank`、`/contest/submissions`→`submissions`、`/contest/announcements`→`announcements`、`/contest/settings`→`settings`（`/contest/problems` 与 `/contest/problem/:displayId` 均归属「题目」，解题页仍高亮题目入口） |
| `showAnnouncementDot` | computed | `announcementStore.unreadCount > 0 && activeKey !== 'announcements'` —— 处于公告页时不显示（进入该页即全部已读，避免标记往返期间红点残留） |
| `links` | 常量数组 | 四个主入口 `{ key, name, label, number }`（编号 `01`–`04` 等宽显示），模板 `v-for` 渲染 |
| 设置项可见性 | `v-if="settingsUnlocked"` | 来自 `@/utils/settings-access` 的会话级单例；未解锁时**整个 router-link 不渲染**（不是 CSS 隐藏） |

## 直接依赖

- `vue`（`computed`）/ `vue-router`（`useRoute` + `router-link`，导航目标：ProblemSet / Rank / Submissions / Announcements / Settings）
- `@/stores/announcementStore`（仅 `unreadCount` 只读消费）
- `@/utils/settings-access`（`settingsUnlocked`）

## 被依赖

- `components/layout/TopBar.vue` — 顶栏第二行内嵌（**PR #36 由 `views/ContestLayout.vue` 左栏移入**）

## 逻辑流程

```
route.path 前缀匹配 → activeKey → 对应 router-link 高亮（底部 2px 边框指示 + aria-current="page"）
announcementStore.unreadCount > 0 且不在公告页 → 公告项渲染 5px 红点（.unread-mark）
settingsUnlocked（连点状态栏版本号 5 下解锁，见 utils/settings-access）→ 末尾「环境设置」渲染/移除
```

设计要点：

- 导航纯展示 + 路由跳转；唯一的 store 消费是公告未读数（只读 getter，不发起请求）。
- **从左侧竖栏改为顶栏横排文字链接**：视觉上让出左侧整列空间给内容；
  编号 `01..04` 用等宽字体作视觉锚点，窄屏时 `.contest-navigation` 横向滚动（不换行、不挤压）。
- **红点不造假**：徽标严格来自服务端公告列表与本地已读集合的差集，
  且带 `aria-label="有未读公告"`（红点不能只靠颜色传达）。
- **设置项默认隐藏**：见 `utils/settings-access` —— 设置页里是可改变客户端行为的开关
  （OJ 地址、轮询节拍、缓存），赛场误触后很难自查，故藏进需要主动寻找的入口。
