# PagePager（极简分页器）

> 源文件：`src/components/common/PagePager.vue`

## 职责

评测页与榜单页共用的分页控件。纯展示 + 事件上报：不持有页码窗口算法（`items` 由调用方计算注入），不直接触 store，翻页意图经 `go` 事件交给父级。

设计语言（苏博式极简）：靠留白与线条分割，不画框 —— 全部按钮**无边框、无底色**；「上一页 / 下一页」纯文字，深灰（`--text-secondary`）悬停转近黑（`--text-primary`）；页码为等宽纯文本，**当前页加粗 + 1px 木色（`#8c6a43`）下划线**（仪表盘指针的隐喻，与页头编号气质一致）；`focus-visible` 用木色 1px 外描边保证键盘可见性。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `current` / `pages` | prop `number` | 当前页（1 起）/ 总页数；决定「上一页 / 下一页」的禁用态 |
| `items` | prop `readonly (number \| '…')[]` | 页码窗口（首尾页 + 当前页 ±1，间隙为 `'…'`），由调用方的 `pageItems` computed 提供 |
| `loading` | prop `boolean`（可选） | 翻页请求在途时禁用全部按钮，防止连点并发 |
| `go` | emit `(page: number)` | 翻页意图（上一页 / 页码 / 下一页统一出口） |

当前页按钮带 `aria-current="page"`；根元素为 `<nav aria-label="分页">`。

## 直接依赖

- `vue`（`defineProps` / `defineEmits`）

## 被依赖

- `views/SubmissionsView.vue` — 评测页底栏（`history.pages > 1` 时渲染，`@go="goToPage"`）
- `views/RankView.vue` — 榜单页底栏（与 `totalLabel` 并列，`@go="goToPage"`）

## 逻辑流程

```
点击「上一页/下一页」→ 边界判定（current <= 1 / current >= pages）或 loading → disabled 不触发
                      否则 emit('go', current ± 1)
点击页码 item        → loading 时禁用；否则 emit('go', item)（父级 store 内部对同页点击自判 no-op）
```

设计要点：

- **样式集中在 scoped CSS 而非 Tailwind 链**：分页器的视觉规范（无框、悬停转黑、当前页木色下划线）是一组内聚规则，写在 `.pager-nav` / `.pager-page` 两条类里比散在模板原子类中更易维护；`#8c6a43` 木色与全站 hover 棕同源。
- **`items` 注入而非内部计算**：两个调用方各自持有「页码窗口折叠」computed（算法相同但数据源不同），组件保持无状态，便于复用与测试。
- 非当前页**不要任何背景与边框**（含 hover）—— 曾经用「主题色实心方块 + 灰框邻居」的传统 SaaS 分页，在浅灰白页面上视觉噪音过重，已按极简语言收敛为纯文本。
