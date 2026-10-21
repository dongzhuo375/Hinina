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
it('refreshes status only after a lifecycle operation completes', async () => {
  pluginService.list.mockResolvedValue([{ id: 'test.demo', state: 'running', error: null }])
  const store = usePluginStore(); await store.start('test.demo')
  expect(pluginService.start).toHaveBeenCalledWith('test.demo')
  expect(store.plugins[0].state).toBe('running')
})
