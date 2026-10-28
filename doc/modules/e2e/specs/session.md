# session.spec.ts（E2E：会话失效）

> 源文件：`e2e/specs/session.spec.ts`；施工方案 §6.3 场景 6 的收敛子集。

## 职责

验证会话失效的全链路与域状态清理：sessionGuard 判定 → invalidateSession（logout + clearDomainState）→ 跳登录页，以及失效后可立即重新登录。

## 核心类型/函数

四个用例：

1. **Auth 错误触发状态清理并跳回登录页**：`failNext('load_workspace', 'Auth', …)` → 点击「打开题目 A」按钮触发加载 → 断言跳 `/login`、`calls` 含 `logout`（invalidateSession 的可观测凭证）、登录表单可见可编辑（表单为空时按钮禁用属正常校验行为）。**已知现象**：失效文案（`SESSION_INVALID_MESSAGE`）会被登录页挂载时的 `checkSession` 清空（`error = null`），用户实际看不到失效原因——待产品决策，见 `doc/cicd/m3-integration.md` §五。
2. **会话失效停止评测轮询（clearDomainState 的可观测凭证）**：`judgementSequence: ['Running']` 让轮询永不终态 → 提交并确认 `get_judgement` 已开始 → 注入 Auth 错误失效 → **先等 1.5s 让在途轮询落地，再观测 3s 断言计数停增**（若未停，3s 内按 1s±20% 间隔还会增长约 2-3 次）——对应 `clearDomainState` 里 `submissionStore.stopAllPolling()`。
3. **失效后重新登录可再次进场**：回到登录页重新提交表单 → 再次进入题目总览。
4. **提交在途时失效（端到端跨会话守卫，评审五轮）**：`delayNext('submit_code', 1500)` 让提留在途 → 用一次 `list_contest_submissions` 的 Auth 错误触发失效 → 等延迟响应真正落地（`expect.poll` 等 `submit_code` 计入调用）并再等两个轮询周期 → 断言 `get_judgement` 恒 0（旧提交未重建状态/未重启轮询）→ 重新登录回到同题，提交按钮可用、无「已提交」状态。**变异验证**：移除 submitCode 守卫后该用例恰红（观测到 `get_judgement = 2`）。

**错误注入路径**：用例 1 与 3 经总览页「打开题目 A」按钮注入（必然触发 `load_workspace`）；用例 2 选 B 题 chip（同题 chip 点击是 no-op 不触发加载，跨题 chip 才会加载并命中失效）。

## 直接依赖

- `../fake-tauri`（`callCount` / `loginAndEnterContest` / `failNext` / `judgementSequence`）
- `src/guards/sessionGuard.ts`（被测全链路）、`src/stores/session.ts`（clearDomainState）

## 被依赖

- 无（叶子 spec）

## 逻辑流程

进场 → 提交挂起轮询（按需）→ failNext 注入一次性 Auth 错误 → 任一命令失败经 `setIpcErrorObserver` 触发 `handleAuthFailure` → `invalidateSession` → `router.replace Login` → 断言清理后的可观测状态。
