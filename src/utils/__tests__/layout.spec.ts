import { describe, expect, it } from 'vitest'

import {
  DEFAULT_SPLIT_RATIO,
  SPLIT_RATIO_MAX,
  SPLIT_RATIO_MIN,
  clampSplitRatio,
} from '@/utils/layout'

describe('clampSplitRatio — 分栏比例值域归一（单源）', () => {
  it('带内值原样透传（含边界恒等）', () => {
    expect(clampSplitRatio(0.55)).toBe(0.55)
    expect(clampSplitRatio(SPLIT_RATIO_MIN)).toBe(0.3)
    expect(clampSplitRatio(SPLIT_RATIO_MAX)).toBe(0.7)
  })

  it('有限越界值钳到边界（保留方向，与 Rust sanitize 一致）', () => {
    // 手改 config.json 写 0.25：旧口径（0.2–0.8 透传带）会原样放行，
    // 解题页首次拖拽即跳变入 [0.3, 0.7]
    expect(clampSplitRatio(0.25)).toBe(0.3)
    expect(clampSplitRatio(0.05)).toBe(0.3)
    expect(clampSplitRatio(0.95)).toBe(0.7)
    expect(clampSplitRatio(5)).toBe(0.7)
    expect(clampSplitRatio(-1)).toBe(0.3)
  })

  it('非数字 / 非有限值回退默认 0.48（镜像 Rust NaN 回退）', () => {
    expect(clampSplitRatio(undefined)).toBe(DEFAULT_SPLIT_RATIO)
    expect(clampSplitRatio(Number.NaN)).toBe(DEFAULT_SPLIT_RATIO)
    expect(clampSplitRatio(Number.POSITIVE_INFINITY)).toBe(DEFAULT_SPLIT_RATIO)
    expect(clampSplitRatio('0.5')).toBe(DEFAULT_SPLIT_RATIO)
  })
})
