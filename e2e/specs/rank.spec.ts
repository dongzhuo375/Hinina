import { expect, test } from '../fixtures'
import { callCount, loginAndEnterContest } from '../fake-tauri'

/**
 * §6.3 补缺（2026-11 审计 C4）：榜单页 —— 全应用最复杂的客户端状态机。
 * 覆盖：加载与实时刷新形态、封榜横幅、分页翻页与失败回滚、打星筛选触发的
 * 全量快照跨页聚合（fake 按 currentPage/limit 分页，与真实 IPage 形态一致）。
 */
test.describe('实时榜单', () => {
  test('加载表格、实时刷新状态与参与人数', async ({ page }) => {
    // 55 队：分页底栏（含「共 N 队」）只在 pages > 1 时渲染
    await loginAndEnterContest(page, { rankTeams: 55 })
    await page.getByRole('link', { name: /榜单/ }).click()
    await page.waitForURL('**/contest/rank')

    // 形态断言前先等业务加载标记（表格行），避免轮询空窗期假绿。
    // uid 1 = 当前登录用户：其记录被合并为置顶「我」行，页面上不存在 team_01
    await expect(page.getByText('team_02')).toBeVisible()
    await expect(page.getByRole('heading', { name: /实时榜单/ })).toBeVisible()
    await expect(page.getByText('实时刷新中')).toBeVisible()
    await expect(page.getByText(/共 \d+ 队/)).toBeVisible()
    expect(await callCount(page, 'get_contest_rank')).toBeGreaterThanOrEqual(1)
  })

  test('封榜：横幅说明与榜单照常展示', async ({ page }) => {
    await loginAndEnterContest(page, { sealRank: true })
    await page.getByRole('link', { name: /榜单/ }).click()
    await page.waitForURL('**/contest/rank')

    await expect(page.getByText('榜单已封榜，仅显示尝试次数')).toBeVisible()
    await expect(page.getByText('封榜时段内的提交结果将在赛后统一揭晓')).toBeVisible()
    // 封榜不是禁用页面：表格照常渲染，轮询继续
    await expect(page.getByText('e2e_player').first()).toBeVisible()
  })

  test('翻页与失败回滚：页码同步退回、旧数据保留、错误条可见', async ({ page }) => {
    // 105 队 → 50/页 共 3 页。回滚用例刻意翻到一个「从未加载过的页」发起失败
    // 请求：用户操作路径的加载有 3 秒查询 memo（连点保护），刚加载过的页会
    // 命中 memo 直接返回而不发请求，失败注入根本不会被消费
    await loginAndEnterContest(page, { rankTeams: 105 })
    await page.getByRole('link', { name: /榜单/ }).click()
    await page.waitForURL('**/contest/rank')
    await expect(page.getByText('team_02')).toBeVisible()

    const pager = page.getByRole('navigation', { name: '分页' })
    await pager.getByRole('button', { name: '2', exact: true }).click()
    await expect(page.getByText('team_51').first()).toBeVisible()
    await pager.getByRole('button', { name: '3', exact: true }).click()
    await expect(page.getByText('team_101').first()).toBeVisible()
    await expect(pager.locator('[aria-current="page"]')).toHaveText('3')

    // 让第 2 页的查询 memo 过期后注入失败并折返第 2 页：请求真发、真失败
    await page.waitForTimeout(3_200)
    // 控制面方法必须在页面内调用（evaluate 返回值经结构化克隆，函数会丢）
    await page.evaluate(() =>
      window.__e2e.failNext('get_contest_rank', 'Network', 'fake: 榜单服务暂不可用'),
    )
    await pager.getByRole('button', { name: '2', exact: true }).click()

    await expect(page.getByText(/榜单刷新失败/)).toBeVisible()
    // 页码回滚（「旧页的行 + 新页的页码」是历史缺陷形态）
    await expect(pager.locator('[aria-current="page"]')).toHaveText('3')
    await expect(page.getByText('team_101').first()).toBeVisible()
  })

  test('打星筛选触发全量快照：跨页聚合与手动刷新', async ({ page }) => {
    await loginAndEnterContest(page, { rankTeams: 55 })
    await page.getByRole('link', { name: /榜单/ }).click()
    await page.waitForURL('**/contest/rank')
    await expect(page.getByText('team_02')).toBeVisible()

    // star 筛选 → fetchAllRows 从第 1 页循环到末页（2 页），聚合 55 行
    await page.getByRole('button', { name: '打星队 (Star)' }).click()
    await expect(page.getByText(/客户端筛选 · 已加载 55 行/)).toBeVisible({ timeout: 15_000 })
    // 快照跨页：至少两次 get_contest_rank（页间有 400ms 节流）
    expect(await callCount(page, 'get_contest_rank')).toBeGreaterThanOrEqual(2)
    // 打星行（第 13 队 rank=-1）经筛选可见；手动刷新入口在状态行
    await expect(page.getByText('team_13').first()).toBeVisible()
    await expect(page.getByRole('button', { name: '手动刷新' })).toBeVisible()
  })
})
