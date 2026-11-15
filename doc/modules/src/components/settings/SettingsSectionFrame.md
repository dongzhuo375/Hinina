# SettingsSectionFrame（设置分组 DOM/样式壳）

> 源文件：`src/components/settings/SettingsSectionFrame.vue`

## 职责

统一输出设置页真实 `<section>` 的 `id`、`tabindex="-1"` 与 `settings-section` class、区块头（图标容器 + `<h2>` 标题 + 可选右侧提示），并承载分组内部共享样式。它是纯 DOM/样式 frame，不产生第八个导航项，不持有业务状态。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `id` | prop `string` | 显式绑定 `settings-N`，供 `useSettingsScrollSpy` 的 `getElementById`、focus 和 scroll 使用 |
| `title` | prop `string` | 区块头 `<h2>` 标题文本 |
| `hint` | prop `string?` | 区块头右侧提示（如维护者锁定说明）；不传则不渲染 |
| `#icon` | slot | 区块头图标的 SVG 内部元素（svg 容器由 frame 统一输出） |
| default slot | slot | 各 section 的正文 DOM |

## 直接依赖

- `vue`（`defineProps`）

## 被依赖

- `components/settings/` 下七个 section 组件。

## 逻辑流程

区块头由 frame 输出（七个 section 的同构头部单点）；头部 DOM 形态（`section > div:first-child > svg + h2 [+ span]`）是本组件 `:deep` 样式的目标（隐藏图标、重排首块 padding），不得改变层级。正文通过默认 slot 原样进入 frame，DOM 中仍是单个真实 section，不增加额外 wrapper。`scroll-margin-top:24px` 与 scroll-spy 的 24px docked 判定保持一致；section 内部规则通过 scoped `:deep()` 覆盖 slotted 内容，避免父级 scoped CSS 无法作用到子组件后代。

## 测试

由 `SettingsView` 组合根测试验证七组 id/顺序；Playwright Settings 验收检查标题、输入框、数字等宽字体和响应式样式。
