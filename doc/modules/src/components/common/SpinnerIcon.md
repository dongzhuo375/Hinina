# SpinnerIcon（旋转加载图标）

> 源文件：`src/components/common/SpinnerIcon.vue`

## 职责

旋转加载 SVG（25% 底圈 + 90° 弧线 + `animate-spin`）的单点：提交按钮、状态 pill、评测中徽章等共用。尺寸/颜色经 class 透传（fallthrough 合并到根 svg），线宽经 `strokeWidth` 覆盖（状态 pill 用 2.5，按钮用默认 2）。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `strokeWidth` | prop `number \| string?` | 描边粗细，默认 2 |
| `class` | fallthrough attr | 尺寸（h-3/h-3.5/h-4）与颜色（如 text-[var(--color-warning)]） |

## 直接依赖

- `vue`（`defineProps`）

## 被依赖

- `components/common/SubmitButton.vue`
- `components/editor/EditorConsoleBar.vue`（最新记录 pill 的 pending 态）
- `components/problem/QuickSubmitDialog.vue`（评测状态行）
- `views/SubmissionsView.vue`（状态 pill 的评测中图标）
- `views/SubmissionDetailView.vue`（评测中徽章 + 大状态 pill）

## 逻辑流程

纯静态图标组件，`aria-hidden="true"`（装饰性，状态文本由相邻 span 提供）。

## 测试

`src/components/common/__tests__/SpinnerIcon.spec.ts` 锁定结构（底圈/弧线/animate-spin）、默认线宽、class 合并与 strokeWidth 覆盖。
