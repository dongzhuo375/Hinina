# index（IPC 统一出口与错误归一化）

> 源文件：`src/bridge/index.ts`

## 职责

全部 Tauri IPC 的唯一出口：`ipcInvoke` 透传参数、把 Rust `AppError` 载荷归一化为 `IpcError`、单点日志、通知组合根注入的错误观察者（全局会话守卫的挂载点）。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `AppErrorVariant` | 联合类型（11 个变体名） | Rust `core::error::AppError` 的变体名（serde 外部标签）；Rust 新增变体时此列表会滞后，未识别变体归为 `null`，仅退化为通用错误处理 |
| `IpcError` | `class extends Error` | 归一化错误：`cmd` / `variant: AppErrorVariant \| null` / `raw`（原始载荷）；getter `isAuthError`（variant === 'Auth'） |
| `parseAppError` | `(raw) => { variant, message }`（私有） | 识别 AppError 外部标签形态 `{ Variant: message }`（单键 + 字符串值）；Error/字符串/`{message}` 各按其形处理；多键对象回退 JSON 文本 |
| `IpcErrorObserver` / `setIpcErrorObserver` | type / `(observer \| null) => void` | 错误观察者注册（仅组合根 `main.ts` 调用一次）；Bridge 只回调，不感知 store/router |
| `ipcInvoke<T>` | `(cmd, args?) => Promise<T>` | invoke 包装：成功透传；失败 → parseAppError → 构造 IpcError → `log.error` 记录（`utils/logger` 作用域日志）→ 通知观察者 → 抛出 |

## 直接依赖

- `@tauri-apps/api/core`（`invoke`）
- `@/utils/logger`（`createLogger` —— 作用域日志）

## 被依赖

- 全部桥接模块：`auth.bridge` / `contest.bridge` / `submission.bridge` / `config.bridge` / `problem.bridge` / `rank.bridge` / `workspace.bridge`
- `main.ts` — 组合根注入观察者（`installSessionGuard`）
- `bridge/__tests__/index.spec.ts`

## 逻辑流程

```
ipcInvoke(cmd, args)
  ├─ invoke 成功 → 原样返回
  └─ reject(raw)
       raw 是 Rust AppError 经 serde 外部标签序列化的对象，形如 { "Auth": "登录失败: …" }
       —— 既不是 Error 也不是字符串！
       → parseAppError：单键+字符串值 → { variant（在白名单内）, message }
       → new IpcError(cmd, message, variant, raw)
       → log.error(`${cmd} 调用失败 (${variant}): ${message}`)   // [scope] 前缀由 utils/logger 统一加；绝不记录 args
       → errorObserver?.(error)      // sessionGuard 据 isAuthError 分流
       → throw error
```

设计要点：

- **为什么必须归一化**：Tauri 以 AppError 序列化对象 reject，若原样透传，上层所有
  `e instanceof Error` 判定恒假、`e.message` 为 undefined——用户只能看到兜底文案，
  真实原因（如「该账号已在其他设备登录」）被吞掉。归一化后上层统一按 Error 处理，
  且 `variant` 保留机器可读的错误类别。
- **日志只记 cmd 与 message，绝不记 args**：`login` 的参数含明文密码（开发手册 5.1
  安全性要求）；有测试直接锁定该约束。
- **观察者模式解耦**：Bridge 是最底层，不能 import store/router（会形成底层反向依赖
  上层）；会话守卫由组合根注入，装配关系集中在 `main.ts`（详见 `stores/sessionGuard.md`）。
- 变体白名单滞后的降级策略：未知变体 `variant=null` + 保留消息，只失去分流能力，
  不丢错误信息。

## 测试

`src/bridge/__tests__/index.spec.ts` 锁定：成功路径透传命令与参数、AppError 载荷 → IpcError（保留变体与真实原因）、非认证错误 `isAuthError=false`（守卫不误触发）、未知变体降级 null、字符串/Error/多键对象三种载荷形态、**安全约束「日志只含命令名与消息，绝不包含调用参数（明文密码）」**、观察者收到同一实例/注销后不回调/未注册不影响抛出。
