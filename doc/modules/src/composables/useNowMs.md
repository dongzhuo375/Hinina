# useNowMs（响应式墙钟）

> 源文件：`src/composables/useNowMs.ts`

## 职责

为多个视图提供同一形式的响应式当前时间：setup 时读取一次 `Date.now()`，组件 mounted 后每秒重新读取，scope 销毁时清理 interval。只提供时间状态，不包含赛程、倒计时、封榜或轮询业务逻辑。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `useNowMs` | `() => Readonly<Ref<number>>` | 返回 epoch 毫秒；调用方可按需要派生秒、倒计时或相对时间 |

- 不使用 `now += 1`，系统时间调整或后台定时器节流后在下一拍追上真实墙钟。
- mounted 时不额外立即 tick，初始值在 setup 已读取。
- 每个组件实例拥有独立 interval，不是全局单例时间源。

## 直接依赖

- `vue`（`ref` / `onMounted` / `onScopeDispose` / `Ref` 类型）

## 被依赖

- `components/layout/TopBar.vue` — 派生 epoch 秒，驱动阶段、剩余时间和进度
- `views/ContestLayout.vue` — 派生秒，驱动赛程阶段；2 秒退出 timer 仍由视图持有
- `views/LoginView.vue` — 派生秒，驱动倒计时与开赛进场；简报/预检 timer 不动
- `views/RankView.vue` — 直接使用毫秒并派生秒，保留 `updatedText` 的亚秒边界
- `composables/__tests__/useNowMs.spec.ts`

## 逻辑流程

1. setup 读取当前毫秒时间。
2. mounted 创建独立 `setInterval(1000)`，每拍重新读取 `Date.now()`。
3. scope dispose 清理 interval；业务 watcher、页面退出和其它 timer 仍归各消费方管理。

## 测试

`src/composables/__tests__/useNowMs.spec.ts` 使用组件挂载与 fake timers 验证：初始值、999/1000ms 边界、系统时间跳变后跟随墙钟、卸载清理，以及多实例独立。`LoginView.spec.ts` 与 `ContestLayout.spec.ts` 继续作为业务集成回归。
