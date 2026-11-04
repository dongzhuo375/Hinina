import { expect, test } from '../fixtures'
import { callCount, loginAndEnterContest } from '../fake-tauri'

/**
 * 外链全链路：markdown 渲染（DOMPurify 钩子注入 target/rel）→ document 级拦截
 * （utils/external-links）→ Rust `open_external`（scheme 白名单守卫）。
 *
 * fake 只记录转交意图——真实「系统默认浏览器打开」无法自动化，属真机人工验收
 * （`npm run tauri dev` 点题面外链：浏览器打开且应用界面不动）。
 */
test.describe('外链拦截与转交', () => {
  test('点击题面外链转交 open_external，且页面不导航', async ({ page }) => {
    await loginAndEnterContest(page)
    await page.getByRole('button', { name: '打开题目 A' }).click()
    await expect(page.locator('h1.statement-title')).toHaveText(/^A\./)

    // 渲染层加固断言：DOMPurify 钩子注入的属性（fail-safe 半边）
    const link = page.getByRole('link', { name: '勘误说明' })
    await expect(link).toHaveAttribute('target', '_blank')
    await expect(link).toHaveAttribute('rel', /noopener/)

    // 点击 → 拦截 → open_external（一次，URL 为原样绝对地址）
    await link.click()
    await expect
      .poll(() => callCount(page, 'open_external'), { timeout: 5_000 })
      .toBe(1)
    const args = await page.evaluate(() => {
      const calls = window.__e2e?.calls ?? []
      const last = calls.filter((c) => c.cmd === 'open_external').at(-1)
      return last?.args ?? null
    })
    expect(args).toEqual({ url: 'https://errata.example.com/a1' })

    // WebView 永不导航离开应用：点击后题面仍在屏（负向断言）
    await expect(page.locator('h1.statement-title')).toHaveText(/^A\./)
  })
})
