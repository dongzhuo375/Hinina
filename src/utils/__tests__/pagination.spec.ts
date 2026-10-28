import { describe, expect, it } from 'vitest'
import { getPageItems } from '@/utils/pagination'

describe('getPageItems', () => {
  it('单页或无页时不渲染页码窗口', () => {
    expect(getPageItems(1, 0)).toEqual([])
    expect(getPageItems(1, 1)).toEqual([])
  })

  it('不超过 7 页时全部展开，且不额外钳制 current', () => {
    expect(getPageItems(1, 2)).toEqual([1, 2])
    expect(getPageItems(99, 7)).toEqual([1, 2, 3, 4, 5, 6, 7])
  })

  it('首页附近保留前四页与尾页，不重复插入省略号', () => {
    expect(getPageItems(1, 8)).toEqual([1, 2, 3, 4, '…', 8])
    expect(getPageItems(3, 10)).toEqual([1, 2, 3, 4, '…', 10])
  })

  it('中间页保留首尾、当前页与左右邻居', () => {
    expect(getPageItems(4, 10)).toEqual([1, '…', 3, 4, 5, '…', 10])
    expect(getPageItems(5, 10)).toEqual([1, '…', 4, 5, 6, '…', 10])
  })

  it('末页附近保留首页与末三页', () => {
    expect(getPageItems(8, 10)).toEqual([1, '…', 7, 8, 9, 10])
    expect(getPageItems(10, 10)).toEqual([1, '…', 7, 8, 9, 10])
  })

  it('异常 current 沿用原算法的最终越界过滤，不做额外钳制', () => {
    expect(getPageItems(0, 10)).toEqual([1, 2, 3, 4, '…', 10])
    expect(getPageItems(99, 10)).toEqual([1, '…', 7, 8, 9, 10])
  })
})
