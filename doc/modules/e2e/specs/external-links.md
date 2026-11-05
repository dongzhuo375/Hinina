# external-links.spec.ts（E2E：外链拦截与转交）

> 源文件：`e2e/specs/external-links.spec.ts`。

## 职责

外链全链路的端到端证明：markdown 渲染（局部 DOMPurify 实例的钩子给**外链**注入 `target="_blank"` / `rel="noopener noreferrer"`；同 host/mailto/`#` 片段不加）→ document 级拦截（`utils/external-links`，左键含修饰键 + 中键）→ Rust `open_external` 命令（scheme 白名单守卫）。

## 用例

- **点击题面外链转交 `open_external`，且页面不导航**：打开题目 A（fixture 描述含勘误外链）→ 断言渲染层加固属性 → 在 document 上**后注册**一次性监听读应用处理后的 `defaultPrevented`（主锁，确定性）→ 先挂 `popup` 监听再点击 → `callCount('open_external')` 恰为 1、URL 为原样绝对地址 → 断言 `defaultPrevented === true` 且 `popup === null`（`h1` 仍在屏断言看不到新窗口；`preventDefault` 失效时 popup 约 0.9s 后出现）→ 负向断言 `h1.statement-title` 仍在屏（WebView 永不导航离开应用）。
- **Ctrl+左键与中键同样转交**：修饰键与中键在 WebView 里没有「新标签」语义，默认策略 = 新窗口被拒绝（无动作）。两种点击各触发一次 `open_external`（计数 1 → 2），题面仍在屏。

## 边界

真实「系统默认浏览器打开」无法自动化——属真机人工验收（`npm run tauri dev` 点题面外链：浏览器打开且应用界面不动）。fake 的 `open_external` case 只记录转交意图（`open_external` 已入 `REQUIRED_ARGS`，`url` 缺参按 Tauri 形态拒绝）。
