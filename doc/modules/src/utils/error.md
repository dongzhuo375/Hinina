# error（错误文案收敛）

> 源文件：`src/utils/error.ts`

## 职责

把任意错误载荷收敛为**可直接展示**的文案：有信息用信息，没有则回退调用方给的兜底文案。是「错误 → 用户可见文案」的唯一出口，避免各层重复写 `e instanceof Error ? e.message : '…'`。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `errorMessage` | `(e: unknown, fallback: string) => string` | `Error`（含 `IpcError`）取 `message`；字符串载荷 trim 后返回；空 message / 非 Error 对象 / null / undefined 一律回退 `fallback` |

## 直接依赖

无（纯函数，不 import 任何模块 —— 刻意不依赖 `@/bridge`，避免工具层反向依赖 IPC 层）。

## 被依赖

- stores：`authStore` / `announcementStore` / `problemStore` / `contestStore` / `rankStore` / `submissionStore`（错误态 `error` 文案）
- views：`ProblemSolveView`（编排错误）、`SettingsView`（读取/保存配置）、`SubmissionDetailView`（详情加载）

## 逻辑流程

```
errorMessage(e, '加载题目失败')
  e instanceof Error  → e.message.trim() 非空则返回，空则回退
  typeof e === 'string' → e.trim() 非空则返回，空则回退
  其它（对象/null/undefined） → 回退 '加载题目失败'
```

设计要点：

- **与 `bridge/ipcInvoke` 分工明确**：bridge 负责把 Rust `AppError`（`{ Variant: msg }`）
  归一为 `IpcError extends Error` 并保留变体；本模块只负责「拿什么给用户看」，
  不解析变体、不做分流（分流判据在 `IpcError.isAuthError`）。
- **空 message 也要兜底**：`new Error('')` 真实存在（如某些 WebView 异常），
  原样展示会得到一片空白，用户看不出发生了什么。
- **非 Error 载荷不回显原始内容**：`{ code: 500 }` 这类对象直接 `String()` 只会得到
  `[object Object]`，回退兜底文案更诚实。

## 测试

`src/utils/__tests__/error.spec.ts`：Error/IpcError 取 message、字符串 trim、空 message / 非 Error 对象 / null / undefined 回退兜底文案。
