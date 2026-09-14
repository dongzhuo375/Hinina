import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

/// vi.mock 会被提升到文件顶部，工厂内引用的变量必须经 vi.hoisted 声明
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke }))

import { IpcError, ipcInvoke, setIpcErrorObserver } from '@/bridge'

/// Rust `AppError` 经 serde 外部标签序列化后的载荷形态
const appError = (variant: string, message: string) => ({ [variant]: message })

/// 执行一次必然失败的调用并取回抛出的错误
async function captureError(cmd = 'x'): Promise<IpcError> {
  return (await ipcInvoke(cmd).catch((e: unknown) => e)) as IpcError
}

beforeEach(() => {
  // 静默被测代码的错误日志，避免污染测试输出
  vi.spyOn(console, 'error').mockImplementation(() => {})
})

afterEach(() => {
  setIpcErrorObserver(null)
})

describe('ipcInvoke — 成功路径', () => {
  it('透传命令名与参数并返回后端结果', async () => {
    invoke.mockResolvedValue({ ok: 1 })
    await expect(ipcInvoke('get_config', { a: 1 })).resolves.toEqual({ ok: 1 })
    expect(invoke).toHaveBeenCalledWith('get_config', { a: 1 })
  })
})

describe('ipcInvoke — 错误归一化（Rust AppError ↔ 前端跨端契约）', () => {
  it('AppError 载荷 → IpcError，保留变体与真实原因', async () => {
    const payload = appError('Auth', '登录失败: 用户名或密码错误')
    invoke.mockRejectedValue(payload)

    const error = await captureError('login')
    expect(error).toBeInstanceOf(IpcError)
    // 上层 `e instanceof Error` 判定成立，真实原因不再被兜底文案吞掉
    expect(error).toBeInstanceOf(Error)
    expect(error.message).toBe('登录失败: 用户名或密码错误')
    expect(error.variant).toBe('Auth')
    expect(error.isAuthError).toBe(true)
    expect(error.cmd).toBe('login')
    expect(error.raw).toEqual(payload)
  })

  it('非认证类错误的 isAuthError 为 false（会话守卫不会误触发）', async () => {
    invoke.mockRejectedValue(appError('Contest', '比赛不存在'))
    const error = await captureError('load_configured_contest')
    expect(error.variant).toBe('Contest')
    expect(error.isAuthError).toBe(false)
  })

  it('未知变体降级为 variant=null，但仍保留消息', async () => {
    // Rust 端新增 AppError 变体时前端列表会滞后，必须优雅降级而非丢消息
    invoke.mockRejectedValue(appError('SomeFutureVariant', '新错误'))
    const error = await captureError()
    expect(error.variant).toBeNull()
    expect(error.message).toBe('新错误')
  })

  it('字符串载荷按消息处理', async () => {
    invoke.mockRejectedValue('command not found')
    const error = await captureError()
    expect(error.variant).toBeNull()
    expect(error.message).toBe('command not found')
  })

  it('Error 实例保留原消息', async () => {
    invoke.mockRejectedValue(new Error('boom'))
    const error = await captureError()
    expect(error.message).toBe('boom')
    expect(error.variant).toBeNull()
  })

  it('多键对象回退为 JSON 文本', async () => {
    const payload = { a: 1, b: 'x' }
    invoke.mockRejectedValue(payload)
    expect((await captureError()).message).toBe(JSON.stringify(payload))
  })

  it('安全约束：日志只含命令名与消息，绝不包含调用参数（明文密码）', async () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {})
    invoke.mockRejectedValue(appError('Auth', '登录失败'))

    await ipcInvoke('login', { username: 'team01', password: 'S3CRET-PIN' }).catch(() => {})

    const logged = spy.mock.calls.flat().join(' ')
    expect(logged).toContain('login')
    expect(logged).not.toContain('S3CRET-PIN')
    expect(logged).not.toContain('team01')
  })
})

describe('setIpcErrorObserver — 组合根注入的全局观察者', () => {
  it('错误发生时收到同一个 IpcError 实例', async () => {
    const observer = vi.fn()
    setIpcErrorObserver(observer)
    invoke.mockRejectedValue(appError('Auth', '会话失效'))

    const thrown = await captureError('get_problem')
    expect(observer).toHaveBeenCalledTimes(1)
    expect(observer.mock.calls[0]![0]).toBe(thrown)
  })

  it('传 null 注销后不再回调', async () => {
    const observer = vi.fn()
    setIpcErrorObserver(observer)
    setIpcErrorObserver(null)
    invoke.mockRejectedValue(appError('Auth', 'x'))

    await ipcInvoke('y').catch(() => {})
    expect(observer).not.toHaveBeenCalled()
  })

  it('未注册观察者时不影响错误抛出', async () => {
    setIpcErrorObserver(null)
    invoke.mockRejectedValue(appError('Network', '超时'))
    await expect(ipcInvoke('z')).rejects.toThrow('超时')
  })
})
