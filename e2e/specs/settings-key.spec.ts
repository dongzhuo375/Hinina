import { expect, test } from '../fixtures'
import { installFakeTauri } from '../fake-tauri'

// --settings 管理方钥匙的**跨层旅程**：守卫直落 → 最小壳 → 维护者层解锁 →
// 返回登录。各环节已被单测分别锁定（launch-redirect / ContestLayout /
// SettingsView.behavior），这里用真实浏览器把它们串起来——单测 mock 掉的
// 恰恰是「环节之间」的接缝。
test.describe('--settings 管理方钥匙旅程', () => {
  test('首航直落设置页：最小壳 + 维护者层解锁 + 返回登录回登录页', async ({ page }) => {
    await installFakeTauri(page, { launchFlags: { settings: true } })
    await page.goto('/')

    // 直落：首屏就是设置页（不闪登录表单）
    await expect(page.getByText('环境设置', { exact: true })).toBeVisible()
    // 最小壳：拖动区标题 + 返回登录；无会话形态不出状态条连接信息
    await expect(page.getByText('Hinina 设置')).toBeVisible()
    const back = page.getByRole('button', { name: '返回登录' })
    await expect(back).toBeVisible()
    // 维护者层解锁：连接字段可编辑；选手层字段照常
    // （label 内含提示文本，可达名互相沾染 —— 一律用 role + 名字正则定位）
    await expect(page.getByRole('combobox', { name: /当前 OJ/ })).toBeEnabled()
    await expect(page.getByRole('textbox', { name: /比赛 ID \/ 引用/ })).toBeEnabled()
    await expect(page.getByRole('textbox', { name: /请求超时/ })).toBeEnabled()

    // 旅程尾：返回登录 → 正常登录页
    await back.click()
    await expect(page.getByText('登录参赛系统')).toBeVisible()
  })

  test('普通启动（无钥匙）：首航停在登录页，永不进最小壳', async ({ page }) => {
    await installFakeTauri(page)
    await page.goto('/')
    await expect(page.getByText('登录参赛系统')).toBeVisible()
    // 构造无会话直达设置路由（守卫条件豁免仅钥匙模式）：仍被送回登录页
    await page.goto('/contest/settings')
    await expect(page.getByText('登录参赛系统')).toBeVisible()
  })

  test('评审四 P2-1：钥匙 + 存量过期会话——首个认证请求 401 后留在设置页', async ({ page }) => {
    await installFakeTauri(page, {
      // 预置（已过期的）会话：get_session 本地恢复 → 先以正常外壳落设置页
      user: { id: 'u1', username: 'maintainer', token: 'stale-token' },
      launchFlags: { settings: true },
      failConfiguredContest: { variant: 'Auth', message: 'fake: 会话已过期' },
    })
    await page.goto('/')

    // bootstrap 的认证请求 401 → sessionGuard 清理会话但不跳转（钥匙旁路）
    // → 最小壳接管，维护者层随会话失效而解锁；绝不被打回登录页
    await expect(page.getByText('环境设置', { exact: true })).toBeVisible()
    await expect(page.getByText('Hinina 设置')).toBeVisible()
    const oj = page.getByRole('combobox', { name: /当前 OJ/ })
    await expect(oj).toBeEnabled()
    await expect(page.getByText('登录参赛系统')).toBeHidden()
  })

  test('评审六：钥匙 + 会话有效 + 已结束赛程——设置页无遮罩无 inert 且可交互', async ({ page }) => {
    await installFakeTauri(page, {
      user: { id: 'u1', username: 'maintainer', token: 't' },
      launchFlags: { settings: true },
      contestWindow: { startOffsetSecs: -7_200, endOffsetSecs: -3_600 },
    })
    await page.goto('/')

    // 有会话：完整外壳（无最小壳标题），但赛程已结束——闸门的遮罩与 inert
    // 不得锁死钥匙设置页。先等赛程数据确已加载（StatusBar 连接态翻转），
    // 再做遮罩/inert 断言——避免在 phase=none 的空窗期假绿（评审八 P2-2）
    await expect(page.getByText('环境设置', { exact: true })).toBeVisible()
    await expect(page.getByText('已连接比赛服务器')).toBeVisible()
    await expect(page.locator('[role="alert"]')).toHaveCount(0)
    await expect(page.locator('[inert]')).toHaveCount(0)
    // 可交互实证：fill 走 Playwright 的 hit-target 检查，inert 子树里必然失败
    const addr = page.getByRole('textbox', { name: /服务器地址/ })
    await addr.fill('https://repaired.example')
    await expect(addr).toHaveValue('https://repaired.example')
  })

  test('评审七：钥匙 + 会话失效翻转外壳——未保存草稿跨形态保留', async ({ page }) => {
    await installFakeTauri(page, {
      user: { id: 'u1', username: 'maintainer', token: 'stale-token' },
      launchFlags: { settings: true },
      failConfiguredContest: { variant: 'Auth', message: 'fake: 会话已过期' },
      // 失败延迟落地：先在完整外壳里输入草稿，再等会话失效翻转最小壳
      delayConfiguredContestMs: 2_000,
    })
    await page.goto('/')
    await expect(page.getByText('环境设置', { exact: true })).toBeVisible()

    // 翻转前输入草稿（此时会话仍在、完整外壳）
    const addr = page.getByRole('textbox', { name: /服务器地址/ })
    await addr.fill('https://draft-kept.example')
    await expect(addr).toHaveValue('https://draft-kept.example')

    // bootstrap 的延迟 401 落地 → 会话失效 → 最小壳翻转（chrome 换、视图不换）
    await expect(page.getByText('Hinina 设置')).toBeVisible()

    // 草稿仍在输入框：若视图被重挂或表单被磁盘值覆盖，此处即红
    await expect(addr).toHaveValue('https://draft-kept.example')
    await expect(page.getByText('登录参赛系统')).toBeHidden()
  })
})
