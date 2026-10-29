# LayoutSection（分栏比例分组）

> 源文件：`src/components/settings/LayoutSection.vue`

## 职责

展示解题页分栏比例滑杆和百分比。只编辑 `splitRatio`，不在组件内自动保存；解题页分栏条的另一条自动保存路径不变。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `splitRatio` | prop `number` | 当前比例 |
| `splitPercent` | computed | `round(splitRatio * 100)` 展示 |
| `onSplitInput` | fn | 经共享 `clampRatio` 钳位后 emit `{ splitRatio }` |
| `update:form` | emit `SettingsFormPatch` | 通知父级更新同一中央草稿 |

## 直接依赖

- `vue`
- `SettingsSectionFrame`
- `@/composables/useSettingsForm`（`clampRatio` 与补丁类型）

## 被依赖

- `views/SettingsView.vue`

## 逻辑流程

滑杆 `0.30–0.70`、步长 `0.01` 与原实现一致。输入先经 `clampRatio`，再以补丁写回；加载回填与输入共用同一钳位函数。

## 测试

由 `useSettingsForm` 的 `clampRatio` 与保存载荷测试覆盖；组合根测试验证组件接线。
