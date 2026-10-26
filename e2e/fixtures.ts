import { test as base } from '@playwright/test'
import { writeFile } from 'node:fs/promises'

/**
 * 共享 test 入口：失败时把 fake 的 IPC 调用时序附进 test-results。
 *
 * 施工方案 §6.3 要求「失败保留 trace、截图、控制台和 IPC 日志」——trace/截图/
 * 视频由 playwright.config 负责；`window.__e2e.calls` 此前只留在页面内存里，
 * 随用例结束销毁，失败 artifact 中没有完整 IPC 时序（评审三轮低优先）。
 * 附件随 test-results 与 HTML 报告一并上传（内容全部虚构，无凭证）。
 */
export const test = base.extend({
  page: async ({ page }, use, testInfo) => {
    await use(page)
    if (testInfo.status !== testInfo.expectedStatus) {
      try {
        const calls = await page.evaluate(() => window.__e2e?.calls ?? [])
        const file = testInfo.outputPath('ipc-calls.json')
        await writeFile(file, JSON.stringify(calls, null, 2), 'utf-8')
        await testInfo.attach('ipc-calls.json', { path: file, contentType: 'application/json' })
      } catch {
        // 页面已关闭/无 fake 时放弃附件，不掩盖原始失败
      }
    }
  },
})

export { expect } from '@playwright/test'
