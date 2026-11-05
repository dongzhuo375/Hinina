# external-links.spec.ts（E2E：外链拦截与转交）

> 源文件：`e2e/specs/external-links.spec.ts`。

## 职责

外链全链路的端到端证明：markdown 渲染（DOMPurify 钩子注入 `target="_blank"` / `rel="noopener noreferrer"`）→ document 级拦截（`utils/external-links`）→ Rust `open_external` 命令（scheme 白名单守卫）。

## 用例

- **点击题面外链转交 `open_external`，且页面不导航**：打开题目 A（fixture 描述含勘误外链）→ 断言渲染层加固属性 → **先挂 `popup` 监听再点击** → `callCount('open_external')` 恰为 1、URL 为原样绝对地址 → 断言 `popup === null`（`preventDefault` 失效时 Chromium 会把 `target="_blank"` 变成新窗口——`h1` 仍在屏断言看不到这个差异，popup 是默认动作是否被拦的可观测信号）→ 负向断言 `h1.statement-title` 仍在屏（WebView 永不导航离开应用）。

## 边界

真实「系统默认浏览器打开」无法自动化——属真机人工验收（`npm run tauri dev` 点题面外链：浏览器打开且应用界面不动）。fake 的 `open_external` case 只记录转交意图（`open_external` 已入 `REQUIRED_ARGS`，`url` 缺参按 Tauri 形态拒绝）。
