// @vitest-environment jsdom
import { createApp, nextTick } from 'vue'
import { describe, expect, it } from 'vitest'
import Countdown from '@/components/login/Countdown.vue'

describe('Countdown', () => {
  it('uses the connection guidance as its timer label when the schedule is unavailable', async () => {
    const host = document.createElement('div')
    const app = createApp(Countdown, {
      phase: 'none',
      startTime: null,
      endTime: null,
      now: 1_000,
      isLoggedIn: false,
    })

    app.mount(host)
    await nextTick()

    expect(host.querySelector('[role="timer"]')?.getAttribute('aria-label')).toBe('赛程信息连接后显示')

    app.unmount()
  })
})
