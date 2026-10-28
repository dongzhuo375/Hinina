/**
 * 页码窗口：首尾页 + 当前页 ±1，其余折叠成省略号。
 *
 * 保留调用方原有的边界语义：current 不做钳制；pages 较多时仅在最终排序前
 * 过滤越界页码，避免抽取共享函数时悄悄改变服务端异常页码的展示行为。
 */
export function getPageItems(current: number, pages: number): (number | '…')[] {
  if (pages <= 1) return []
  if (pages <= 7) return Array.from({ length: pages }, (_, i) => i + 1)
  const nums = new Set<number>([1, pages, current])
  for (const delta of [-1, 1]) {
    const candidate = current + delta
    if (candidate > 1 && candidate < pages) nums.add(candidate)
  }
  if (current <= 3) for (const n of [2, 3, 4]) nums.add(n)
  if (current >= pages - 2) for (const n of [pages - 3, pages - 2, pages - 1]) nums.add(n)
  const sorted = [...nums].filter((n) => n >= 1 && n <= pages).sort((a, b) => a - b)
  const items: (number | '…')[] = []
  let previous = 0
  for (const n of sorted) {
    if (previous !== 0 && n - previous > 1) items.push('…')
    items.push(n)
    previous = n
  }
  return items
}
