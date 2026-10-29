import { expect, test } from '../fixtures'
import { callCount, loginAndEnterContest } from '../fake-tauri'

/** 设置页真实渲染、响应式导航与 fake IPC 保存链（原生目录/真实剪贴板仍属桌面人工验收）。 */
test.describe('设置页', () => {
  test('七个真实分区、section 样式与响应式导航保持', async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 })
    await loginAndEnterContest(page)
    await page.goto('/contest/settings')
    await expect(page.getByText('环境设置', { exact: true })).toBeVisible()

    await expect(page.locator('section.settings-section')).toHaveCount(7)
    const ids = await page.locator('section.settings-section').evaluateAll((sections) =>
      sections.map((section) => section.id))
    expect(ids).toEqual(Array.from({ length: 7 }, (_, index) => `settings-${index}`))

    const headingSize = await page.locator('#settings-0 h2').evaluate(
      (element) => getComputedStyle(element).fontSize)
    expect(headingSize).toBe('18px')

    const inputStyle = await page.locator('#settings-0 input[inputmode="numeric"]').first().evaluate(
      (element) => {
        const style = getComputedStyle(element)
        return {
          borderTopWidth: style.borderTopWidth,
          borderBottomWidth: style.borderBottomWidth,
          borderRadius: style.borderRadius,
          fontFamily: style.fontFamily,
        }
      })
    expect(inputStyle).toMatchObject({
      borderTopWidth: '0px',
      borderBottomWidth: '1px',
      borderRadius: '0px',
    })
    expect(inputStyle.fontFamily).toContain('JetBrains Mono')

    const indexWidth = await page.locator('.settings-index').evaluate(
      (element) => getComputedStyle(element).width)
    expect(indexWidth).toBe('175px')
    await page.setViewportSize({ width: 1024, height: 700 })
    await expect.poll(() => page.locator('.settings-index').evaluate(
      (element) => getComputedStyle(element).width)).toBe('140px')

    await page.locator('a[href="#settings-6"]').click()
    await expect(page.locator('#settings-6')).toBeFocused()
    await expect(page.locator('a[href="#settings-6"]')).toHaveAttribute('aria-current', 'true')

    await page.locator('a[href="#settings-0"]').click()
    await expect(page.locator('#settings-0')).toBeFocused()
    await expect(page.locator('a[href="#settings-0"]')).toHaveAttribute('aria-current', 'true')
  })

  test('真实表单保存经 update_config 落盘，重新进入后保持', async ({ page }) => {
    await loginAndEnterContest(page)
    await page.goto('/contest/settings')
    const fontSize = page.locator('label').filter({ hasText: '字号' }).locator('input')
    await expect(fontSize).toHaveValue('14')

    await fontSize.fill('16')
    await page.getByRole('button', { name: '保存', exact: true }).click()
    await expect.poll(() => callCount(page, 'update_config')).toBe(1)
    await expect(page.getByText('已保存', { exact: true })).toBeVisible()

    await page.reload()
    await page.goto('/contest/settings')
    await expect(page.locator('label').filter({ hasText: '字号' }).locator('input'))
      .toHaveValue('16')
  })
})
