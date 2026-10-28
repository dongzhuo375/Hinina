# submission-tone（提交状态 pill 样式）

> 源文件：`src/utils/submission-tone.ts`

## 职责

集中维护提交状态 pill 的默认 Tailwind/主题变量类串。`utils/submission.statusTone` 负责把评测状态映射为语义 tone，本模块只负责 tone 到视觉类串的映射，避免多个视图各维护一份 `TONE_CLASSES`。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `STATUS_TONE_CLASSES` | `Readonly<Record<StatusTone, string>>` | 六个 tone 的默认完整类串；pending 默认使用 `text-info`，system 与 neutral 有意共用灰色 |
| `statusToneClasses` | `(tone, overrides?) => string` | 返回默认类串；未知运行时 tone 回退 neutral。`overrides` 仅用于确有历史视觉差异的消费方 |

## 直接依赖

- `@/utils/submission`（仅 `StatusTone` 类型）

## 被依赖

- `views/SubmissionsView.vue` — 默认状态色
- `views/SubmissionDetailView.vue` — 默认状态色
- `components/problem/QuickSubmitDialog.vue` — 仅覆盖 `pending` 为原有 `text-warning`，其余沿用默认表
- `utils/__tests__/submission-tone.spec.ts`

## 逻辑流程

调用方先经 `statusTone(record.status)` 得到语义 tone，再调用 `statusToneClasses(tone)`。QuickSubmit 额外传入单项 `pending` 覆盖，冻结其历史琥珀色；不做全局改色，也不改变非终态旋转图标的独立 warning 样式。

## 测试

`src/utils/__tests__/submission-tone.spec.ts` 锁定六个 tone 的完整类串、system/neutral 共用、未知 tone 回退，以及 QuickSubmit 单项 pending 覆盖不影响其它 tone。
