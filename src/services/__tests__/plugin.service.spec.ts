// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest'

const { onNotification } = vi.hoisted(() => ({ onNotification: vi.fn() }))
vi.mock('@/bridge/plugin.bridge', () => ({ pluginBridge: { onNotification } }))

import { pluginService } from '@/services/plugin.service'

beforeEach(() => { vi.useFakeTimers(); vi.clearAllMocks() })
afterEach(() => vi.useRealTimers())

it('validates, bounds, expires, and clears plugin notifications in the service layer', async () => {
  let receive!: (value: unknown) => void
  const unlisten = vi.fn()
  onNotification.mockImplementation(handler => { receive = handler; return Promise.resolve(unlisten) })
  const added: Array<{ id: number, message: string }> = []
  const removed: number[] = []
  const dispose = await pluginService.subscribeNotifications(value => added.push(value), id => removed.push(id))
  receive({ pluginId: 'test.demo', level: 'info', message: '<script>literal</script>' })
  for (let index = 0; index < 6; index++) receive({ pluginId: 'test.demo', level: 'warn', message: String(index) })
  receive({ pluginId: 7, level: 'info', message: 'bad' })
  receive({ pluginId: 'test.demo', level: 'unexpected', message: 'bad' })
  receive({ pluginId: 'test.demo', level: 'info', message: 'x'.repeat(513) })
  expect(added).toHaveLength(7)
  expect(removed).toHaveLength(2)
  expect(added[0].message).toBe('<script>literal</script>')
  vi.advanceTimersByTime(6000)
  expect(removed).toHaveLength(7)
  dispose()
  expect(unlisten).toHaveBeenCalledOnce()
})

it('dismissal and disposal cancel timers and reject late delivery', async () => {
  let receive!: (value: unknown) => void
  const unlisten = vi.fn()
  onNotification.mockImplementation(handler => { receive = handler; return Promise.resolve(unlisten) })
  const added = vi.fn()
  const removed = vi.fn()
  const dispose = await pluginService.subscribeNotifications(added, removed)
  const notification = { pluginId: 'test.demo', level: 'info', message: 'fixture' }
  receive(notification)
  dispose.dismiss(added.mock.calls[0][0].id)
  expect(vi.getTimerCount()).toBe(0)
  receive(notification)
  dispose()
  dispose()
  expect(unlisten).toHaveBeenCalledOnce()
  expect(removed).toHaveBeenCalledTimes(2)
  receive(notification)
  vi.advanceTimersByTime(6000)
  expect(added).toHaveBeenCalledTimes(2)
  expect(removed).toHaveBeenCalledTimes(2)
  expect(vi.getTimerCount()).toBe(0)
})

it('clears any early notifications when listener setup rejects', async () => {
  const removed = vi.fn()
  onNotification.mockImplementation(handler => {
    handler({ pluginId: 'test.demo', level: 'info', message: 'early' })
    return Promise.reject(new Error('subscription failed'))
  })
  await expect(pluginService.subscribeNotifications(vi.fn(), removed)).rejects.toThrow('subscription failed')
  expect(vi.getTimerCount()).toBe(0)
  expect(removed).toHaveBeenCalledOnce()
})

// Subscription supersession is Service policy: one active listener, late arrivals discarded.
type Pending = { receive: (value: unknown) => void, unlisten: ReturnType<typeof vi.fn>, resolve: () => void }
function pendingListeners() {
  const listeners: Pending[] = []
  onNotification.mockImplementation(handler => {
    const unlisten = vi.fn()
    return new Promise<() => void>(resolve => {
      listeners.push({ receive: handler, unlisten, resolve: () => resolve(unlisten) })
    })
  })
  return listeners
}
const sample = (message: string) => ({ pluginId: 'test.demo', level: 'info', message })

it('re-subscribing disposes the previous subscription instead of leaking it', async () => {
  const listeners = pendingListeners()
  const added: string[] = []
  const first = pluginService.subscribeNotifications(v => added.push(v.message), vi.fn())
  listeners[0].resolve(); await first
  const second = pluginService.subscribeNotifications(v => added.push(v.message), vi.fn())
  listeners[1].resolve(); const stop = await second
  expect(listeners[0].unlisten).toHaveBeenCalledOnce()
  listeners[0].receive(sample('leaked'))
  listeners[1].receive(sample('active'))
  expect(added).toEqual(['active'])
  stop()
})

it('disposes an overlapping subscription that resolves after a newer one superseded it', async () => {
  const listeners = pendingListeners()
  const added: Array<{ id: number, message: string }> = []
  const removed: number[] = []
  const first = pluginService.subscribeNotifications(v => added.push(v), id => removed.push(id))
  const second = pluginService.subscribeNotifications(v => added.push(v), id => removed.push(id))
  expect(listeners).toHaveLength(2)
  // The newer call settles first; the superseded one then arrives late.
  listeners[1].resolve(); await second
  listeners[0].resolve(); await first
  expect(listeners[0].unlisten).toHaveBeenCalledOnce()
  expect(listeners[1].unlisten).not.toHaveBeenCalled()
  listeners[0].receive(sample('late'))
  listeners[1].receive(sample('active'))
  expect(added.map(item => item.message)).toEqual(['active'])
  // dismissNotification targets the active subscription only.
  pluginService.dismissNotification(added[0].id)
  expect(removed).toEqual([added[0].id])
  ;(await second)()
})

it('disposing a superseded subscription does not disturb the newer one', async () => {
  const listeners = pendingListeners()
  const added: string[] = []
  const first = pluginService.subscribeNotifications(v => added.push(v.message), vi.fn())
  listeners[0].resolve(); const firstStop = await first
  const second = pluginService.subscribeNotifications(v => added.push(v.message), vi.fn())
  listeners[1].resolve(); const secondStop = await second
  firstStop()
  expect(listeners[1].unlisten).not.toHaveBeenCalled()
  listeners[1].receive(sample('still active'))
  expect(added).toEqual(['still active'])
  secondStop()
  expect(listeners[1].unlisten).toHaveBeenCalledOnce()
})
