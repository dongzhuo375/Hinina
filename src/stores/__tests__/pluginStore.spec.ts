// @vitest-environment jsdom
import { beforeEach, afterEach, it, expect, vi } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'
const { pluginService } = vi.hoisted(() => ({ pluginService: {
  list: vi.fn(), start: vi.fn(), stop: vi.fn(), reload: vi.fn(), resync: vi.fn(), subscribeNotifications: vi.fn(),
} }))
vi.mock('@/services/plugin.service', () => ({ pluginService }))
import { usePluginStore } from '@/stores/pluginStore'
beforeEach(() => { setActivePinia(createPinia()); vi.clearAllMocks(); vi.useFakeTimers() })
afterEach(() => vi.useRealTimers())
it('stores only service-normalized notifications and applies service dismissals', async () => {
  let add!: (value: { id: number, pluginId: string, level: 'info', message: string }) => void
  let remove!: (id: number) => void
  pluginService.subscribeNotifications.mockImplementation((onAdd, onDismiss) => {
    add = onAdd; remove = onDismiss
    return Promise.resolve(Object.assign(() => {}, { dismiss: onDismiss }))
  })
  const store = usePluginStore(); await store.subscribe()
  add({ id: 1, pluginId: 'test.demo', level: 'info', message: '<script>alert(1)</script>' })
  expect(store.notifications).toEqual([{ id: 1, pluginId: 'test.demo', level: 'info', message: '<script>alert(1)</script>' }])
  remove(1)
  expect(store.notifications).toHaveLength(0)
  add({ id: 2, pluginId: 'test.demo', level: 'info', message: 'dismiss' })
  store.dismiss(2)
  expect(store.notifications).toHaveLength(0)
})
it('re-subscribing disposes the previous subscription instead of leaking it', async () => {
  type Emitted = { id: number, pluginId: string, level: 'info', message: string }
  const emitters: Array<(value: Emitted) => void> = []
  const disposed: boolean[] = []
  pluginService.subscribeNotifications.mockImplementation((onAdd: (value: Emitted) => void, onDismiss: (id: number) => void) => {
    const index = emitters.length
    disposed.push(false)
    emitters.push(value => { if (!disposed[index]) onAdd(value) })
    return Promise.resolve(Object.assign(() => { disposed[index] = true }, { dismiss: onDismiss }))
  })
  const store = usePluginStore()
  await store.subscribe()
  await store.subscribe()
  expect(disposed).toEqual([true, false])
  emitters[1]({ id: 1, pluginId: 'test.demo', level: 'info', message: 'active' })
  expect(store.notifications).toHaveLength(1)
  emitters[0]({ id: 2, pluginId: 'test.demo', level: 'info', message: 'leaked' })
  expect(store.notifications).toHaveLength(1)
  expect(store.notifications[0].message).toBe('active')
})
it('disposes overlapping subscriptions that resolve after a newer one superseded them', async () => {
  type Emitted = { id: number, pluginId: string, level: 'info', message: string }
  type Subscription = (() => void) & { dismiss: (id: number) => void }
  const emitters: Array<(value: Emitted) => void> = []
  const disposed: boolean[] = []
  const resolvers: Array<() => void> = []
  pluginService.subscribeNotifications.mockImplementation((onAdd: (value: Emitted) => void, onDismiss: (id: number) => void) => {
    const index = emitters.length
    disposed.push(false)
    emitters.push(value => { if (!disposed[index]) onAdd(value) })
    return new Promise<Subscription>(resolve => {
      resolvers.push(() => resolve(Object.assign(() => { disposed[index] = true }, { dismiss: onDismiss })))
    })
  })
  const store = usePluginStore()
  const first = store.subscribe()
  const second = store.subscribe()
  expect(resolvers).toHaveLength(2)
  // The newer call settles first; the superseded one then arrives late.
  resolvers[1]()
  await second
  resolvers[0]()
  await first
  expect(disposed).toEqual([true, false])
  emitters[0]({ id: 1, pluginId: 'test.demo', level: 'info', message: 'late' })
  emitters[1]({ id: 2, pluginId: 'test.demo', level: 'info', message: 'active' })
  expect(store.notifications.map(item => item.message)).toEqual(['active'])
  store.dismiss(2)
  expect(store.notifications).toHaveLength(0)
})
it('unmounting the current subscription does not disturb a newer one', async () => {
  type Emitted = { id: number, pluginId: string, level: 'info', message: string }
  type Subscription = (() => void) & { dismiss: (id: number) => void }
  const disposed: boolean[] = []
  const emitters: Array<(value: Emitted) => void> = []
  pluginService.subscribeNotifications.mockImplementation((onAdd: (value: Emitted) => void, onDismiss: (id: number) => void) => {
    const index = disposed.length
    disposed.push(false)
    emitters.push(value => { if (!disposed[index]) onAdd(value) })
    return Promise.resolve<Subscription>(Object.assign(() => { disposed[index] = true }, { dismiss: onDismiss }))
  })
  const store = usePluginStore()
  const firstStop = await store.subscribe()
  await store.subscribe()
  firstStop()
  expect(disposed).toEqual([true, false])
  emitters[1]({ id: 1, pluginId: 'test.demo', level: 'info', message: 'active' })
  expect(store.notifications).toHaveLength(1)
})
it('refreshes status only after a lifecycle operation completes', async () => {
  pluginService.list.mockResolvedValue([{ id: 'test.demo', state: 'running', error: null }])
  const store = usePluginStore(); await store.start('test.demo')
  expect(pluginService.start).toHaveBeenCalledWith('test.demo')
  expect(store.plugins[0].state).toBe('running')
})
