# sessionGeneration（会话代际计数器）

> 源文件：`src/stores/sessionGeneration.ts`

## 职责

提供进程级的**会话代际**计数：`clearDomainState()`（登出 / 会话失效 / OJ 切换）递增，各 store 在「请求发起 → await → 写回」路径上捕获并比对，丢弃旧会话的迟到响应。刻意做成不依赖任何 store 的**叶子模块**，避免编排层 `session.ts` 与 store 之间形成 import 环（评审四轮 #6）。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `getSessionGeneration` | `() => number` | 返回当前代际；在请求发起时捕获 |
| `bumpSessionGeneration` | `() => void` | 递增代际；**仅** `stores/session.ts` 的 `clearDomainState()` 调用（`resetSessionForOjSwitch` 经它覆盖） |

无状态、无副作用句柄清理需求：进程内单调递增的标量，测试之间无需复位（每个用例捕获自己的起始值再比对）。

## 直接依赖

- 无（叶子模块）

## 被依赖

- `@/stores/session`（`bumpSessionGeneration`）
- `@/stores/submissionStore`（`submitCode` / `startPolling` / `fetchHistory` 的写入与收尾）
- `@/stores/workspaceStore`（`loadWorkspace` 的 flush 后、**队列 turn 内**、apply 前置 await 后与失败分支；队列 turn 的会话检查保证清理后已排队的旧加载不发 `load_workspace` IPC —— 评审七轮）
- `@/stores/problemStore`（`openProblem` / `loadLimits` / `loadMyStatus`）
- `@/stores/contestStore`（`loadContest`）
- `@/stores/rankStore`（`loadRank` / `fetchAllRows` 的中止检查点）
- `@/stores/announcementStore`（`load` / `markAllRead`）

## 逻辑流程

```
clearDomainState()
  → bumpSessionGeneration()          // 先失效所有在途请求
  → cancelPendingSync / stopAllPolling / stopLive / $reset …

store 内的写回路径
  const gen = getSessionGeneration() // 请求发起时
  await service…
  if (gen !== getSessionGeneration()) return   // 旧会话响应：不写状态、不写 error、不动加载态
  …写回…
  finally { if (gen === getSessionGeneration()) …收尾… }
```

边界：`contestStore.loadBrief`（登录页匿名简报）**不比对代际**——登出时简报按设计保留；其余会话域数据（比赛、题面、我的状态、提交历史、榜单、公告）全部比对。`submissionStore.history` 因 `$reset()` 整体替换嵌套对象，迟到写入落在旧引用上（本身不泄漏），其守卫是纵深防御。`workspaceStore.markPersisted`（`workspace-saved` 事件）**也不加代际守卫**：事件不带代际，加了会误挡新会话自己的落盘事件；其跨会话安全性由后端 revision 进程级全局单调 + 仅在磁盘写入后发布 + `workspaceId`/`syncPending` 过滤保证，且本地修订水位**跨会话保留**（`cancelPendingSync` 默认不归零，评审七轮；归零会让旧事件误清新会话的脏标记）。`contestStore` 的**在途去重**另需按代际判定：`loadInFlight` 登记代际，跨会话不复用旧 promise，`clearSessionData()` 一并注销。
