# SubmitButton（提交代码三态按钮）

> 源文件：`src/components/common/SubmitButton.vue`

## 职责

「提交代码」按钮的三态单点：常态（纸飞机图标）→ 提交中（spinner + 「提交中…」）→ 已提交待评测（spinner + 「评测中…」，遮蔽再提交）。快速提交对话框与解题页控制台条共用；三态文案、disabled/aria-busy 判据单点维护。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `busy` | prop `boolean` | 提交请求进行中 |
| `pending` | prop `boolean` | 已提交、等待评测结果（防重复提交遮蔽） |
| `tone` | prop `'success' \| 'primary'` | 底色：对话框 success / 控制台条 primary（hover 色随之） |
| `tracking` | prop `boolean?` | 控制台条版本的文字加宽（tracking-wide） |
| `submit` | emit `() => void` | 点击提交 |

## 直接依赖

- `vue`（`defineProps` / `defineEmits` / `computed`）
- `components/common/SpinnerIcon.vue`

## 被依赖

- `components/problem/QuickSubmitDialog.vue`
- `components/editor/EditorConsoleBar.vue`

## 逻辑流程

`busy || pending` 即 working：disabled + aria-busy + spinner 替换纸飞机图标；文案按 busy → pending → 常态三级取值。

## 测试

`src/components/common/__tests__/SubmitButton.spec.ts` 锁定三态文案/图标/禁用、tone 底色与 tracking 文本类。
