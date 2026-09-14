# submission（评测终态判据）

> 源文件：`src/utils/submission.ts`

## 职责

提供"评测是否结束"的单一判据，决定轮询何时停止。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `isTerminalStatus` | `(status: JudgementStatus) => boolean` | 状态是否为终态（终态即停止轮询） |
| `NON_TERMINAL_STATUSES` | `ReadonlySet<JudgementStatus>`（模块内私有） | 非终态集合：`Pending` / `Compiling` / `Running` |

## 直接依赖

- `@/types/submission`（仅类型）

## 被依赖

- `stores/submissionStore.ts` — `pollOnce()` 判定是否停止轮询

## 逻辑流程

```
status ∈ {Pending, Compiling, Running} → 非终态，继续轮询
其余（含 Unknown）                      → 终态，停止轮询
```

设计要点：

- 判据与 Rust `adapter::hoj::types::is_terminal_status`（仅 HOJ 状态码 0/1 为非终态）严格对齐。
- `Unknown` 必须视为终态：HOJ 的 OLE/SE/RJE/FREQ/UE 因无对应枚举被映射为 `Unknown`，
  若当作非终态将导致这些提交被无限轮询（此前 View 内硬编码状态列表即存在该缺陷）。
- 纯函数、无副作用，便于单元测试与跨层复用。

## 测试

`src/utils/__tests__/submission.spec.ts` 锁定两条回归契约：`Unknown` 必须视为终态（防止 OLE/SE/RJE 等被无限轮询）、终态清单与 `JudgementStatus` 全量取值一一对应（借 `Record<JudgementStatus, boolean>` 穷尽映射，新增枚举值时直接类型报错，逼迫显式归类）。
