# useTransientValue（临时反馈状态）

> 源文件：`src/composables/useTransientValue.ts`

## 职责

在指定时长内持有一个临时值，随后恢复初始值。用于“已复制”等短期 UI 反馈，只管理状态与唯一 timer，不封装剪贴板、网络请求或其它业务动作。

## 核心类型/函数

```ts
useTransientValue<T>(initialValue: T, durationMs: number): {
  value: Readonly<Ref<T>>
  show(next: T): void
  clear(): void
}
```

- `show(next)`：取消旧 timer，写入新值并启动唯一回弹 timer。
- `clear()`：立即恢复初值并取消待执行回弹。
- 到期恢复 `initialValue` 并清空 timer 句柄。
- `onScopeDispose` 清理 timer；scope 销毁后 `show/clear` 不再写状态或创建 timer。
- timer 句柄是普通变量，不进入响应式系统；每个调用方拥有独立实例和时长。

## 直接依赖

- `vue`（`ref` / `onScopeDispose` / `Ref` 类型）

## 被依赖

- `components/problem/ProblemStatement.vue` — 样例下标，1500ms；剪贴板失败仍记录日志
- `views/SubmissionDetailView.vue` — boolean，1500ms；保留 execCommand fallback，路由切换调用 `clear`
- `views/SettingsView.vue` — About 复制 key，2000ms；剪贴板失败保持静默
- `composables/__tests__/useTransientValue.spec.ts`

## 逻辑流程

业务动作成功后调用 `show(next)`，临时值在时长结束前供模板读取。连续操作不会留下多个 timer；路由切换等显式复位调用 `clear()`。组件卸载由 scope 统一回收，无需各视图重复编写 timer 清理样板。

## 测试

`src/composables/__tests__/useTransientValue.spec.ts` 使用 fake timers 与 `effectScope` 锁定：到期回弹、连续 show 取消旧 timer、clear、不同实例/时长独立、dispose 清理以及销毁后不再写状态。`SubmissionDetailView.spec.ts` 另覆盖复制反馈在 A→B 路由切换时立即复位。
