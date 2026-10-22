// @vitest-environment jsdom
import { beforeEach, afterEach, it, expect, vi } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'
const { pluginService } = vi.hoisted(() => ({ pluginService: {
  list: vi.fn(), start: vi.fn(), stop: vi.fn(), reload: vi.fn(), resync: vi.fn(),
  subscribeNotifications: vi.fn(), dismissNotification: vi.fn(),
} }))
vi.mock('@/services/plugin.service', () => ({ pluginService }))
import { usePluginStore } from '@/stores/pluginStore'
beforeEach(() => { setActivePinia(createPinia()); vi.clearAllMocks(); vi.useFakeTimers() })
afterEach(() => vi.useRealTimers())
type Emitted = { id: number, pluginId: string, level: 'info', message: string }
it('stores only service-normalized notifications and applies service dismissals', async () => {
  let add!: (value: Emitted) => void
  let remove!: (id: number) => void
  pluginService.subscribeNotifications.mockImplementation((onAdd, onDismiss) => {
    add = onAdd; remove = onDismiss
    return Promise.resolve(Object.assign(() => {}, { dismiss: onDismiss }))
  })
  pluginService.dismissNotification.mockImplementation((id: number) => remove(id))
  const store = usePluginStore(); await store.subscribe()
  add({ id: 1, pluginId: 'test.demo', level: 'info', message: '<script>alert(1)</script>' })
  expect(store.notifications).toEqual([{ id: 1, pluginId: 'test.demo', level: 'info', message: '<script>alert(1)</script>' }])
  remove(1)
  expect(store.notifications).toHaveLength(0)
  add({ id: 2, pluginId: 'test.demo', level: 'info', message: 'dismiss' })
  store.dismiss(2)
  expect(pluginService.dismissNotification).toHaveBeenCalledWith(2)
  expect(store.notifications).toHaveLength(0)
})
it('leaves subscription lifecycle to the service and returns its disposer unchanged', async () => {
  const disposer = Object.assign(() => {}, { dismiss: vi.fn() })
  pluginService.subscribeNotifications.mockResolvedValue(disposer)
  const store = usePluginStore()
  await expect(store.subscribe()).resolves.toBe(disposer)
  expect(pluginService.subscribeNotifications).toHaveBeenCalledOnce()
  expect(typeof pluginService.subscribeNotifications.mock.calls[0][0]).toBe('function')
  expect(typeof pluginService.subscribeNotifications.mock.calls[0][1]).toBe('function')
})
it('refreshes status only after a lifecycle operation completes', async () => {
  pluginService.list.mockResolvedValue([{ id: 'test.demo', state: 'running', error: null }])
  const store = usePluginStore(); await store.start('test.demo')
  expect(pluginService.start).toHaveBeenCalledWith('test.demo')
  expect(store.plugins[0].state).toBe('running')
})
