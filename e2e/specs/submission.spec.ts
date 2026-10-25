import { expect, test } from '@playwright/test'
import { callCount, loginAndEnterContest } from '../fake-tauri'

/**
 * §6.3 场景 5（收敛）：提交、防重复、评测轮询到终态。
 * fake 评测序列 Pending→Running→Accepted（1s 快轮询），终态停轮。
 */
test.describe('提交流', () => {
  test('提交 → 评测中禁用重复提交 → 轮询到 Accepted 终态', async ({ page }) => {
    await loginAndEnterContest(page)
    await page.getByRole('button', { name: '打开题目 A' }).click()
    await expect(page.locator('h1.statement-title')).toHaveText(/^A\./)

    await page.getByRole('button', { name: '提交代码' }).click()

    // 提交已受理：pending 期按钮禁用、状态行提示已提交
    const busyButton = page.getByRole('button', { name: /评测中|提交中/ })
    await expect(busyButton).toBeDisabled()
    await expect(page.getByRole('status')).toContainText('已提交 #')
    // 此刻只应发生一次 submit_code（视图禁用 + store 五元组去重，双保险）
    expect(await callCount(page, 'submit_code')).toBe(1)

    // 轮询推进：终态 pill（本地 pill 文案 "Accepted"，摘要追上后变为服务端缩写 "AC"，
    // 二者都是终态信号；fake 摘要零延迟，本地形态可能一闪而过）
    await expect(page.getByText(/最新记录: #9001 (Accepted|AC)/)).toBeVisible({ timeout: 20_000 })
    await expect(page.getByRole('button', { name: '提交代码' })).toBeEnabled()
    expect(await callCount(page, 'submit_code')).toBe(1)
    // 终态前至少完成两次轮询（Pending → Running → Accepted）
    expect(await callCount(page, 'get_judgement')).toBeGreaterThanOrEqual(2)
  })

  test('提交历史列表反映会话内提交', async ({ page }) => {
    await loginAndEnterContest(page)
    await page.getByRole('button', { name: '打开题目 A' }).click()
    await expect(page.locator('h1.statement-title')).toHaveText(/^A\./)

    await page.getByRole('button', { name: '提交代码' }).click()
    await expect(page.getByText(/最新记录: #9001 (Accepted|AC)/)).toBeVisible({ timeout: 20_000 })

    await page.getByRole('button', { name: /^提交记录/ }).click()
    await page.waitForURL('**/contest/submissions**')
    await expect(page.getByText('#9001').first()).toBeVisible()
  })

  test('pending 期间离开再回来复用同一提交，不重复发送 IPC', async ({ page }) => {
    // 评测序列恒为 Running（非终态）：pending 窗口覆盖整个用例，断言不受轮询推进影响
    await loginAndEnterContest(page, { judgementSequence: ['Running'] })
    await page.getByRole('button', { name: '打开题目 A' }).click()
    await expect(page.locator('h1.statement-title')).toHaveText(/^A\./)

    await page.getByRole('button', { name: '提交代码' }).click()
    await expect(page.getByRole('status')).toContainText('已提交 #9001')
    expect(await callCount(page, 'submit_code')).toBe(1)

    // 路由往返（解题页卸载重挂）：pendingFor 五元组仍匹配，按钮保持「评测中…」禁用，
    // 且不产生新的 submit_code —— 五元组复用语义的可观测面（store 内部去重行由
    // submissionStore.spec 单测覆盖，UI 全部调用方都预守卫，E2E 不可直达）
    await page.getByRole('link', { name: '返回题目总览' }).click()
    await page.waitForURL('**/contest/problems')
    await page.getByRole('button', { name: '打开题目 A' }).click()
    await expect(page.locator('h1.statement-title')).toHaveText(/^A\./)
    await expect(page.getByRole('button', { name: '评测中…' })).toBeDisabled()
    await expect(page.getByRole('status')).toContainText('已提交 #9001')
    expect(await callCount(page, 'submit_code')).toBe(1)
  })
})
