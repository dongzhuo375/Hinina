# settings-key.spec.ts（`--settings` 管理方钥匙旅程）

> 源文件：`e2e/specs/settings-key.spec.ts`

## 职责

用真实 Chromium 串起钥匙的**跨层旅程**（守卫直落 → 外壳形态 → 分层锁 → 会话路径）。各环节已被单测分别锁定（launch-redirect / ContestLayout / SettingsView.behavior / sessionGuard），这里补的恰是单测 mock 掉的「环节之间的接缝」。

## 核心用例

| 用例 | 覆盖 |
|------|------|
| 首航直落设置页 | 首屏即「环境设置」（不闪登录表单）；最小壳标题「Hinina 设置」+「返回登录」；维护者层字段（OJ/超时）可编辑、比赛 ID 可编辑；OJ 切换成功后**留在设置页**（不踢回登录页）；返回登录可达登录页，登录页的「返回设置」再回到设置页（评审十五 A 的回程） |
| 普通启动（无钥匙） | 首航停在登录页；无会话直达 `/contest/settings` 仍被送回登录页 |
| 过期会话（评审四 P2-1） | 预置过期会话 + `failConfiguredContest: Auth` → 401 清理会话但**留在设置页**，最小壳接管、维护者层解锁，绝不回到登录页 |
| 已结束赛程（评审六） | `contestWindow` 结束态 + 钥匙 + 会话有效 → 无 `role="alert"`、无 `[inert]`；`fill` 实证可交互（inert 子树必失败）；先等「已连接比赛服务器」再做断言（避免 phase=none 空窗期假绿，评审八 P2）；随后**滚到底部**点「题目」（被闸门拦回）→ 提示条可见、`boundingBox.y` 在视口内（评审十五 B）、且不与 `nav.settings-index` 矩形相交（评审十六横向量测） |
| 草稿跨形态保留（评审七） | `delayConfiguredContestMs` 延迟 401：完整外壳输入地址 → 先断言 StatusBar（完整外壳专属标记，时序错位即红）→ 等待最小壳翻转 → 输入框值仍在（重挂/磁盘覆盖都会红） |

## 依赖

- `e2e/fixtures.ts`（失败附 `ipc-calls.json`）
- `e2e/fake-tauri.ts` 的 `FakeOptions`：`launchFlags` / `failConfiguredContest` / `delayConfiguredContestMs` / `contestWindow`（时间窗偏移）

## 边界

- 窗口的最小化/关闭、真实 WebView2 拖拽区行为属桌面人工验收（jsdom 与 fake 浏览器都不模拟无边框窗口）。
- 真实 OJ 的 401 语义、会话过期时序由后端决定；e2e 用注入的 fake 错误覆盖前端分支。
