# StatusBar（底部状态条）

> 源文件：`src/components/layout/StatusBar.vue`

## 职责

工作台底栏：左侧连接状态指示灯（已连接比赛服务器 / 连接异常），右侧版本号。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `connected` | computed | `contestStore.contest !== null && !contestStore.error` —— 复用 store 状态作为判据，**不额外发探测请求** |
| `version` | `__APP_VERSION__` | 版本号构建期注入（vite define ← package.json），与 LoginView 同源，避免写死后漂移 |

## 直接依赖

- `vue`
- `@/stores/contestStore`

## 被依赖

- `views/ContestLayout.vue` — 外壳底栏

## 逻辑流程

```
contestStore.contest / error 变化 → connected 重算 → 绿点脉冲「已连接比赛服务器」
                                                    或红点「连接异常」
```

设计要点：

- 连接状态不单独探活：比赛数据本身就是最好的探针，额外的心跳请求在赛场上是纯开销。
