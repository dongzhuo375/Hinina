# useSettingsScrollSpy（设置页分组导航状态机）

> 源文件：`src/composables/useSettingsScrollSpy.ts`

## 职责

为设置页左侧索引提供滚动高亮与点击定位：根据真实滚动容器计算当前分组，点击时锁定目标以避免平滑滚动途中闪烁，并在到位或超时后交还滚动驱动。每个组件实例独立持有锁和 timer。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `SECTION_LABELS` | readonly string[] | 七个导航标签与 `settings-0..6` 的唯一顺序来源 |
| `scrollerRef` | `ref<HTMLElement \| null>` | 页面纵向滚动容器引用，由父级模板绑定 |
| `activeSection` | `ref<number>` | 当前分组下标，驱动导航 `active/aria-current` |
| `updateActiveSection` | `() => void` | 96px 判定线、真实 scrollable/atBottom、锁释放与目标停靠判定 |
| `scheduleActiveSection` | `() => void` | 滚动事件合并为一次下一 tick 扫描，避免同帧重复读取分区布局 |
| `scrollToSection` | `(index) => void` | 立即锁定目标，`scrollIntoView({ behavior:'smooth' })` + `focus({ preventScroll:true })`；监听 `scrollend`，3s 兜底 |

## 直接依赖

- `vue`（`ref` / `watch` / `nextTick` / `onScopeDispose` / `Ref` 类型）

## 被依赖

- `views/SettingsView.vue`
- `composables/__tests__/useSettingsScrollSpy.spec.ts`

## 逻辑流程

1. 普通滚动取最后一个顶边越过 `scrollerTop + 96px` 的分组；仅实际可滚动时 atBottom 才直接选最后一组。
2. 点击导航立即高亮并启动锁。锁持续到目标顶边到达 `scrollerTop + 24px ± 2px`、`scrollend` 或 3 秒兜底，不能在平滑滚动途中提前释放。
3. atBottom 只对目标本来就是最后一组算到位信号，防止从底部回点上方组时提前解锁。
4. 普通滚动事件经 `scheduleActiveSection` 合并到下一 tick 执行；`loading` 结束后经 `nextTick` 校正；scope dispose 释放锁、滚动监听和 timer。

## 测试

`src/composables/__tests__/useSettingsScrollSpy.spec.ts` 使用可控 DOM geometry 与 fake timers 覆盖：不可滚动、普通判定线、atBottom、非最后目标锁、24px 停靠、最后组释放、scope dispose 与 loading 后校正。`SettingsView.behavior.spec.ts` 继续提供真实组合根导航回归。
