# playwright.config.ts（Playwright 运行器配置）

> 源文件：`playwright.config.ts`（仓库根）

## 职责

E2E 的运行器配置：测试目录、引擎、dev server 拉起方式与失败产物保留策略。本地 `npx playwright test` 与 CI 的 `e2e` job（integration.yml）共用本配置。

## 核心类型/函数

| 配置 | 值 | 用途 |
|------|------|------|
| `testDir` | `./e2e` | 用例与 fake 均在 e2e/ 下；根 tsconfig 只含 `src/**`、vitest include 是显式双 glob，e2e 对二者天然不可见（无需排除配置） |
| `fullyParallel` | `true` | 各用例独立 browser context，fake 状态按页面隔离，可安全并行 |
| `forbidOnly` | `!!process.env.CI` | CI 上提交了 `test.only` 会只跑该用例且退出码为 0（夜间静默漏测），直接拒绝（评审二轮） |
| `timeout` / `expect.timeout` | 60s / 10s | 用例与单断言上限 |
| `reporter` | list + html（`open: never`，输出 `playwright-report/`） | 失败排查用 HTML 报告 |
| `use` | `baseURL http://localhost:1420`；`trace/screenshot/video` 失败保留 | 产物内容仅 fake 数据，无凭证 |
| `projects` | chromium 单引擎（`Desktop Chrome`） | 施工方案 §6.3 首批只承诺 Chromium |
| `webServer` | `npm run dev`，url 1420，`reuseExistingServer: !CI` | vite `server.strictPort=true` 固定 1420，端口冲突直接失败而非漂移 |

## 直接依赖

- `@playwright/test`（devDependency；浏览器二进制装在 `PLAYWRIGHT_BROWSERS_PATH`，本地 `D:\DevTool\pw-browsers`，不落 C 盘）

## 被依赖

- `e2e/specs/*.spec.ts`（运行载体）
- `.github/workflows/integration.yml` 的 `e2e` job（`npx playwright test`；浏览器缓存 key 为 `package-lock.json` 哈希）

## 逻辑流程

`npx playwright test` → webServer 起 vite dev（CI 下不复用既有实例）→ 按 projects 启动 Chromium context → `addInitScript` 注入 fake internals（先于应用脚本）→ 逐用例执行，失败保留 trace/截图/视频。
