# focus-scope（弹层焦点作用域指令）

> 源文件：`src/utils/focus-scope.ts`

## 职责

导出 Vue 自定义指令 `vFocusScope`：弹层挂载时把焦点移入作用域内首个可见可聚焦控件、在作用域内循环 Tab（焦点陷阱）、Esc 触发绑定回调关闭、卸载时把焦点归还给打开者。让所有弹层用一条指令统一获得键盘可访问性。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `cleanups` | `WeakMap<HTMLElement, () => void>` | 每个元素保存卸载清理函数；元素被 GC 时自动释放，无监听器泄漏 |
| `vFocusScope` | `ObjectDirective<HTMLElement, () => void>` | 指令本体；`binding.value` 是关闭回调（无参） |

`mounted` 内部：

| 步骤 | 说明 |
|------|------|
| 记录 `previous` | `document.activeElement`（若是 HTMLElement），卸载时归还 |
| `candidates()` | 查询 `'button:not(:disabled), a[href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex="0"]'`，再过滤 `!hidden && aria-hidden !== 'true' && getClientRects().length > 0` —— 只保留**真正可见**的控件 |
| `el.tabIndex = -1` | 容器本身可编程聚焦但不在 Tab 序列里 |
| `focus()` | `(candidates()[0] ?? el).focus()` —— 无候选时聚焦容器自身 |
| `keydown` | 见逻辑流程 |
| `queueMicrotask(focus)` | 挂载后微任务内聚焦（避免与父级渲染冲突，且 `el.isConnected` 才执行） |

## 直接依赖

- `vue`（仅类型 `ObjectDirective`）

## 被依赖

- `components/editor/CodeEditor.vue` — 语言下拉浮层、清空代码二次确认面板
- `components/editor/EditorSettingsPopover.vue` — 编辑器设置面板
- `components/problem/QuickSubmitDialog.vue` — 快捷提交对话框
- `components/layout/TopBar.vue` — 比赛简介浮层、账号菜单

## 逻辑流程

```
mounted:
  记录 previous 焦点
  定义 candidates()（可见性过滤）
  el.tabIndex = -1
  监听 keydown:
    event.defaultPrevented → return
      // Monaco 与嵌套弹层已处理的按键（尤其 Tab 缩进）不再拦截
    Escape → preventDefault + stopPropagation → binding.value()
    Tab    → items = candidates()
             items 为空 → 阻止默认并聚焦容器
             shiftKey 且 index <= 0            → 聚焦 items.at(-1)（首 → 尾）
             !shiftKey 且 index 为末位或 < 0   → 聚焦 items[0]（尾 → 首）
  queueMicrotask → el.isConnected 时 focus()

unmounted:
  执行 cleanup：移除 keydown 监听；previous?.isConnected 时 previous.focus()（归还焦点）
```

设计要点：

- **`defaultPrevented` 短路是必须的**：Monaco 用 Tab 做缩进、嵌套弹层可能已处理按键；
  若无条件拦截，编辑器里按 Tab 会跳焦点而不是缩进，直接破坏写代码。
- **可见性用 `getClientRects().length > 0` 而非 CSS 类名判断**：条件渲染（`v-if`）、
  `hidden` 属性、被父级裁切的情形都能覆盖；`aria-hidden` 的装饰性元素也一并排除。
- **焦点必须归还**：弹层关闭后焦点回到打开它的按钮，键盘用户不必用鼠标找回位置；
  `isConnected` 判断避免元素已随弹层一起卸载时聚焦到游离节点。
- **`WeakMap` 保存清理函数**：指令可能挂在被 `v-if` 反复创建销毁的节点上，
  用 WeakMap 保证不残留监听器（也不会阻止元素被回收）。
- 作用域内无候选控件时聚焦容器自身：至少保证 Esc 关闭仍可被接收。

## 测试

`src/utils/__tests__/focus-scope.spec.ts`（jsdom，mock `getClientRects` 模拟可见性）锁定：初始焦点跳过隐藏控件选首个可见控件；打开即移入首个控件、Tab 到末尾回卷到首、Shift+Tab 到首位回到尾、Esc 调用关闭回调；`defaultPrevented` 的按键（编辑器 Tab 缩进）不被拦截且焦点不动；关闭/卸载后焦点归还给打开者。
