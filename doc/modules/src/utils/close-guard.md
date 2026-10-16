# close-guard（关窗守卫）

> 源文件：`src/utils/close-guard.ts`

## 职责

关窗前的落盘握手：拦截 `close-requested`，把在途编辑落盘后再关闭窗口，并**保证任何失败路径下窗口最终一定能关上**。状态机与双层时间上界收敛在此，`main.ts` 只做装配（注入真实窗口操作）。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `DEFAULT_FLUSH_TIMEOUT_MS` | `3000` | 单轮落盘等待上限：超时即放行关闭 |
| `DEFAULT_HARD_TIMEOUT_MS` | `5000` | 硬超时：从拦截那一刻起算的绝对上界，覆盖整个多轮循环 |
| `DEFAULT_MAX_FLUSH_ROUNDS` | `3` | 最大落盘轮数：复查恒真的病态场景下兜底（硬超时仍是绝对上界） |
| `CloseGuardState` | `'idle' \| 'flushing' \| 'allowing'` | 守卫状态 |
| `CloseGuardOptions` | interface | `flush` / `close` / `destroy` / `flushTimeoutMs?` / `hardTimeoutMs?` / `shouldReflush?` / `maxFlushRounds?` / `onWarn?` / `setTimeoutFn?` / `clearTimeoutFn?` |
| `CloseGuard` | interface | `handleRequest(): boolean` / `state(): CloseGuardState` |
| `createCloseGuard` | `(options: CloseGuardOptions) => CloseGuard` | 创建守卫 |

`shouldReflush`（PR21-7）：一轮落盘完成后复查，返回 `true` 再做一轮 —— 把「关窗握手窗口内到达的改动」也纳入落盘；不注入则保持单轮（既有调用方契约不变）。`main.ts` 注入 `syncPending || isDirty`：前者 = 防抖窗口内的新键，后者 = 推送成功但 `save_workspace` 失败的轮（给瞬时故障一次有界重试）。

## 直接依赖

无（纯模块，窗口操作与定时器全部由参数注入）

## 被依赖

- `src/main.ts` — `installCloseFlushGuard()` 注入 `getCurrentWindow()` 的 `close` / `destroy`
- `src/utils/__tests__/close-guard.spec.ts` — 单元测试

## 逻辑流程

```
idle ──handleRequest()──▶ flushing ──落盘（有界多轮）/ flushTimeout 到点──▶ allowing ──▶ destroy()
  ▲                          │
  └──── 直接放行（false）─────┘   ← allowing 状态下再来的请求
```

```
handleRequest():
  allowing → false                 // 放行，调用方不 preventDefault
  flushing → true                  // 已拦截，在途流程负责最终关闭（不重复落盘）
  idle     → flushing; void runFlush(); true

runFlush():
  起 hardTimeout 定时器（到点若仍在 flushing → finalize；覆盖整个多轮循环）
  for round in 1..maxFlushRounds:
    outcome = race([flush().catch(吞掉), delay(flushTimeout)])
    状态已离开 flushing（硬超时已收尾）→ break
    outcome == timeout → break      // 超时轮不重试：后端无响应时重试只会推迟关窗
    !shouldReflush?.() → break      // 静默（无新改动）或未注入复查 → 单轮契约
  清 hardTimeout
  await finalize()

finalize():                        // 幂等
  已是 allowing → return
  allowing = true
  await destroy()                  // 落盘已完成，不需要再走一遍 close-requested 往返
  失败 → onWarn + await close() 兜底
```

## 设计要点

- **失效模式 1（原实现「窗口无法关闭」的直接原因）**：原实现内联在 `main.ts`，用 `flushing` 布尔量表示「落盘在途」，收尾调用 `getCurrentWindow().close()`。若该调用抛错而异常被 `void` 吞掉，`flushing` 就永久停在 `true` —— 此后**每一次**点关闭都命中 `if (flushing) return`，且已在 `preventDefault()` 之后，窗口再也关不掉。本实现用状态机取代布尔量：任何路径都不得停在 `flushing`。
- **失效模式 2（收尾依赖二次 close-requested）**：在 `close-requested` 回调里再调 `close()` 会重新触发该事件，靠标志放行；窗口此时已处于 closing 状态，事件可能根本不投递，关闭请求被静默丢弃。故收尾用 `destroy()`（绕过 `close-requested`），`destroy` 不可用（权限缺失 / 平台差异）时才退回 `close()` —— 此时状态已是 `allowing`，二次事件会被放行。
- **双层时间上界**：`flushTimeoutMs` 保证单轮落盘不无限等待；`hardTimeoutMs` 是从拦截起算的绝对上界，覆盖整个多轮循环，兜住「落盘 Promise 永不 settle」与「收尾本身卡住」两种极端。`finalize` 幂等，两条路径同时到达不会重复销毁。
- **有界多轮重刷（PR21-7，2026-10-16 闭合）**：单轮「捕获内容 → 写盘」期间到达的落键不在本轮落盘内，而 destroy 后应用即退、不存在「下一次落盘」—— 每轮完成后经 `shouldReflush` 复查，有在途改动（或上一轮落盘失败留下脏标记）则再来一轮，静默或轮数用尽才收尾。超时轮不重试（后端无响应时重试只会推迟关窗）；硬超时已收尾后，在途轮完成也不得再起下一轮（状态检查）。残余窗口只剩最后一轮「复查通过 → destroy」之间的毫秒级在途段 —— 有限协议无法归零，归零需握手期间锁输入（复用 PR21-8 机制），属后续可选项。
- **重复请求仍拦截**：`flushing` 期间点第二次关闭返回 `true`（拦截）但不重复落盘 —— 不耐烦的双击不该中断在途落盘（等于零超时丢数据）。
- **依赖全注入**：不需要真实窗口即可穷尽测试「落盘成功 / 落盘抛错 / 落盘永不 settle / destroy 失败 / 两者都失败 / 重复点击 / 多轮重刷」等路径，这些正是原实现出问题的地方。
- **落盘失败不阻断退出**：卡住窗口比丢一次自动备份更糟（内容仍留在编辑器与后端内存；失败轮经复查还有有界重试，重试仍失败则照常关窗 —— 硬超时保证一定关得上）。

## 测试

`src/utils/__tests__/close-guard.spec.ts` 锁定：落盘完成后销毁窗口、`allowing` 后放行、在途重复请求不重复落盘、落盘抛错仍放行、落盘永不 settle 由 `flushTimeoutMs` 兜底、`destroy` 失败退回 `close`、两者都失败后状态仍是 `allowing`（不再永久卡死）、硬超时是绝对上界、正常路径不重复销毁、`flushTimeoutMs` 控制放行时刻；有界重刷六契约 —— 一轮后有在途改动则再来一轮且静默后才销毁、未注入 `shouldReflush` 保持单轮、复查恒真时最多 `maxFlushRounds` 轮、失败轮经复查获得有界重试、超时轮不重刷、多轮期间硬超时仍是绝对上界且收尾后不得再起下一轮。

> 测试辅助函数 `makeGuard` 会把覆写的依赖**包成 spy 后再注入** —— 直接把 overrides 展开进 `createCloseGuard` 会让断言的是未被使用的默认 spy（测试永远看到 0 次调用），这是本轮踩过的坑。
