# window.bridge（无边框窗口控制 IPC 封装）

> 源文件：`src/bridge/window.bridge.ts`

## 职责

把 `@tauri-apps/api/window` 的窗口控制（最小化 / 最大化 / 关闭）收敛到 bridge 层。
CLAUDE.md：`bridge/*.bridge.ts` 是唯一接触 `@tauri-apps/api` 的地方——组件经
`systemService` 消费，服务再转发本模块（评审二十四 #5 补齐文档）。

## 核心类型/函数

| 名称 | 签名 | 说明 |
|------|------|------|
| `minimizeWindow` | `() => Promise<void>` | `getCurrentWindow().minimize()` |
| `toggleMaximizeWindow` | `() => Promise<void>` | `toggleMaximize()` |
| `closeWindow` | `() => Promise<void>` | `close()`——经 Tauri 关闭请求，由 `utils/close-guard` 的关窗握手在落盘后退出 |

窗口操作失败没有用户可执行的补救动作，调用方一律忽略结果（`void`）。

## 直接依赖

- `@tauri-apps/api/window`（`getCurrentWindow`）

## 被依赖

- `services/system.service.ts`（`minimizeWindow` / `toggleMaximizeWindow` / `closeWindow` 转发）
- `components/layout/WindowControls.vue` 经 Service 消费（View 不直接 import bridge）

## 逻辑流程

`systemService.closeWindow()` → `windowBridge.closeWindow()` →
`getCurrentWindow().close()`。关闭**不等于**立即退出：Tauri 发出关闭请求，前端
`close-guard` 完成落盘握手后才真正关闭。

## 测试

无单测：纯转发，jsdom 无真实窗口实现；`WindowControls` 在组件测试中以 stub 出现，
真实窗口行为属桌面人工验收。
