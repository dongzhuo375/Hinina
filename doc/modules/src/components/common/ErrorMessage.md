# ErrorMessage（通用错误提示）

> 源文件：`src/components/common/ErrorMessage.vue`

## 职责

全局统一的阻断性错误态组件：警告图标 + 错误消息 + 可选「重试」按钮，供各视图在数据完全不可用时复用。

## 核心类型/函数

| 名称 | 类型 | 用途 |
|------|------|------|
| `message` | prop `string`（必填） | 错误文案（通常来自 store 的 `error`，即 IpcError 归一化后的真实原因） |
| `retry` | prop `(() => void)?` | 重试回调；提供时才渲染按钮（调用方决定重试语义，组件不自带请求逻辑） |

## 直接依赖

无（纯模板组件）

## 被依赖

- `views/ProblemSetView.vue`（showError）、`views/ProblemSolveView.vue`（viewError + `retry: () => load(displayId)`）、`views/RankView.vue`（isFatalError：无任何数据时整块替换）
- `views/SubmissionsView.vue` / `views/SubmissionDetailView.vue` / `views/AnnouncementsView.vue` / `views/SettingsView.vue`（阻断性错误态）

## 逻辑流程

```
<div role="alert">（居中，gap-4 / px-6 / py-12）
  内联 SVG 警告图标（--color-error，32×32 viewBox）
  message 文本（text-sm，max-w-md，break-words —— 长 OJ 错误信息不撑破布局）
  retry 存在 → 「重试」按钮（--color-primary / hover --color-primary-hover）→ 点击执行调用方回调
```

设计要点：

- **只用于阻断性错误**：有旧数据可展示的场景（如榜单轮询失败）由各视图用非阻断错误条
  代替（见 RankView 的错误分流），本组件不承担「带数据警告」职责。
- 无状态、不发起请求：重试策略归调用方，组件保持哑组件（dumb component）。
- **`role="alert"`**（PR #36）：错误必须被读屏即时播报，不能只靠视觉。
- 图标由 emoji `⚠` 换成内联 SVG：emoji 字形随系统/Browser 渲染差异大（Blink 与 WebView2
  不一致），赛场视觉不能被系统 emoji 决定；同时符合「不引外部字体」的离线约定。
