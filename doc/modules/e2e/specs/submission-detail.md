# submission-detail.spec.ts（E2E：提交详情）

> 源文件：`e2e/specs/submission-detail.spec.ts`；2026-11 审计 C4 补缺。

## 职责

提交详情页两种形态的端到端行为：终态详情（状态/耗时/测试点/源代码回显）与
评测中详情（judging 形态 + 明细自动刷新占位 + 详情页自身轮询）。

## 核心类型/函数

两个用例：

1. **终态详情**：解题页输入可辨识源码后提交至终态 → 提交记录页「查看详情」进
   详情页 → 状态 pill（Accepted）、耗时（12 ms）、测试点明细（fake 按终态生成
   3 个 AC 用例 → 「3/3 通过」）、源代码区默认折叠，展开后为只读 Monaco 回显
   提交原文（monaco 按 token 切 span，断言用单 token 子串 `return 0`）。
   `get_submission_detail` / `get_submission_cases` 各 ≥1 次。
2. **评测中详情**：`judgementSequence: ['Running']`（永不终态）→ 详情页显示
   Judging 标签 + 「评测进行中，测试点结果将自动刷新…」占位；`get_submission_cases`
   恒 0（终态才拉明细）、`get_submission_detail` ≥2（详情页自身维持轮询）。

## 直接依赖

- `../fake-tauri`（`callCount` / `loginAndEnterContest` / `FakeOptions.judgementSequence`）
- `src/views/SubmissionDetailView.vue`、`src/views/SubmissionsView.vue`（「查看详情」按钮）

## 被依赖

- 无（叶子 spec）

## 逻辑流程

提交 → 终态 → 列表 → 详情（`submissions/:submitId` 路由复用同一组件，切换
submitId 时旧详情/测试点/轮询一律失效）；非终态详情按 ±20% 抖动轮询到终态后
补拉一次测试点。
