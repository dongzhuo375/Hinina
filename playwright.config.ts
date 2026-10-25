import { defineConfig, devices } from "@playwright/test";

/// E2E（M3，doc/cicd/m3-integration.md）：真实浏览器跑真实前端，IPC 由
/// e2e/fake-tauri.ts 注入的 window.__TAURI_INTERNALS__ fake 承接——Tauri v2 的
/// 命令与事件都经此单点，因此零生产代码改动、零构建开关，fake 只存在于
/// Playwright 会话。失败保留 trace/截图/视频（内容仅 fake 数据，无凭证）。
export default defineConfig({
  testDir: "./e2e",
  // 各用例独立 browser context，fake 状态按页面隔离，可安全并行
  fullyParallel: true,
  timeout: 60_000,
  expect: { timeout: 10_000 },
  reporter: [
    ["list"],
    ["html", { outputFolder: "playwright-report", open: "never" }],
  ],
  use: {
    baseURL: "http://localhost:1420",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
  webServer: {
    // vite.config server.strictPort=true 固定 1420，端口冲突会直接失败而非漂移
    command: "npm run dev",
    url: "http://localhost:1420",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
