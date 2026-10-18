import type { ObjectDirective } from 'vue'

const cleanups = new WeakMap<HTMLElement, () => void>()

/** A mounted popover owns keyboard focus until it closes. No listeners survive unmount. */
export const vFocusScope: ObjectDirective<HTMLElement, () => void> = {
  mounted(el, binding) {
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null
    const candidates = () => Array.from(el.querySelectorAll<HTMLElement>(
      'button:not(:disabled), a[href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex="0"]',
    )).filter(node => !node.hidden && node.getAttribute('aria-hidden') !== 'true' && node.getClientRects().length > 0)
    el.tabIndex = -1
    const focus = () => (candidates()[0] ?? el).focus()
    const keydown = (event: KeyboardEvent) => {
      // Monaco and nested popovers own their handled keystrokes (notably Tab indentation).
      if (event.defaultPrevented) return
      if (event.key === 'Escape') {
        event.preventDefault()
        event.stopPropagation()
        binding.value()
      } else if (event.key === 'Tab') {
        const items = candidates()
        const index = items.indexOf(document.activeElement as HTMLElement)
        if (items.length === 0) { event.preventDefault(); el.focus(); return }
        if (event.shiftKey && index <= 0) { event.preventDefault(); items.at(-1)!.focus() }
        else if (!event.shiftKey && (index === items.length - 1 || index < 0)) { event.preventDefault(); items[0].focus() }
      }
    }
    el.addEventListener('keydown', keydown)
    queueMicrotask(() => { if (el.isConnected) focus() })
    cleanups.set(el, () => {
      el.removeEventListener('keydown', keydown)
      if (previous?.isConnected) previous.focus()
    })
  },
  unmounted(el) { cleanups.get(el)?.(); cleanups.delete(el) },
}
