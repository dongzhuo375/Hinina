import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createLogger } from '@/utils/logger'

beforeEach(() => {
  vi.spyOn(console, 'debug').mockImplementation(() => {})
  vi.spyOn(console, 'info').mockImplementation(() => {})
  vi.spyOn(console, 'warn').mockImplementation(() => {})
  vi.spyOn(console, 'error').mockImplementation(() => {})
})

afterEach(() => {
  vi.restoreAllMocks()
})

describe('createLogger — 作用域前缀与级别分流', () => {
  it('统一加 [scope] 前缀，原始参数原样透传（便于 DevTools 展开对象）', () => {
    const log = createLogger('configService')
    const err = new Error('IPC 失败')

    log.warn('读取编辑器配置失败，使用兜底值:', err)

    expect(console.warn).toHaveBeenCalledWith('[configService]', '读取编辑器配置失败，使用兜底值:', err)
  })

  it('warn / error 始终输出（现场排障的唯一线索，不按环境关闭）', () => {
    const log = createLogger('workspaceStore')

    log.warn('warn 文案')
    log.error('error 文案')

    expect(console.warn).toHaveBeenCalledTimes(1)
    expect(console.error).toHaveBeenCalledTimes(1)
  })

  it('debug / info 仅开发环境输出', () => {
    const log = createLogger('ipc')

    log.debug('debug 文案')
    log.info('info 文案')

    const expected = import.meta.env.DEV ? 1 : 0
    expect(console.debug).toHaveBeenCalledTimes(expected)
    expect(console.info).toHaveBeenCalledTimes(expected)
  })

  it('不同作用域互不串扰', () => {
    createLogger('a').error('x')
    createLogger('b').error('x')

    expect(console.error).toHaveBeenNthCalledWith(1, '[a]', 'x')
    expect(console.error).toHaveBeenNthCalledWith(2, '[b]', 'x')
  })
})
