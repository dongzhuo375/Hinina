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

    // 默认动作必须被拦下（确定性信号）：在 document 上**后注册**一次性监听 ——
    // 应用监听器在启动时先注册，同一节点按注册顺序执行，此处读到的是应用处理
    // 之后的 defaultPrevented。preventDefault 失效时它是 false（变异验证锁定）。
    await page.evaluate(() => {
      const probe = window as unknown as { __e2eLinkPrevented?: boolean }
      probe.__e2eLinkPrevented = false
      document.addEventListener(
        'click',
        (event) => {
          const target = event.target instanceof Element ? event.target : null
          if (target?.closest('a[href^="https://errata.example.com"]')) {
            probe.__e2eLinkPrevented = event.defaultPrevented
          }
        },
        { once: true },
      )
    })

    // 默认动作的下游表现：没有新窗口。窗口要等外部导航发起后才可见（本环境约
    // 0.9s，500ms 会假绿），故给 2s 窗口；主锁是上面的 defaultPrevented。
    const popup = page.waitForEvent('popup', { timeout: 2_000 }).catch(() => null)
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
    // 主锁（确定性）：应用处理后的 defaultPrevented
    expect(
      await page.evaluate(
        () => (window as unknown as { __e2eLinkPrevented?: boolean }).__e2eLinkPrevented,
      ),
    ).toBe(true)
    // 下游佐证：没有新窗口
    expect(await popup).toBeNull()

    // WebView 永不导航离开应用：点击后题面仍在屏（负向断言）
    await expect(page.locator('h1.statement-title')).toHaveText(/^A\./)
  })
})
