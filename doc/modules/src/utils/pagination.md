# pagination（页码窗口纯函数）

> 源文件：`src/utils/pagination.ts`

## 职责

为榜单页与提交历史页提供同一套页码窗口算法：保留首页、尾页、当前页及有效邻居，其余间隙用 `'…'` 折叠。只计算展示数组，不持有分页状态，也不决定翻页行为。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `getPageItems` | `(current: number, pages: number) => (number \| '…')[]` | `pages <= 1` 返回空数组；`pages <= 7` 全展开；更多页时生成首尾与当前页窗口，并在间隙大于 1 时插入省略号 |

边界语义与原两处视图内联实现完全一致：不主动钳制 `current`，只在多页分支最终过滤越界页码；不对服务端异常页码额外纠偏。

## 直接依赖

无（纯 TypeScript 函数）。

## 被依赖

- `views/RankView.vue` — 传入 `rankStore.current/pages`
- `views/SubmissionsView.vue` — 传入 `history.current/pages`
- `utils/__tests__/pagination.spec.ts`

`components/common/PagePager.vue` 不直接依赖本模块，仍由调用方注入 `items`。

## 逻辑流程

1. 少页直接返回完整页码或空数组。
2. 用 `Set` 收集首尾、当前页及有效 `current ± 1`，首页/末页附近按原规则补页。
3. 过滤越界页码并数值排序。
4. 相邻页码间隙大于 1 时插入 `'…'`，返回供 `PagePager` 渲染的数组。

## 测试

`src/utils/__tests__/pagination.spec.ts` 锁定：单页/无页、少页全展开、首页附近、中间窗口、末页附近、省略号去重，以及异常 current 不额外钳制的现行边界。
