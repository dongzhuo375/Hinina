# WindowControls（无边框窗口控制按钮）

> 源文件：`src/components/layout/WindowControls.vue`

## 职责

窗口的最小化 / 最大化 / 关闭按钮。窗口 `decorations: false`，这三个按钮是唯一
的 UI 出口（Alt+F4 与任务栏是系统兜底）。TopBar 与 `--settings` 极简壳共用同一
实现——极简壳若不渲染它，维护者在设置页上只能用 Alt+F4 关窗（评审十 #2）。

## 核心类型/函数

`<script setup>`，无 props/emits：

| 名称 | 签名 | 用途 |
|------|------|------|
| `appWindow` | `getCurrentWindow()`（模块顶层） | Tauri 窗口句柄 |
| `minimize` | `() => void` | `appWindow.minimize()` |
| `toggleMaximize` | `() => void` | `appWindow.toggleMaximize()` |
| `closeWindow` | `() => void` | `appWindow.close()` |

模板是三个 `aria-label` 按钮（最小化 / 最大化 / 关闭窗口）内嵌 16×16 SVG。
父容器需保留 `data-tauri-drag-region` 供拖动；按钮自身的点击不会触发拖动。

## 直接依赖

- `@/bridge/window.bridge`（`minimizeWindow` / `toggleMaximizeWindow` / `closeWindow` 薄封装——CLAUDE.md 要求 bridge 是唯一接触 `@tauri-apps/api` 的地方，评审十九 #8）

## 被依赖

- `components/layout/TopBar.vue`（窗口条右侧）
- `views/ContestLayout.vue`（`--settings` 极简壳的拖动区标题栏）

## 逻辑流程

点击按钮 → 调用对应窗口 API → 结果不等待、不回报（窗口操作失败无用户可执行
的补救动作）。关闭经 Tauri 的关闭请求事件，由 `utils/close-guard` 的关窗握手
处理落盘后再退出。

## 测试

无单测：组件只是三个 API 调用，jsdom 无窗口实现；测试中它以 stub 出现
（`ContestLayout.spec` 用 `data-test="wc"` 标记断言极简壳包含窗口控件）。
真实关窗/最小化/最大化属桌面人工验收（RELEASE.md 清单覆盖启动与关窗 smoke）。
