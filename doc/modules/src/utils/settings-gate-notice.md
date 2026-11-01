# settings-gate-notice（钥匙模式的阶段闸门提示）

> 源文件：`src/utils/settings-gate-notice.ts`

## 职责

钥匙模式下从设置页点赛场页面（题目/榜单等）时，守卫的阶段闸门会把导航拦回
设置页——vue-router 只 resolve 一个 `NavigationFailure`，用户看不到任何反馈，
而导航又不能禁用（那正是评审六修的 `inert`）。守卫写入本模块，SettingsView
渲染一行提示并自动清除（评审十五 B）。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `settingsGateNotice` | `Ref<string \| null>` | 当前提示文案；null = 无 |
| `markSettingsGateBlocked` | `(message: string) => void` | 记录提示并启动 4 秒自动清除（重复调用重置计时器） |
| `clearSettingsGateNotice` | `() => void` | 清空提示与计时器（SettingsView `onUnmounted` 调用——4 秒计时器不跨页残留） |

模块级单例（与 `settings-access` 同先例）：无领域数据、不走 IPC，不值得一个
store。

## 直接依赖

- `vue`（`ref`）

## 被依赖

- `router/index.ts` —— 钥匙设置页出发的导航被阶段闸门拦回时调用
- `views/SettingsView.vue` —— 渲染提示条（`role="status"`，warning 配色）

## 逻辑流程

```
守卫：fromKeyedSettings 且 phase !== running
  → markSettingsGateBlocked('比赛未进行，已留在设置页…')
  → return { name: 'Settings' }
SettingsView 读取 settingsGateNotice → 渲染提示条（`sticky top-2`：滚到底部改数据目录时也
在视口内，评审十五实测普通定位会被滚出视口）。提示条放在内容列 `.settings-sections`
内（自动继承索引栏左距）——放在外层整幅块级会横跨 float 索引栏、压住「01」首项
（评审十六视觉实测）
4 秒后（或再次触发时重置）自动清空
```

## 测试

`router/__tests__/launch-redirect.spec.ts`：钥匙设置页 + 已结束简报 → 导航
ProblemSet 后仍停在 Settings，且 `settingsGateNotice` 含「比赛未进行」。
自动清除依赖计时器，属实现细节，未单独断言。
