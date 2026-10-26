# workspace.spec.ts（E2E：题目与工作区）

> 源文件：`e2e/specs/workspace.spec.ts`；施工方案 §6.3 场景 3（部分）+ 场景 4 的收敛子集。

## 职责

验证「代码保留」承诺的端到端链路：题面按展示题号加载、编辑经 2s 防抖推送到后端内存、切题与回切按题目真实 pid 隔离恢复、推送的文件名与内容与编辑器一致。

## 核心类型/函数

两个用例：

1. **加载 → 编辑 → 切题 → 回切内容恢复**：打开 A 题（断言 `h1.statement-title` 与初始代码来自 load_workspace fixture）→ `.view-lines` 点击后键盘输入 `// e2e edited` → `expect.poll` 等 `update_workspace_file` 调用出现（防抖 2s，`workspaceStore.SYNC_DEBOUNCE_MS`）→ Tab 条切 B（断言 B 初始代码，不携带 A 的编辑）→ 切回 A（断言编辑内容恢复）。
2. **编辑推送的文件名与内容一致**：B 题全选重写 `int main(void) { return 1; }` → **等「携带预期内容」的那次调用**（不能只等 count>=1 再读最后一次：慢打字下防抖可能先 flush 出内容不完整的中间快照，`toContain` 偶发失败）→ 断言最后一次调用的 `fileName === 'main.cpp'` 且内容含 `return 1;`（打字结束后无新输入，最后一次即最终快照）。

**断言锚点**：`.monaco-editor .view-lines`（Monaco 可见面文本，短代码全量可见）；Tab 条按钮以 displayId 为可访问名（`getByRole('button', { name: 'B', exact: true })`）。

## 直接依赖

- `../fake-tauri`（`callCount` / `loginAndEnterContest` / `window.__e2e.calls`）
- `src/components/problem/ProblemTabStrip.vue`（切题）、`src/components/editor/CodeEditor.vue`（Monaco）

## 被依赖

- 无（叶子 spec）

## 逻辑流程

进场 → 打开题目（load_workspace + get_problem 编排）→ 键盘输入驱动 `workspaceStore.updateCode` → 防抖 flush 为 `update_workspace_file` IPC → 路由切换触发切题前落盘 → 回切时按 pid 从 fake 内存恢复。
