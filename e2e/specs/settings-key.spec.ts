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
})
