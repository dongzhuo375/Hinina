import { expect, test } from '@playwright/test'
import { callCount, loginAndEnterContest } from '../fake-tauri'

/**
 * §6.3 场景 6（收敛）：会话失效后的状态清理与登录跳转。
 * 经 failNext 注入一次性 Auth 错误，覆盖 sessionGuard 全链路：
 * 任意命令 Auth 失败 → invalidateSession → logout + 域状态清理 → 跳登录页。
 */
test.describe('会话失效', () => {
  test('Auth 错误触发状态清理并跳回登录页', async ({ page }) => {
    await loginAndEnterContest(page)

    // 下一次 load_workspace 返回认证错误（打开题目必然触发）
    await page.evaluate(() => window.__e2e.failNext('load_workspace', 'Auth', '会话已被服务端吊销'))
    await page.getByRole('button', { name: '打开题目 A' }).click()

    await page.waitForURL('**/login')
    // sessionGuard 全链路的可观测凭证：失效处理会调用一次 logout 清理后端会话
    // （invalidateSession → logout → clearDomainState → replace Login）。
    // 注：失效文案（SESSION_INVALID_MESSAGE）会被登录页挂载时的 checkSession 清空，
    // 实际落到登录页的是干净表单 —— 该现象记录于 doc/cicd/m3-integration.md。
    const calls = await page.evaluate(() => window.__e2e.calls.map((c) => c.cmd))
    expect(calls).toContain('logout')
    // 登录表单回到可输入态（域状态已清理；表单为空时按钮禁用属正常校验行为）
    await expect(page.locator('#contest-username')).toBeVisible()
    await expect(page.locator('#contest-username')).toBeEditable()
    await expect(page.locator('#contest-password')).toBeEditable()
  })

  test('会话失效停止评测轮询（clearDomainState 的可观测凭证）', async ({ page }) => {
    // 评测序列恒为 Running：不手动失效就会一直轮询，停增断言因此有意义
    await loginAndEnterContest(page, { judgementSequence: ['Running'] })
    await page.getByRole('button', { name: '打开题目 A' }).click()
    await expect(page.locator('h1.statement-title')).toHaveText(/^A\./)
    await page.getByRole('button', { name: '提交代码' }).click()
    await expect(page.getByRole('status')).toContainText('已提交 #9001')
    await expect
      .poll(() => callCount(page, 'get_judgement'), { timeout: 10_000 })
      .toBeGreaterThanOrEqual(1)

    // 注入认证错误触发 sessionGuard（clearDomainState 含 submissionStore.stopAllPolling）
    await page.evaluate(() => window.__e2e.failNext('load_workspace', 'Auth', '会话已被服务端吊销'))
    await page.getByRole('button', { name: 'B', exact: true }).click()
    await page.waitForURL('**/login')
    const calls = await page.evaluate(() => window.__e2e.calls.map((c) => c.cmd))
    expect(calls).toContain('logout')

    // 先等在途轮询落地（间隔 1s±20%），再观测 3s：轮询器已停则 get_judgement 停增
    //（若未停，3s 内按 1s 间隔还会增长约 2-3 次）
    await page.waitForTimeout(1_500)
    const before = await callCount(page, 'get_judgement')
    await page.waitForTimeout(3_000)
    expect(await callCount(page, 'get_judgement')).toBe(before)
  })

  test('失效后重新登录可再次进入赛场', async ({ page }) => {
    await loginAndEnterContest(page)
    await page.evaluate(() => window.__e2e.failNext('load_workspace', 'Auth', '会话已被服务端吊销'))
    await page.getByRole('button', { name: '打开题目 A' }).click()
    await page.waitForURL('**/login')

    await page.locator('#contest-username').fill('e2e_player')
    await page.locator('#contest-password').fill('e2e-not-a-real-password')
    await page.getByRole('button', { name: '登录进入赛场' }).click()
    await page.waitForURL('**/contest/problems')
    await expect(page.getByText('题目总览')).toBeVisible()
  })
})
