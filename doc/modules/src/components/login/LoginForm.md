# LoginForm（登录卡片）

> 源文件：`src/components/login/LoginForm.vue`

## 职责

登录页左侧登录卡片：未登录时渲染参赛账号/密码表单与提交按钮，已登录时渲染账号欢迎态与「切换参赛账号」按钮，并承载比赛简介折叠面板。纯受控组件——状态与业务编排全在 `LoginView`。

## 核心类型/函数

**v-model**：`username` / `password`（`defineModel<string>({ required: true })`，父级 `v-model:username` / `v-model:password`）。

**props**：

| 名称 | 类型 | 用途 |
|------|------|------|
| `isSubmitting` | `boolean` | 提交中：禁用输入与按钮、按钮文案「正在验证参赛资格」+ 加载线、表单 `aria-busy` |
| `isSigningOut` | `boolean` | 退出中：禁用「切换参赛账号」，文案「正在退出…」 |
| `isLoggedIn` | `boolean` | 表单态 / 欢迎态分支（标题「登录参赛系统」/「账号已验证」） |
| `accountName` | `string \| null` | 欢迎态展示的账号名（`title` 悬浮全文，超长截断） |
| `authError` | `string \| null` | 错误文案：`role="alert"` 呈现，并给两个输入框置 `aria-invalid` + `aria-describedby="login-error"` |
| `phase` | `ContestPhase` | 欢迎态状态文案分支 |
| `briefHtml` | `string` | 已渲染的比赛简介 HTML（`v-html`）；空串显示「暂无比赛简介」 |
| `briefVisible` | `boolean` | 简介面板开合（受控，切换经 emit） |

**emits**：`submit`、`switchAccount`、`toggleBrief`。

| 名称 | 签名 | 用途 |
|------|------|------|
| `showPassword` | `ref<boolean>` | 密码明文切换（`aria-pressed` + 图标切换） |
| `loginDisabled` | computed | `isSubmitting \|\| !username.trim() \|\| !password` —— 空账号/密码不可提交 |
| `accountStatus` | computed | 欢迎态文案：`upcoming`「账号已就绪，开赛后将自动进入赛场」/ `running`「比赛正在进行，正在打开赛场…」/ `ended`「比赛已结束，你已返回入口页面」/ 其它「账号已登录」 |

## 直接依赖

- `vue`（`computed` / `ref`）
- `@/utils/contest`（仅 `ContestPhase` 类型）

## 被依赖

- `views/LoginView.vue` — 左栏登录卡片（双向绑定账号密码，转发三个事件）

## 逻辑流程

```
isLoggedIn ? 欢迎态 : 登录表单
  欢迎态：账号行（当前账号 + Ready 徽标）→ accountStatus 状态行 → 「切换参赛账号」
          → authError（如有）
  表单：  账号输入 → 密码输入（可显隐）→ authError（如有）→ 提交按钮
          提交：@submit.prevent → emit('submit')（原生 required + loginDisabled 双重拦截）
  底部：  「查看/收起比赛简介」→ emit('toggleBrief')；展开时渲染 briefHtml（空则占位）
```

设计要点：

- **受控组件、零业务**：不做校验以外的逻辑（不调 store/service、不导航），因此可被任意容器复用与单测。
- **错误可达性**：`role="alert"` + `aria-invalid` + `aria-describedby` 三件套齐全——错误不能只靠颜色表达；`loginDisabled` 用 `trim()` 后的用户名，避免「只打了空格」也能提交。
- **提交中状态可感知**：`aria-busy` + 按钮文案 + 底部加载线（`prefers-reduced-motion` 下降级为静态满宽条），选手知道请求确实发出去了。
- **密码框等宽 + 字距**：便于逐字符核对长密码；明文切换按钮带 `aria-label` 与 `aria-pressed`。
- 简介面板 `max-h-48 overflow-y-auto`：长简介在卡片内滚动，不把提交按钮挤出视口。

## 测试

无独立 spec；由 `src/views/__tests__/LoginView.spec.ts` 覆盖（密码显隐、错误 `role=alert` 与 `aria-invalid`、提交禁用态与 `aria-busy`、成功后清空密码、简介折叠与 Markdown 渲染）。
