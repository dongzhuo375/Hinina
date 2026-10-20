// @vitest-environment jsdom
import { beforeEach, afterEach, it, expect, vi } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'
import type { PluginNotification } from '@/types/plugin'
const { pluginService } = vi.hoisted(() => ({ pluginService: {
  list: vi.fn(), start: vi.fn(), stop: vi.fn(), reload: vi.fn(), resync: vi.fn(), onNotification: vi.fn(),
} }))
vi.mock('@/services/plugin.service', () => ({ pluginService }))
import { usePluginStore } from '@/stores/pluginStore'
beforeEach(() => { setActivePinia(createPinia()); vi.clearAllMocks(); vi.useFakeTimers() })
afterEach(() => vi.useRealTimers())
it('keeps notifications as literal text and bounds the queue and lifetime', async () => {
  let receive!: (value: PluginNotification) => void
  pluginService.onNotification.mockImplementation(handler => { receive = handler; return Promise.resolve(() => {}) })
  const store = usePluginStore(); await store.subscribe()
  for (let n = 0; n < 10; n++) receive({ pluginId: 'test.demo', level: 'info', message: '<script>alert(1)</script>' })
  expect(store.notifications).toHaveLength(5)
  expect(store.notifications[0].message).toBe('<script>alert(1)</script>')
  receive({ pluginId: 'test.demo', level: 'info', message: 'x'.repeat(513) })
  expect(store.notifications).toHaveLength(5)
  vi.advanceTimersByTime(6000)
  expect(store.notifications).toHaveLength(0)
})
it('refreshes status only after a lifecycle operation completes', async () => {
  pluginService.list.mockResolvedValue([{ id: 'test.demo', state: 'running', error: null }])
  const store = usePluginStore(); await store.start('test.demo')
  expect(pluginService.start).toHaveBeenCalledWith('test.demo')
  expect(store.plugins[0].state).toBe('running')
})
