# SettingsSectionFrame（设置分组 DOM/样式壳）

> 源文件：`src/components/settings/SettingsSectionFrame.vue`

## 职责

统一输出设置页真实 `<section>` 的 `id`、`tabindex="-1"` 与 `settings-section` class，并承载分组内部共享样式。它是纯 DOM/样式 frame，不产生第八个导航项，不持有业务状态。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `id` | prop `string` | 显式绑定 `settings-N`，供 `useSettingsScrollSpy` 的 `getElementById`、focus 和 scroll 使用 |
| default slot | slot | 保留各 section 的标题和正文 DOM 顺序 |

## 直接依赖

- `vue`（`defineProps`）

## 被依赖

- `components/settings/` 下七个 section 组件。

## 逻辑流程

section 模板通过默认 slot 原样进入 frame，DOM 中仍是单个真实 section，不增加额外 wrapper。`scroll-margin-top:24px` 与 scroll-spy 的 24px docked 判定保持一致；section 内部规则通过 scoped `:deep()` 覆盖 slotted 内容，避免父级 scoped CSS 无法作用到子组件后代。

## 测试

由 `SettingsView` 组合根测试验证七组 id/顺序；Playwright Settings 验收检查标题、输入框、数字等宽字体和响应式样式。
