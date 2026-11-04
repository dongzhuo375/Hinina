// @vitest-environment jsdom
// 外链拦截（`installExternalLinkOpener`）的判定用例。
//
// 生产装配在 main.ts（document 级委托，opener 注入 systemService.openExternal
// → Rust `open_external`，scheme 白名单在 Rust 侧守卫）。本文件锁定拦截规则：
// 什么该转交、什么不该、转交的 URL 形态，以及 detach。
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installExternalLinkOpener } from '@/utils/external-links'

describe('installExternalLinkOpener — 外链拦截', () => {
  let opener: ReturnType<typeof vi.fn<(url: string) => void>>
  let detach: () => void

  beforeEach(() => {
    document.body.innerHTML = ''
    opener = vi.fn()
    detach = installExternalLinkOpener(opener, document)
  })

  afterEach(() => {
    detach()
    document.body.innerHTML = ''
  })

  /** 写入一段含锚点的 HTML 并左键点击第一个 `<a>`。 */
  function clickAnchor(html: string, init: MouseEventInit = {}): void {
    document.body.innerHTML = html
    const a = document.body.querySelector('a')
    if (!a) throw new Error('fixture 缺少 <a>')
    a.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, button: 0, ...init }))
  }

  it('http(s) 外链转交 opener（preventDefault + 绝对 URL）', () => {
    clickAnchor('<a href="https://errata.example.com/a1?t=1">勘误</a>')
    expect(opener).toHaveBeenCalledTimes(1)
    expect(opener).toHaveBeenCalledWith('https://errata.example.com/a1?t=1')
  })

  it('协议相对地址 //host 解析为 https 外链后转交', () => {
    clickAnchor('<a href="//errata.example.com/a1">勘误</a>')
    expect(opener).toHaveBeenCalledTimes(1)
    expect(opener).toHaveBeenCalledWith('http://errata.example.com/a1')
  })

  it('锚点内层元素触发（closest 委托）同样转交', () => {
    clickAnchor('<a href="https://errata.example.com/a1"><span><b>勘误</b></span></a>')
    expect(opener).toHaveBeenCalledTimes(1)
  })

  it('同 host（应用内）不转交——RouterLink 与站内锚点不受影响', () => {
    clickAnchor('<a href="/contest/problems">题目列表</a>')
    clickAnchor('<a href="http://localhost:3000/in-app">应用内绝对地址</a>')
    clickAnchor('<a href="#section">页内片段</a>')
    expect(opener).not.toHaveBeenCalled()
  })

  it('非 http(s) 协议不转交（scheme 白名单的最后一道在 Rust 命令）', () => {
    clickAnchor('<a href="javascript:void(0)">触发器</a>')
    clickAnchor('<a href="ftp://example.com/file">FTP</a>')
    expect(opener).not.toHaveBeenCalled()
  })

  it('修饰键点击不转交（浏览器默认行为交给默认策略）', () => {
    clickAnchor('<a href="https://errata.example.com/a1">勘误</a>', { ctrlKey: true })
    clickAnchor('<a href="https://errata.example.com/a1">勘误</a>', { metaKey: true })
    clickAnchor('<a href="https://errata.example.com/a1">勘误</a>', { shiftKey: true })
    clickAnchor('<a href="https://errata.example.com/a1">勘误</a>', { altKey: true })
    clickAnchor('<a href="https://errata.example.com/a1">勘误</a>', { button: 2 })
    expect(opener).not.toHaveBeenCalled()
  })

  it('已被前置处理器拦下的点击不重复转交', () => {
    document.body.innerHTML = '<a href="https://errata.example.com/a1">勘误</a>'
    const a = document.body.querySelector('a')!
    a.addEventListener('click', (e) => e.preventDefault())
    a.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, button: 0 }))
    expect(opener).not.toHaveBeenCalled()
  })

  it('点击非锚点区域不转交', () => {
    document.body.innerHTML = '<div class="prose">纯文本</div>'
    document.body
      .querySelector('div')!
      .dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, button: 0 }))
    expect(opener).not.toHaveBeenCalled()
  })

  it('detach 后不再拦截', () => {
    detach()
    clickAnchor('<a href="https://errata.example.com/a1">勘误</a>')
    expect(opener).not.toHaveBeenCalled()
  })
})
