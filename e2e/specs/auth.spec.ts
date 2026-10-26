import { expect, test } from '../fixtures'
import { installFakeTauri, loginAndEnterContest } from '../fake-tauri'

/**
 * §6.3 场景 1（收敛）：登录失败 / 登录成功 / 会话恢复。
 * fake 的 login 命令只认 failLogin 注入，凭证本身为虚构值。
 */
test.describe('认证与会话', () => {
  test('登录失败：错误可见且停留在登录页', async ({ page }) => {
    await installFakeTauri(page, {
      failLogin: { variant: 'Auth', message: '用户名或密码错误' },
    })
    await page.goto('/')
    await page.locator('#contest-username').fill('e2e_player')
    await page.locator('#contest-password').fill('wrong-password')
    await page.getByRole('button', { name: '登录进入赛场' }).click()

    // #login-error 精确锚点（getByRole('alert') 会撞上 Monaco 的 aria-alert）
    await expect(page.locator('#login-error')).toContainText('用户名或密码错误')
    await expect(page).toHaveURL(/\/login/)
    // 失败后表单仍可再次提交（不是死锁态）
    await expect(page.getByRole('button', { name: '登录进入赛场' })).toBeEnabled()
  })

  test('登录成功：进入比赛外壳与题目总览', async ({ page }) => {
    await installFakeTauri(page)
    await page.goto('/')
    await page.locator('#contest-username').fill('e2e_player')
    await page.locator('#contest-password').fill('e2e-not-a-real-password')
    await page.getByRole('button', { name: '登录进入赛场' }).click()

    await page.waitForURL('**/contest/problems')
    await expect(page.getByText('题目总览')).toBeVisible()
    // 比赛题目清单已渲染（A/B 两题卡片）
    await expect(page.getByRole('button', { name: '打开题目 A' })).toBeVisible()
    await expect(page.getByRole('button', { name: '打开题目 B' })).toBeVisible()
  })

  test('会话恢复：已持有会话时直达赛场', async ({ page }) => {
    await installFakeTauri(page, {
      user: { id: '1', username: 'e2e_player', token: 'e2e-restored-token' },
    })
    // 深链进入受保护路由：路由守卫经 get_session 恢复会话后放行
    await page.goto('/contest')
    await page.waitForURL('**/contest/problems')

    await expect(page.getByText('题目总览')).toBeVisible()
    // 登录表单不应出现（会话已恢复，无需再走认证）
    await expect(page.locator('#contest-username')).toHaveCount(0)
    // 事件注销的前置全局存在：@tauri-apps/api 2.11 的 _unlisten 首行即调用它，
    // fake 缺失时任何 unlisten 都会抛 TypeError（评审 P2 的回归哨兵）
    await expect(
      page.evaluate(() => typeof window.__TAURI_EVENT_PLUGIN_INTERNALS__?.unregisterListener),
    ).resolves.toBe('function')
  })

  test('登录后刷新恢复会话（fake 后端状态不随 reload 重建）', async ({ page }) => {
    await loginAndEnterContest(page)
    // reload 重跑 addInitScript：若 fake 的后端状态只挂在文档内存里，
    // get_session 会退回 null，守卫把用户弹回登录页——本用例正是这条回归的哨兵
    await page.reload()
    await page.waitForURL('**/contest/problems')
    await expect(page.getByText('题目总览')).toBeVisible()
    await expect(page.locator('#contest-username')).toHaveCount(0)
  })
})
