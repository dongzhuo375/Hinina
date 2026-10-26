# submission.spec.ts（E2E：提交流）

> 源文件：`e2e/specs/submission.spec.ts`；施工方案 §6.3 场景 5 的收敛子集。

## 职责

验证提交、防重复、评测轮询到终态、提交历史与 pending 复用的端到端行为。

## 核心类型/函数

三个用例：

1. **提交 → 评测中禁用重复提交 → 轮询到终态**：提交后断言按钮禁用（名匹配 `/评测中|提交中/`）、`role="status"` 显示「已提交 #9001」、`submit_code` 恰 1 次；终态 pill 断言用正则 `/最新记录: #9001 (Accepted|AC)/`——本地 pill（状态原词）会被紧随的摘要刷新接管为服务端缩写，fake 摘要零延迟时本地形态一闪而过，二者都是终态信号；终态后按钮恢复、`get_judgement` ≥ 2 次。
2. **提交历史反映会话内提交**：终态后经「提交记录」进评测页，断言 `#9001` 可见。
3. **pending 期间离开再回来复用同一提交**：评测序列用 `judgementSequence: ['Running']`（永不终态）让 pending 窗口覆盖整个用例 → 提交 → 路由往返（解题页卸载重挂）→ `pendingFor` 五元组仍匹配：按钮保持「评测中…」禁用、状态行仍显示 `#9001`、`submit_code` 不重发。**store 内部的五元组去重行由 `submissionStore.spec` 单测覆盖**——UI 全部调用方（含快捷提交弹窗）都以同一 getter 预守卫，E2E 不可直达，故只断言可观测面。
4. 另注：`list_contest_submissions` 支撑解题页摘要与历史页；状态筛选映射（HOJ 码 → 变体）在 fake 内实现。fake 的 `submit_code` 会校验 `displayId ↔ problemId` 与语言列表（评审六轮），因此本 spec 同时充当「提交参数契约」的端到端哨兵。

## 直接依赖

- `../fake-tauri`（`callCount` / `loginAndEnterContest` / `FakeOptions.judgementSequence`）
- `src/components/editor/EditorConsoleBar.vue`（pill / 状态行 / 提交按钮）、`src/views/ProblemSetView.vue`（往返导航）

## 被依赖

- 无（叶子 spec）

## 逻辑流程

进场 → 打开 A 题 → 提交（store 记 pendingSubmissions + 会话条目并启动 Poller）→ fake 按 1s±20% 返回序列状态 → 终态停轮、摘要刷新接管 pill；往返用例复用同一 pending 条目。
