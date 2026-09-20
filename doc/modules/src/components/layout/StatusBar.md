# StatusBar（底部状态条）

> 源文件：`src/components/layout/StatusBar.vue`

## 职责

工作台底栏：左侧连接状态指示灯（已连接比赛服务器 / 连接异常），右侧版本号（**同时是设置入口的隐藏解锁点**，连点 5 下）。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `connected` | computed | `contestStore.contest !== null && !contestStore.error` —— 复用 store 状态作为判据，**不额外发探测请求** |
| `version` | `__APP_VERSION__` | 版本号构建期注入（vite define ← package.json），与 LoginView 同源，避免写死后漂移 |
| `onVersionClick` | `fn` | 调 `tapVersion()`；**仅在「刚刚解锁」那一次**给出 3 秒提示「设置已解锁」 |
| `unlockedHint` | ref | 解锁提示的可见性（3 秒后自动隐藏，`onUnmounted` 清定时器） |

## 直接依赖

- `vue`
- `@/stores/contestStore`
- `@/utils/settings-access`（`tapVersion`）
- `@/utils/logger`（`createLogger('StatusBar')` —— 解锁只记日志，不给可点击的视觉暗示）

## 被依赖

- `views/ContestLayout.vue` — 外壳底栏

## 逻辑流程

```
contestStore.contest / error 变化 → connected 重算 → 绿点脉冲「已连接比赛服务器」
                                                    或红点「连接异常」
版本号 click → tapVersion()（连点 5 下解锁，见 utils/settings-access）
  ├─ 未解锁 → 无任何反馈（刻意）
  └─ 刚刚解锁 → 显示「设置已解锁」3 秒 + 记日志
```

设计要点：

- 连接状态不单独探活：比赛数据本身就是最好的探针，额外的心跳请求在赛场上是纯开销。
- 版本号按钮**刻意不给可点击的视觉暗示**（无 `title`、`cursor-default`、hover 无变化）：它要防的是误触，不是引导用户去点。连点本身没有反馈，故解锁瞬间给一次 3 秒提示 —— 否则设置项若不在当前视野内，用户无法判断是否生效。

