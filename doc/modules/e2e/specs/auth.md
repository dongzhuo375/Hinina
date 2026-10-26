# auth.spec.ts（E2E：认证与会话）

> 源文件：`e2e/specs/auth.spec.ts`；施工方案 §6.3 场景 1 的收敛子集。

## 职责

验证认证三态与刷新的端到端行为：登录失败、登录成功进场、会话恢复直达赛场、登录后 reload 会话仍在，以及事件注销前置全局的存在性哨兵。

## 核心类型/函数

四个用例：

1. **登录失败**：`failLogin` 预置 `{"Auth": "用户名或密码错误"}` → 提交表单 → 断言 `#login-error` 可见（`getByRole('alert')` 会撞上 Monaco 的 aria-alert 节点，必须用精确锚点）且停留 `/login`、按钮可再次提交（非死锁态）。
2. **登录成功**：正常登录 → `waitForURL /contest/problems` → 断言「题目总览」与 A/B 两张题目卡渲染（进入外壳链路：loadBrief + 路由守卫赛段放行 + ContestLayout bootstrap）。
3. **会话恢复**：`user` 预置 get_session 结果 → 深链 `/contest` → 直达赛场、登录表单不出现；**哨兵断言**：`window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener` 为 function（缺失时任何 unlisten 都会在 `@tauri-apps/api` 2.11 的 `_unlisten` 首行抛 TypeError）。
4. **登录后刷新恢复会话**（评审三轮 #4）：`loginAndEnterContest` 后 `page.reload()` → 仍直达题目总览、登录表单不出现。fake 的后端状态若只挂文档内存，reload 后 get_session 退回 null、守卫弹回登录页——本用例即该回归的哨兵（变异验证：无持久化时恰红）。

## 直接依赖

- `../fake-tauri`（`installFakeTauri`）
- 登录表单锚点：`#contest-username` / `#contest-password` / `#login-error`（`src/components/login/LoginForm.vue`）

## 被依赖

- 无（叶子 spec）

## 逻辑流程

每用例独立 browser context：`installFakeTauri` → `goto '/'` → 按场景走登录/深链 → 断言路由与界面状态。
