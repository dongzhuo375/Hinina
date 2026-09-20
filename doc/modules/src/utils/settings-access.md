# settings-access（设置入口的隐藏解锁）

> 源文件：`src/utils/settings-access.ts`

## 职责

实现「设置入口默认隐藏，连点底部版本号 5 下才出现」：提供可注入时间源的解锁状态机（`createUnlockGate`）+ 应用级单例（`settingsUnlocked` / `tapVersion`）。

## 核心类型/函数

| 名称 | 签名 | 说明 |
|------|------|------|
| `SETTINGS_UNLOCK_TAPS` | `5` | 解锁所需连点次数 |
| `SETTINGS_TAP_WINDOW_MS` | `2_000` | 相邻两次点击的最大间隔；超过即**从 1 重新计数**（而非继续累加，否则零散点击也能凑满） |
| `createUnlockGate(options?)` | `(options?: UnlockGateOptions) => UnlockGate` | 纯状态机（无 Vue 依赖）：`taps` / `windowMs` / `now` 均可注入 |
| `UnlockGate.tap()` | `() => boolean` | 记一次点击，返回**本次之后**是否已解锁 |
| `UnlockGate.unlocked()` | `() => boolean` | 当前是否已解锁 |
| `settingsUnlocked` | `Ref<boolean>` | 应用级单例状态，`ActivityBar` 据此决定是否渲染设置项 |
| `tapVersion()` | `() => boolean` | `StatusBar` 的点击入口；返回「**刚刚**解锁」（仅一次为 `true`，供一次性反馈） |

计数规则：间隔在窗口内则累加，超窗从 1 重新开始；达到 `taps` 即解锁。解锁后 `tap()` 直接返回 `true` 且不再计数（幂等）。

## 直接依赖

- `vue`（`ref` —— 仅用于应用级单例的响应式）

## 被依赖

- `components/layout/StatusBar.vue` — 版本号按钮调用 `tapVersion()`
- `components/layout/ActivityBar.vue` — `v-if="settingsUnlocked"` 控制设置项渲染

## 逻辑流程

```
版本号 click → tapVersion()
  ├─ gate.tap()（计数/判窗/解锁）
  ├─ settingsUnlocked.value = gate.unlocked()
  └─ 返回「刚刚解锁」→ StatusBar 给出 3 秒提示「设置已解锁」
ActivityBar 渲染时读 settingsUnlocked → 决定设置 router-link 是否出现
```

设计要点：

- **状态仅本次运行有效、不落盘**：解锁是「我现在要调试」的临时意图，不是配置。重启后重新隐藏 —— 否则一次误触会永久暴露设置入口，闸门形同虚设。
- **为什么藏**：设置页里是可改变客户端行为的开关（OJ 地址、轮询节拍、缓存）。赛场上误触后很难自查（改了服务器地址就再也连不上），藏进一个需要主动寻找的入口等于给「我不该点这个」加了一道最低成本的闸门。
- **不给可点击的视觉暗示**：版本号无 `title`、光标保持默认、hover 无变化 —— 它要防的是误触，不是引导用户去点。
- **不引入 Pinia store**：两个兄弟组件需要同一份状态，但它没有领域数据、不走 IPC、不参与数据流分层，一个会话级布尔量不值得为它多一个 store 文件。
- 判定逻辑抽成注入式状态机（`now` 可注入）以便穷尽单测：「间隔超窗重新计数」「窗口边界」「解锁后幂等」这些边界靠真实点击测不出来。

## 测试

`src/utils/__tests__/settings-access.spec.ts`：默认 5 下解锁、超窗重新计数、窗口边界取闭区间、解锁后幂等、自定义次数/窗口，以及单例 `tapVersion` 只在「刚刚解锁」那一次返回 `true`。
