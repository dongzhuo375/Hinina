# ThemeSection（主题占位分组）

> 源文件：`src/components/settings/ThemeSection.vue`

## 职责

展示界面主题与编辑器主题的只读占位。没有业务状态、props、服务调用或配置写入；不启用暗色主题占位项。

## 核心类型/函数

无业务 API。仅复用 `SettingsSectionFrame` 与共享 `SETTINGS_INPUT` 样式，保持两个 disabled select 的原外观。

## 直接依赖

- `SettingsSectionFrame`
- `@/composables/useSettingsForm`（`SETTINGS_INPUT` 常量）

## 被依赖

- `views/SettingsView.vue`

## 逻辑流程

静态模板保持不变：界面主题显示浅色，编辑器主题指向解题页“编辑器设置”控制入口；当前保存事务不写 `AppConfig.theme`。

## 测试

由 SettingsView 七组顺序测试和 Playwright 样式验收覆盖，无需独立业务单测。
