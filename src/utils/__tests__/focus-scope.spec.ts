// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, h, nextTick, ref, withDirectives } from 'vue'
import type { App, VNode } from 'vue'
import { vFocusScope } from '@/utils/focus-scope'

let app: App | undefined
let host: HTMLDivElement

function render(content: () => VNode) {
  host = document.createElement('div')
  document.body.append(host)
  app = createApp({ render: content })
  app.mount(host)
}

async function flushFocus() {
  await nextTick()
  await Promise.resolve()
}

beforeEach(() => {
  // jsdom has no layout; model rendered controls without changing production visibility checks.
  vi.spyOn(HTMLElement.prototype, 'getClientRects').mockImplementation(function (this: HTMLElement) {
    return (this.closest('[hidden], [style*="display: none"]') ? [] : [{}]) as unknown as DOMRectList
  })
})

afterEach(() => {
  app?.unmount()
  app = undefined
  host?.remove()
  vi.restoreAllMocks()
})

describe('popover focus scope', () => {
  it('skips hidden controls when choosing the initial focus target', async () => {
    render(() => withDirectives(h('section', [
      h('div', { style: 'display: none' }, [h('button', 'hidden')]),
      h('button', { id: 'visible' }, 'visible'),
    ]), [[vFocusScope, vi.fn()]]))
    await flushFocus()
    expect(document.activeElement).toBe(host.querySelector('#visible'))
  })

  it('moves focus inside on open, traps Tab and restores focus on Escape', async () => {
    const close = vi.fn()
    render(() => withDirectives(h('section', { 'aria-label': 'menu' }, [
      h('button', { id: 'first' }, 'first'),
      h('button', { id: 'last' }, 'last'),
    ]), [[vFocusScope, close]]))
    await flushFocus()

    const first = host.querySelector<HTMLButtonElement>('#first')!
    const last = host.querySelector<HTMLButtonElement>('#last')!
    expect(document.activeElement).toBe(first)

    last.focus()
    last.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true }))
    expect(document.activeElement).toBe(first)
    first.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', shiftKey: true, bubbles: true, cancelable: true }))
    expect(document.activeElement).toBe(last)

    first.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }))
    expect(close).toHaveBeenCalledOnce()
  })

  it('leaves handled editor keystrokes alone and returns focus to its opener on close', async () => {
    const visible = ref(true)
    render(() => {
      const opener = h('button', { id: 'opener' }, 'open')
      const scope = visible.value
        ? withDirectives(h('section', [h('button', { id: 'inside' }, 'inside')]), [[vFocusScope, () => { visible.value = false }]])
        : null
      return h('div', [opener, scope])
    })
    await flushFocus()
    const opener = host.querySelector<HTMLButtonElement>('#opener')!
    const inside = host.querySelector<HTMLButtonElement>('#inside')!
    opener.focus()
    // Re-open after the opener has focus, matching the real button -> popover flow.
    visible.value = false
    await nextTick()
    visible.value = true
    await flushFocus()
    const reopened = host.querySelector<HTMLButtonElement>('#inside')!

    const editorHandled = new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true })
    editorHandled.preventDefault()
    reopened.dispatchEvent(editorHandled)
    expect(document.activeElement).toBe(reopened)

    reopened.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }))
    await nextTick()
    expect(document.activeElement).toBe(opener)
  })
})
