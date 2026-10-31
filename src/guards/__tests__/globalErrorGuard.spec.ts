// @vitest-environment jsdom
// 全局错误转发守卫：四类钩子触发 → 注入的 forward 收到归一化消息；
// 限频去重；forward 抛错不炸守卫；IpcError 归一为 ipc:{cmd}。
// 每个用例自建 app + router 并在 afterEach 统一拆卸 —— 单个断言失败不得把
// 监听器泄漏进后续用例（否则计数类断言会互相污染）。
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, defineComponent, h, type App } from 'vue'
import { createMemoryHistory, createRouter, type Router } from 'vue-router'
import { installGlobalErrorGuard } from '@/guards/globalErrorGuard'
import type { FrontendLogLevel } from '@/bridge/diagnostics.bridge'

type ForwardCall = { level: FrontendLogLevel; scope: string; message: string }

let calls: ForwardCall[] = []
let disposers: Array<() => void> = []

beforeEach(() => {
  calls = []
})

afterEach(() => {
  // 先拆卸再恢复 spy：断言失败的用例也保证监听器被移除
  for (const dispose of disposers) {
    try {
      dispose()
    } catch {
      /* 拆卸失败不影响其余用例 */
    }
  }
  disposers = []
  vi.restoreAllMocks()
})

function setup(): { app: App; router: Router } {
  const app = createApp(defineComponent({ render: () => h('div') }))
  const router = createRouter({ history: createMemoryHistory(), routes: [] })
  disposers.push(
    installGlobalErrorGuard(app, router, (level, scope, message) => {
      calls.push({ level, scope, message })
    }),
  )
  return { app, router }
}

function makeError(): Error {
  return new Error('boom')
}

function emitWindowError(init: { message?: string; error?: Error; target?: EventTarget }): void {
  const event = new ErrorEvent('error', {
    message: init.message,
    filename: 'app.js',
    ...(init.error ? { error: init.error } : {}),
  })
  Object.defineProperty(event, 'target', { value: init.target ?? window })
  window.dispatchEvent(event)
}

function emitRejection(reason: unknown): void {
  // jsdom 未实现 PromiseRejectionEvent 构造器：用 CustomEvent 承载同名字段
  const event = new CustomEvent('unhandledrejection')
  Object.defineProperty(event, 'reason', { value: reason })
  window.dispatchEvent(event as unknown as PromiseRejectionEvent)
}

describe('installGlobalErrorGuard', () => {
  it('四类钩子触发都转发 error 级日志，归一化消息携带环节/来源', async () => {
    const { app, router } = setup()
    // 路由错误的确定性触发源：导航守卫抛错必然进入 router.onError
    router.beforeEach(() => {
      throw new Error('guard exploded')
    })

    app.config.errorHandler!(makeError(), null, 'setup function')
    emitWindowError({ error: makeError() })
    emitRejection(new Error('rejected!'))
    await router.push('/').catch(() => {})

    expect(calls.length).toBe(4)
    expect(calls[0]).toMatchObject({
      level: 'error',
      scope: 'vue',
      message: 'boom (vue:setup function)',
    })
    expect(calls[1]).toMatchObject({ level: 'error', scope: 'window', message: 'boom' })
    expect(calls[2]).toMatchObject({ level: 'error', scope: 'promise', message: 'rejected!' })
    expect(calls[3]).toMatchObject({ level: 'error', scope: 'router', message: 'guard exploded' })
  })

  it('拆卸后钩子不再转发（errorHandler 恢复 undefined、监听器移除）', () => {
    const { app } = setup()
    const disposer = disposers[disposers.length - 1]!
    disposer()

    expect(app.config.errorHandler).toBeUndefined()
    // 不用 window 为 target 的 ErrorEvent 验证移除：jsdom 会把「无监听的 window
    // error 事件（带 error 属性）」上报为 Uncaught Exception，污染 vitest 结果。
    // 资源加载形态（target 非 window）与 rejection 无此副作用。
    const img = document.createElement('img')
    img.setAttribute('src', 'assets/after-dispose.png')
    emitWindowError({ target: img })
    emitRejection(new Error('after dispose'))
    expect(calls).toHaveLength(0)
  })

  it('IpcError（cmd+message 鸭子识别）归一为 ipc:{cmd}: message', () => {
    setup()

    emitRejection({ cmd: 'list_contests', message: '登录失败' })

    expect(calls[0]?.message).toBe('ipc:list_contests: 登录失败')
  })

  it('无消息的资源加载错误从 src/href 取址', () => {
    setup()

    const img = document.createElement('img')
    img.setAttribute('src', 'assets/missing.png')
    emitWindowError({ target: img })

    // jsdom 会把 src 解析成绝对 URL，断言文件名部分
    expect(calls[0]).toMatchObject({
      scope: 'window',
      message: expect.stringContaining('资源加载失败: '),
    })
    expect(calls[0]?.message).toContain('missing.png')
  })

  it('同签名错误 1 秒内只转发一次；不同签名互不影响', () => {
    vi.useFakeTimers()
    setup()

    emitRejection(new Error('same'))
    emitRejection(new Error('same'))
    emitRejection(new Error('other'))

    expect(calls).toHaveLength(2)
    expect(calls.map((c) => c.message)).toEqual(['same', 'other'])

    vi.advanceTimersByTime(1001)
    emitRejection(new Error('same'))
    expect(calls).toHaveLength(3) // 窗口过后再次转发
  })

  it('forward 抛错被吞掉，守卫存活且后续错误照常转发', () => {
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
    let shouldThrow = true
    const { app, router } = setup()
    // 覆盖默认注入：forward 主动抛错
    const last = disposers[disposers.length - 1]!
    last()
    disposers.push(
      installGlobalErrorGuard(app, router, () => {
        if (shouldThrow) throw new Error('forward failed')
        calls.push({ level: 'error', scope: 'window', message: 'ok' })
      }),
    )

    expect(() => emitWindowError({ error: makeError() })).not.toThrow()
    expect(calls).toHaveLength(0)

    shouldThrow = false
    emitWindowError({ error: makeError() })
    expect(calls).toHaveLength(1)
    // 守卫没有把转发失败变成新的 unhandled rejection（无二次 console.error）
    expect(errorSpy).not.toHaveBeenCalled()
  })

  it('空消息与非 Error reason 归一为可读文本', () => {
    setup()

    emitRejection(undefined)
    emitRejection('plain string')
    emitRejection({ code: 7 })

    expect(calls[0]?.message).toBe('<empty error>')
    expect(calls[1]?.message).toBe('plain string')
    expect(calls[2]?.message).toBe('{"code":7}')
  })
})
