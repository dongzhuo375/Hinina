// @vitest-environment jsdom
// 全局错误转发守卫：四类钩子触发 → 注入的 forward 收到归一化消息；
// 限频去重；forward 抛错不炸守卫；IpcError 归一为 ipc:{cmd}。
// 每个用例自建 app + router 并在 afterEach 统一拆卸 —— 单个断言失败不得把
// 监听器泄漏进后续用例（否则计数类断言会互相污染）。
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, defineComponent, h, type App } from 'vue'
import { createMemoryHistory, createRouter, type Router } from 'vue-router'
import { installGlobalErrorGuard } from '@/guards/globalErrorGuard'
import { IpcError } from '@/bridge'
import type { FrontendLogLevel } from '@/bridge/diagnostics.bridge'

type ForwardCall = { level: FrontendLogLevel; scope: string; message: string }

let calls: ForwardCall[] = []
let disposers: Array<() => void> = []

beforeEach(() => {
  calls = []
  // errorHandler 会补 console.error（Vue 设了 handler 后不再走默认输出）；
  // 全文件静音，个别用例自行 spy 断言
  vi.spyOn(console, 'error').mockImplementation(() => {})
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
    // Error reason 带 stack（V8 首行含消息，环节后缀在整个栈之后），断言用包含
    expect(calls[0]).toMatchObject({
      level: 'error',
      scope: 'vue',
      message: expect.stringContaining('boom'),
    })
    expect(calls[0]?.message.endsWith('(vue:setup function)')).toBe(true)
    expect(calls[1]).toMatchObject({ level: 'error', scope: 'window', message: expect.stringContaining('boom') })
    expect(calls[2]).toMatchObject({ level: 'error', scope: 'promise', message: expect.stringContaining('rejected!') })
    expect(calls[3]).toMatchObject({ level: 'error', scope: 'router', message: expect.stringContaining('guard exploded') })
  })

  it('errorHandler 补发 console.error（Vue 设了 handler 后默认输出消失，DevTools 可见性自补）', () => {
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
    const { app } = setup()

    const boom = new Error('console passthrough')
    app.config.errorHandler!(boom, null, 'setup function')

    expect(errorSpy).toHaveBeenCalledWith(boom, 'setup function')
    expect(calls[0]?.message).toContain('console passthrough')
  })

  it('Error 上报携带堆栈（只取头部若干帧，由 2000 字符上限自然截断）', () => {
    setup()

    emitRejection(new Error('with stack'))

    expect(calls[0]?.message).toContain('with stack')
    expect(calls[0]?.message).toContain('at ') // 至少一帧
  })

  it('按码点截断：边界处的 emoji 不产生孤立代理项（serde_json 拒收即整条丢失）', () => {
    setup()

    // 1999 个 a + 😀（2 码元）+ 10 个 b：码元截断会切在 😀 中间
    emitRejection(`${'a'.repeat(1999)}😀${'b'.repeat(10)}`)

    expect(calls[0]?.message).toContain('😀')
  })

  it('去重键用完整归一化消息：同前缀不同后缀的错误不被误并', () => {
    setup()

    emitRejection(new Error(`${'x'.repeat(300)}-tail-A`))
    emitRejection(new Error(`${'x'.repeat(300)}-tail-B`))

    expect(calls).toHaveLength(2)
    expect(calls[0]?.message).toContain('tail-A')
    expect(calls[1]?.message).toContain('tail-B')
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

  it('IpcError 归一为 ipc:{cmd}: message（须先于 instanceof Error 判定，否则被 stack 分支吞掉）', () => {
    setup()

    // 真实类型：IpcError extends Error —— 用实例而非鸭子对象，
    // 锁定「IpcError 判定先于 Error」的顺序（鸭子对象测不出这个回归）
    emitRejection(new IpcError('list_contests', '登录失败', 'Auth'))

    expect(calls[0]?.message).toBe('ipc:list_contests: 登录失败')
  })

  it('资源加载错误不冒泡 —— capture 阶段监听经真实 DOM 派发捕获（回归：无 capture 则为死代码）', () => {
    setup()

    // 真实 DOM 形态：元素挂进文档、事件派发在**元素**上且不冒泡（HTML 规范）。
    // 若监听未注册在 capture 阶段，本用例必然失败（变异验证过）—— 伪造
    // event.target 后直接向 window 派发只会走 target 阶段，测不出传播路径。
    const img = document.createElement('img')
    img.setAttribute('src', 'https://cdn.example.com/gallery/missing.png?token=SECRET&x=1')
    document.body.appendChild(img)
    img.dispatchEvent(new ErrorEvent('error', { bubbles: false }))

    // 只记标签名与路径末段文件名：完整 URL（含签名令牌查询参数）不得进日志
    expect(calls[0]).toMatchObject({
      level: 'error',
      scope: 'window',
      message: '资源加载失败: <img> missing.png',
    })
    expect(calls[0]?.message).not.toContain('SECRET')
    expect(calls[0]?.message).not.toContain('http')
  })

  it('相对地址的资源错误取末段文件名（URL 解析失败按分隔符兜底）', () => {
    setup()

    const script = document.createElement('script')
    script.setAttribute('src', 'assets/chunk.abc123.js')
    document.body.appendChild(script)
    script.dispatchEvent(new ErrorEvent('error', { bubbles: false }))

    expect(calls[0]?.message).toBe('资源加载失败: <script> chunk.abc123.js')
  })

  it('同签名错误 1 秒内只转发一次；不同签名互不影响', () => {
    vi.useFakeTimers()
    setup()

    // 字符串 reason：归一化后恰好就是原文（Error 的 stack 含构造行号，
    // 两次构造天然不同键，不适合做去重窗口的判定载体）
    emitRejection('same')
    emitRejection('same')
    emitRejection('other')

    expect(calls).toHaveLength(2)
    expect(calls.map((c) => c.message)).toEqual(['same', 'other'])

    vi.advanceTimersByTime(1001)
    emitRejection('same')
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
