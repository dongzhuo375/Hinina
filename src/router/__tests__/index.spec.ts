// @vitest-environment jsdom
import { describe, expect, it } from 'vitest'
import { createMemoryHistory, createRouter } from 'vue-router'
import { routes } from '../index'

/**
 * 守卫的 requiresAuth 判据依赖**合并后的 `to.meta`**（vue-router 沿匹配链由
 * 父到子合并、子路由同键覆盖父路由）。这里用真实 routes 逐条锁定合并结果 ——
 * 判据本身是一行 `=== true`，真正会漂移的是「哪条路由声明了什么 meta」。
 */
function mergedMeta(name: string): Record<string, unknown> {
  const router = createRouter({ history: createMemoryHistory(), routes })
  return router.resolve({ name })['meta']
}

describe('requiresAuth 合并矩阵（--settings 豁免的基础）', () => {
  it('设置子路由豁免外壳父级的保护（子级 false 覆盖父级 true）', () => {
    expect(mergedMeta('Settings')['requiresAuth']).toBe(false)
  })

  it('工作台其余子路由继承外壳父级的保护', () => {
    expect(mergedMeta('Contest')['requiresAuth']).toBe(true)
    expect(mergedMeta('ProblemSet')['requiresAuth']).toBe(true)
    expect(mergedMeta('Announcements')['requiresAuth']).toBe(true)
  })

  it('登录页与 catch-all 不受保护（无 meta 声明即不受保护）', () => {
    expect(mergedMeta('Login')['requiresAuth']).toBeUndefined()
  })
})
