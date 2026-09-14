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

- `views/RankView.vue`（isFatalError：无任何数据时整块替换）、`views/ProblemSetView.vue`（showError）、`views/ProblemSolveView.vue`（viewError + `retry: () => load(displayId)`）

## 逻辑流程

```
message 渲染为居中提示；retry 存在 → 「重试」按钮 → 点击执行调用方回调
```

设计要点：

- **只用于阻断性错误**：有旧数据可展示的场景（如榜单轮询失败）由各视图用非阻断错误条
  代替（见 RankView 的错误分流），本组件不承担「带数据警告」职责。
- 无状态、不发起请求：重试策略归调用方，组件保持哑组件（dumb component）。
