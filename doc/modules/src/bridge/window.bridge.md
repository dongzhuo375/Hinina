# window.bridge（无边框窗口控制 IPC 封装）

> 源文件：`src/bridge/window.bridge.ts`

## 职责

把 `@tauri-apps/api/window` 的窗口控制（最小化 / 最大化 / 关闭 / 销毁 / 关闭请求订阅）收敛到 bridge 层。
`bridge/*.bridge.ts` 是唯一接触 `@tauri-apps/api` 的地方，**无例外**——所有层经
`systemService` 消费，服务再转发本模块（评审二十四 #5 补齐文档；2026-11-04 G3 完成 main.ts/App.vue 两处组合根遗留迁移）。

## 核心类型/函数

| 名称 | 签名 | 说明 |
|------|------|------|
| `minimizeWindow` | `() => Promise<void>` | `getCurrentWindow().minimize()` |
| `toggleMaximizeWindow` | `() => Promise<void>` | `toggleMaximize()` |
| `closeWindow` | `() => Promise<void>` | `close()`——经 Tauri 关闭请求，由 `utils/close-guard` 的关窗握手在落盘后退出 |
| `destroyWindow` | `() => Promise<void>` | `destroy()`——close-guard 落盘完成后的收尾，不走 close-requested 往返 |
| `onCloseRequested` | `(handler) => Promise<() => void>` | 关窗握手事件源，返回取消订阅函数（preventDefault 由调用方决定） |

窗口操作失败没有用户可执行的补救动作，调用方一律忽略结果（`void`）。

## 直接依赖

- `@tauri-apps/api/window`（`getCurrentWindow`）

## 被依赖

- `services/system.service.ts`（五个函数的转发：minimize/toggleMaximize/close/destroy/onCloseRequested）
- `src/main.ts`（经 `systemService.onCloseRequested` 订阅关窗握手，`closeWindow`/`destroyWindow` 注入 close-guard）
- `components/layout/WindowControls.vue` 经 Service 消费（View 不直接 import bridge）

## 逻辑流程

`systemService.closeWindow()` → `windowBridge.closeWindow()` →
`getCurrentWindow().close()`。关闭**不等于**立即退出：Tauri 发出关闭请求，前端
`close-guard` 完成落盘握手后才真正关闭。

## 测试

无单测：纯转发，jsdom 无真实窗口实现；`WindowControls` 在组件测试中以 stub 出现，
真实窗口行为属桌面人工验收。
