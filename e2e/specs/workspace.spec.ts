import { expect, test } from '@playwright/test'
import { callCount, loginAndEnterContest } from '../fake-tauri'

/**
 * §6.3 场景 3+4（收敛）：题目加载、编辑器输入防抖推送、切题与回切内容恢复。
 * 工作区按题目真实 ID（pid）在 fake 内存隔离，对应 Rust 侧存储语义。
 */
test.describe('题目与工作区', () => {
  test('加载 → 编辑 → 切题 → 回切内容恢复', async ({ page }) => {
    await loginAndEnterContest(page)
    await page.getByRole('button', { name: '打开题目 A' }).click()

    // 题面按展示题号加载
    await expect(page.locator('h1.statement-title')).toHaveText(/^A\./)
    await expect(page.locator('.monaco-editor')).toBeVisible()
    // 初始代码来自 load_workspace fixture（A 题）
    await expect(page.locator('.monaco-editor .view-lines')).toContainText('int main()')

    // 编辑 → 2s 防抖后经 update_workspace_file 推送到后端内存
    await page.locator('.monaco-editor .view-lines').click()
    await page.keyboard.type('// e2e edited')
    await expect
      .poll(() => callCount(page, 'update_workspace_file'), { timeout: 10_000 })
      .toBeGreaterThanOrEqual(1)

    // 切到 B：B 题工作区独立（不携带 A 的编辑内容）
    await page.getByRole('button', { name: 'B', exact: true }).click()
    await expect(page.locator('h1.statement-title')).toHaveText(/^B\./)
    await expect(page.locator('.monaco-editor .view-lines')).toContainText('B 题工作区初始代码')

    // 切回 A：内容恢复（工作区按 pid 隔离的恢复承诺）
    await page.getByRole('button', { name: 'A', exact: true }).click()
    await expect(page.locator('h1.statement-title')).toHaveText(/^A\./)
    await expect(page.locator('.monaco-editor .view-lines')).toContainText('// e2e edited')
  })

  test('编辑推送的文件名与内容与编辑器一致', async ({ page }) => {
    await loginAndEnterContest(page)
    await page.getByRole('button', { name: '打开题目 B' }).click()
    await expect(page.locator('.monaco-editor .view-lines')).toContainText('B 题工作区初始代码')

    await page.locator('.monaco-editor .view-lines').click()
    await page.keyboard.press('ControlOrMeta+a')
    await page.keyboard.type('int main(void) { return 1; }')

    // 不能只等 count >= 1 再读最后一次：慢打字下 2s 防抖可能先 flush 出内容不完整
    // 的中间快照，读到它会让 toContain 偶发失败——必须等「携带预期内容」的那次调用
    //（评审二轮竞态）
    await expect
      .poll(async () => {
        const calls = await page.evaluate(() =>
          window.__e2e.calls.filter((c) => c.cmd === 'update_workspace_file'),
        )
        return calls.some((c) => String(c.args.content).includes('return 1;'))
      }, { timeout: 10_000 })
      .toBe(true)

    // 最后一次调用即最终快照（打字结束后无新输入，不会再触发防抖）
    const calls = await page.evaluate(() =>
      window.__e2e.calls.filter((c) => c.cmd === 'update_workspace_file'),
    )
    const last = calls[calls.length - 1]
    expect(last.args.fileName).toBe('main.cpp')
    expect(String(last.args.content)).toContain('return 1;')
  })
})
