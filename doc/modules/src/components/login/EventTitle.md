# EventTitle（比赛事件标题）

> 源文件：`src/components/login/EventTitle.vue`

## 职责

登录页右侧氛围区的比赛事件标题：渲染响应式字号的 `<h1>`（衬线字体）与其下方一条细分隔线。

## 核心类型/函数

**props**（无 emits）：

| 名称 | 类型 | 用途 |
|------|------|------|
| `title` | `string` | 比赛标题文本（父级传 `brief?.title ?? 'Hinina 竞赛'`） |

无响应式状态与函数——纯展示组件。

## 直接依赖

无（仅 `defineProps`）

## 被依赖

- `views/LoginView.vue` — `:title="eventTitle"`

## 逻辑流程

```
<section aria-labelledby="event-title">
  <h1 id="event-title">  字号 clamp(2rem, 4.1vw, 3.8rem)，break-words，login-title（衬线字体）
  分隔线 h-px
```

设计要点：

- **`aria-labelledby` 绑定 `<h1>` 的 id**：把标题区块与标题文本显式关联，读屏按区块朗读时能拿到标题。
- **字号用 `clamp()` 而非断点**：登录页需要适配从普通窗口到最大化窗口的连续宽度，断点式跳变会让标题与倒计时比例失衡。
- **`break-words`**：比赛标题可能很长且无空格（如带副标题的英文全称），必须允许折行而不是横向溢出。
- 字体与字距由 `login-title` 类承担（登录页衬线字体），与 `LoginForm` 的标题同源。
