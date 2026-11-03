import { expect, test } from '../fixtures'
import { callCount, loginAndEnterContest } from '../fake-tauri'

/**
 * §6.3 补缺（2026-11 审计 C4）：公告页 —— 列表渲染、进入即标已读、
 * 新公告事件驱动的红点/未读计数（事件只是刷新触发，不是状态来源）。
 */
test.describe('赛事公告', () => {
  test('列表渲染，进入即查询并写入已读，无未读标记', async ({ page }) => {
    await loginAndEnterContest(page)
    await page.getByRole('link', { name: /公告/ }).click()
    await page.waitForURL('**/contest/announcements')

    // 形态断言前等业务加载标记（公告标题）
    await expect(page.getByText('比赛须知')).toBeVisible()
    await expect(page.getByRole('heading', { name: /赛事公告/ })).toBeVisible()
    await expect(page.getByText('共 1 条公告')).toBeVisible()
    await expect(page.getByText('请勿交流题目解法。')).toBeVisible()

    // 进入即把当前列表全部标记已读（store.load 编排），ActivityBar 红点不亮
    await expect.poll(async () => callCount(page, 'get_read_announcement_ids')).toBeGreaterThanOrEqual(1)
    await expect
      .poll(async () => callCount(page, 'mark_announcements_read'))
      .toBeGreaterThanOrEqual(1)
    await expect(page.locator('[aria-label="有未读公告"]')).toHaveCount(0)
  })

  test('新公告事件 → 列表刷新 → 红点亮起，进入公告页后清除', async ({ page }) => {
    await loginAndEnterContest(page)
    // 先进入公告页把存量公告标为已读（否则红点是存量未读，事件旅程无从观测）
    await page.getByRole('link', { name: /公告/ }).click()
    await page.waitForURL('**/contest/announcements')
    await expect(page.getByText('比赛须知')).toBeVisible()
    await expect
      .poll(async () => callCount(page, 'mark_announcements_read'))
      .toBeGreaterThanOrEqual(1)

    // 回到题目页：离开即「不再看着」，新公告必须保持未读
    await page.getByRole('link', { name: /题目/ }).click()
    await page.waitForURL('**/contest/problems')
    // 控制面方法必须在页面内调用（evaluate 返回值经结构化克隆，函数会丢）
    const before = await callCount(page, 'list_contest_announcements')
    await page.evaluate(() => {
      window.__e2e.pushAnnouncement()
      window.__e2e.emitEvent('announcements-published', { contestId: '1001', newIds: ['702'] })
    })

    // 事件到达即刷新列表（不等 60s 轮询），红点随即点亮——且刷新发生在**进入
    // 公告页之前**（增量断言：事件触发的 refresh 已多打了至少一次 list）
    await expect(page.locator('[aria-label="有未读公告"]')).toBeVisible({ timeout: 10_000 })
    await expect
      .poll(async () => callCount(page, 'list_contest_announcements'))
      .toBeGreaterThan(before)
    const after = await callCount(page, 'list_contest_announcements')

    // 进入公告页：新公告在列表中，且「进入即标已读」只补 702（701 上次已读）
    await page.getByRole('link', { name: /公告/ }).click()
    await page.waitForURL('**/contest/announcements')
    await expect(page.getByText(/共 2 条公告/)).toBeVisible()
    await expect(page.getByText('补充说明 2')).toBeVisible()
    // mark 的载荷含新公告 id（fake 已读集合有状态：未读集 = 列表 − 已读）
    await expect
      .poll(() =>
        page.evaluate(
          () =>
            window.__e2e.calls.filter(
              (c) =>
                c.cmd === 'mark_announcements_read' &&
                (c.args.ids as string[]).includes('702'),
            ).length,
        ),
      )
      .toBeGreaterThanOrEqual(1)
    expect(await callCount(page, 'list_contest_announcements')).toBeGreaterThan(after)
    // 读完即清
    await expect(page.locator('[aria-label="有未读公告"]')).toHaveCount(0)
  })

  test('公告与已读状态跨 reload 持久化（后端状态保真）', async ({ page }) => {
    // fake 的后端状态（sessionStorage）不随 WebView reload 重建，与真实 Tauri
    // 的 Rust 进程一致：追加的公告仍在、已读集不回退（评审低 6 的专项回归）
    await loginAndEnterContest(page)
    await page.getByRole('link', { name: /公告/ }).click()
    await page.waitForURL('**/contest/announcements')
    await expect(page.getByText('比赛须知')).toBeVisible()
    await expect
      .poll(async () => callCount(page, 'mark_announcements_read'))
      .toBeGreaterThanOrEqual(1)

    await page.getByRole('link', { name: /题目/ }).click()
    await page.waitForURL('**/contest/problems')
    await page.evaluate(() => {
      window.__e2e.pushAnnouncement()
      window.__e2e.emitEvent('announcements-published', { contestId: '1001', newIds: ['702'] })
    })
    await expect(page.locator('[aria-label="有未读公告"]')).toBeVisible({ timeout: 10_000 })

    await page.reload()
    await expect(page.getByRole('link', { name: /题目/ })).toBeVisible()

    // 追加的公告跨 reload 存活：进入公告页列表是 2 条
    await page.getByRole('link', { name: /公告/ }).click()
    await page.waitForURL('**/contest/announcements')
    await expect(page.getByText(/共 2 条公告/)).toBeVisible()
    await expect(page.getByText('补充说明 2')).toBeVisible()

    // 已读集跨 reload 存活：reload 后（calls 为文档级、已清零）进入页的 mark
    // 载荷**只含新公告 702** —— 701 已在上个会话读过，不会被重复标已读
    await expect
      .poll(() =>
        page.evaluate(() =>
          window.__e2e.calls
            .filter((c) => c.cmd === 'mark_announcements_read')
            .map((c) => (c.args.ids as string[])),
        ),
      )
      .toContainEqual(['702'])
  })
})
