# PageHeading（赛场通用页头）

> 源文件：`src/components/common/PageHeading.vue`

## 职责

赛场各主视图共用的页头：左侧渲染「编号 / 副标题」眉标（eyebrow）、主标题与可选描述，右侧渲染操作插槽。

## 核心类型/函数

**props**（无 emits）：

| 名称 | 类型 | 用途 |
|------|------|------|
| `index` | `string` | 页序编号（如 `01`），与眉标副标题以 ` / ` 拼接 |
| `subtitle` | `string` | 眉标副标题（英文大写短语，如 `PROBLEM COLLECTION`） |
| `title` | `string` | 主标题 |

**插槽**：`description`（具名，`v-if="$slots.description"` 才渲染描述行）、默认插槽（右侧操作区）。

无响应式状态与函数——纯展示组件；样式全部取自全局 `styles/contest.css` 的 `.page-heading` / `.page-title` / `.page-description` / `.eyebrow`。

## 直接依赖

无（仅 `defineProps`；样式依赖全局 `contest.css`）

## 被依赖

- `views/ProblemSetView.vue`（`01` / `PROBLEM COLLECTION`）
- `views/RankView.vue`（`02` / `LIVE SCOREBOARD`）
- `views/SubmissionsView.vue`（`03` / `SUBMISSION ARCHIVE`）
- `views/AnnouncementsView.vue`（`04` / `BULLETIN BOARD`）
- `views/SettingsView.vue`（`05` / `PREFERENCES`）

## 逻辑流程

```
<header class="page-heading">
  左：.eyebrow（{{ index }} / {{ subtitle }}）+ h1.page-title（{{ title }}）+ .page-description（description 插槽）
  右：默认插槽（操作按钮等）
```

设计要点：

- **统一五个主视图的页头版式**：编号制眉标 + 大标题 + 右插槽；此前各视图各自拼
  `<h1>` 与说明文案，字号/间距/编号口径不一致，视觉上像五个不同应用。
- **样式外置到 `contest.css`**：页头字号随 `--ui-step`（按窗口宽高共同放大的响应式密度）
  一起缩放，5 个页面不会各自漂移；组件本身零样式，避免 `<style scoped>` 与全局令牌打架。
- **描述与操作都走插槽**：不同页面需要的内容形态差异大（筛选工具条、刷新按钮、刷新时间），
  用插槽而不是把十来个 prop 堆进组件。
