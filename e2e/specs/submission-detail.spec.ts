import { expect, test } from '../fixtures'
import { callCount, loginAndEnterContest } from '../fake-tauri'

/**
 * §6.3 补缺（2026-11 审计 C4）：提交详情页 —— 终态详情（状态/耗时/测试点/源代码）
 * 与评测中详情（judging 形态 + 明细自动刷新占位）。fake 的详情按会话内提交构造，
 * code 取提交原文、测试点按终态状态生成，与真实契约（SubmissionDetail/
 * SubmissionCases）形状一致。
 */
test.describe('提交详情', () => {
  test('终态详情：状态 pill、耗时、测试点明细与源代码原文', async ({ page }) => {
    await loginAndEnterContest(page)
    await page.getByRole('button', { name: '打开题目 A' }).click()
    await expect(page.locator('h1.statement-title')).toHaveText(/^A\./)

    // 先写入可辨识的源码：详情页的 code 字段必须回显提交原文（而非空白兜底）
    await page.locator('.monaco-editor .view-lines').click()
    await page.keyboard.type('int main() { return 0; }')
    await page.getByRole('button', { name: '提交代码' }).click()
    await expect(page.getByText(/最新记录: #9001 (Accepted|AC)/)).toBeVisible({ timeout: 20_000 })

    await page.getByRole('button', { name: /^提交记录/ }).click()
    await page.waitForURL('**/contest/submissions**')
    await expect(page.getByText('#9001').first()).toBeVisible()
    await page.getByRole('button', { name: '查看详情' }).first().click()
    await page.waitForURL('**/contest/submissions/9001')

    // 形态断言前等业务加载标记（详情标题），避免加载态空窗
    await expect(page.getByRole('heading', { name: /提交 #9001/ })).toBeVisible()
    await expect(page.getByText('Accepted').first()).toBeVisible()
    await expect(page.getByText('12 ms')).toBeVisible()

    // 测试点明细：fake 为终态生成 3 个 AC 用例
    await expect(page.getByRole('heading', { name: '测试点明细' })).toBeVisible()
    await expect(page.getByText('3/3 通过')).toBeVisible()

    // 源代码默认折叠：展开后是只读 Monaco，回显提交原文
    //（monaco 按 token 切 span，用单个 token 子串断言避免跨 span 拼接差异）
    await page.getByRole('button', { name: /源代码/ }).click()
    await expect(page.locator('.monaco-editor .view-lines')).toContainText('return 0', {
      timeout: 10_000,
    })
    await expect.poll(async () => callCount(page, 'get_submission_detail')).toBeGreaterThanOrEqual(1)
    await expect.poll(async () => callCount(page, 'get_submission_cases')).toBeGreaterThanOrEqual(1)
  })

  test('评测中详情：judging 标记与测试点自动刷新占位', async ({ page }) => {
    // 评测序列恒为 Running（非终态）：detail 页保持 judging 形态，明细不加载
    await loginAndEnterContest(page, { judgementSequence: ['Running'] })
    await page.getByRole('button', { name: '打开题目 A' }).click()
    await expect(page.locator('h1.statement-title')).toHaveText(/^A\./)

    await page.getByRole('button', { name: '提交代码' }).click()
    await expect(page.getByRole('status')).toContainText('已提交 #9001')

    await page.getByRole('button', { name: /^提交记录/ }).click()
    await page.waitForURL('**/contest/submissions**')
    await page.getByRole('button', { name: '查看详情' }).first().click()
    await page.waitForURL('**/contest/submissions/9001')

    await expect(page.getByRole('heading', { name: /提交 #9001/ })).toBeVisible()
    // judging 形态：状态标签 Judging + 明细区自动刷新占位（终态才拉 get_submission_cases）
    await expect(page.getByText('Judging').first()).toBeVisible()
    await expect(page.getByText('评测进行中，测试点结果将自动刷新…')).toBeVisible()
    await expect.poll(async () => callCount(page, 'get_submission_cases')).toBe(0)
    // 详情页自身维持轮询（非终态不停止）
    await expect.poll(async () => callCount(page, 'get_submission_detail')).toBeGreaterThanOrEqual(2)
  })
})
