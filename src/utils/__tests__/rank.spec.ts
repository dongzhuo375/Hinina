import { describe, expect, it } from 'vitest'
import {
  dedupeRankRows,
  filterRankRowsByGroup,
  formatPenaltyMinutes,
  formatRankTime,
  mergeRankPages,
  paginateRankRows,
  resolveDisplayName,
  resolveMyRow,
  resolveOiRankCell,
  resolveParticipantCount,
  resolveParticipantCountFromPage,
  resolveRankCell,
  resolveRowKind,
} from '@/utils/rank'
import type { ContestRankPage, ContestRankRow, RankCell } from '@/types/rank'

/// 构造榜单行，测试只关心被测字段，其余给合法默认值
function makeRow(overrides: Partial<ContestRankRow> = {}): ContestRankRow {
  return {
    rank: 1,
    uid: 'uid-1',
    username: 'alice',
    realname: '张三',
    nickname: 'Alice',
    school: 'XX大学',
    gender: 'male',
    avatar: '',
    ac: 0,
    total: 0,
    totalTime: 0,
    totalScore: null,
    submissionInfo: {},
    timeInfo: {},
    ...overrides,
  }
}

/// 构造单元格，默认「无任何记录」的空态
function makeCell(overrides: Partial<RankCell> = {}): RankCell {
  return {
    errorNum: 0,
    tryNum: null,
    isAc: false,
    isFirstAc: false,
    acTime: null,
    isAfterContest: false,
    score: null,
    ...overrides,
  }
}

describe('resolveRankCell — 空态', () => {
  it('cell 为 undefined（该题无任何记录）→ none，文案均为 null（UI 显示 -）', () => {
    expect(resolveRankCell(undefined)).toEqual({
      kind: 'none',
      timeText: null,
      triesText: null,
    })
  })

  it('errorNum=0 且未 AC 无 tryNum → none', () => {
    expect(resolveRankCell(makeCell())).toEqual({
      kind: 'none',
      timeText: null,
      triesText: null,
    })
  })
})

describe('resolveRankCell — AC 行', () => {
  it('普通 AC：kind=ac，timeText 为格式化 acTime', () => {
    const cell = makeCell({ errorNum: 1, isAc: true, acTime: 600 })
    expect(resolveRankCell(cell)).toEqual({
      kind: 'ac',
      timeText: '10:00',
      triesText: '2 试',
    })
  })

  it('回归：AC 行尝试数为 errorNum+1（HOJ 前端把本次 AC 计入）', () => {
    const cell = makeCell({ errorNum: 3, isAc: true, acTime: 60 })
    expect(resolveRankCell(cell).triesText).toBe('4 试')
  })

  it('一血：isFirstAc → kind=first-ac', () => {
    const cell = makeCell({ isAc: true, isFirstAc: true, acTime: 300 })
    expect(resolveRankCell(cell).kind).toBe('first-ac')
  })

  it('回归：isAfterContest 时 kind=after-ac 且 timeText 前加 *', () => {
    const cell = makeCell({ isAc: true, acTime: 3720, isAfterContest: true })
    expect(resolveRankCell(cell)).toEqual({
      kind: 'after-ac',
      timeText: '*1:02:00',
      triesText: '1 试',
    })
  })

  it('after-ac 优先级高于 first-ac（赛后一血仍标 *）', () => {
    const cell = makeCell({ isAc: true, isFirstAc: true, isAfterContest: true, acTime: 10 })
    const resolved = resolveRankCell(cell)
    expect(resolved.kind).toBe('after-ac')
    expect(resolved.timeText).toBe('*00:10')
  })

  it('AC 但 acTime 缺失（脏数据）→ 时间渲染 --:--，不崩溃', () => {
    expect(resolveRankCell(makeCell({ isAc: true })).timeText).toBe('--:--')
  })
})

describe('resolveRankCell — 封榜行', () => {
  it('回归：未 AC 且 tryNum != null → sealed，只显示 errorNum+tryNum tries', () => {
    const cell = makeCell({ errorNum: 2, tryNum: 3 })
    expect(resolveRankCell(cell)).toEqual({
      kind: 'sealed',
      timeText: null,
      triesText: '2+3 tries',
    })
  })

  it('封榜期间 errorNum=0 也按 sealed 渲染', () => {
    const cell = makeCell({ errorNum: 0, tryNum: 1 })
    expect(resolveRankCell(cell)).toEqual({
      kind: 'sealed',
      timeText: null,
      triesText: '0+1 tries',
    })
  })

  it('封榜行优先于 wa 行（封榜前失败 + 封榜后盲提交合并显示）', () => {
    expect(resolveRankCell(makeCell({ errorNum: 5, tryNum: 2 })).kind).toBe('sealed')
  })
})

describe('resolveRankCell — WA 行', () => {
  it('未 AC 且 errorNum > 0 → wa，triesText 为 -errorNum', () => {
    expect(resolveRankCell(makeCell({ errorNum: 2 }))).toEqual({
      kind: 'wa',
      timeText: null,
      triesText: '-2',
    })
  })
})

describe('formatRankTime', () => {
  it('不足 1 小时按 mm:ss（acTime 是相对开赛的进度秒数）', () => {
    expect(formatRankTime(0)).toBe('00:00')
    expect(formatRankTime(8)).toBe('00:08')
    expect(formatRankTime(480)).toBe('08:00')
    expect(formatRankTime(3599)).toBe('59:59')
  })

  it('超过 1 小时按 h:mm:ss', () => {
    expect(formatRankTime(3600)).toBe('1:00:00')
    expect(formatRankTime(3720)).toBe('1:02:00')
    expect(formatRankTime(5 * 3600 + 61)).toBe('5:01:01')
  })

  it('小数秒向下取整', () => {
    expect(formatRankTime(60.9)).toBe('01:00')
  })

  it('负数/NaN/Infinity 等非法值返回 --:--', () => {
    expect(formatRankTime(-1)).toBe('--:--')
    expect(formatRankTime(Number.NaN)).toBe('--:--')
    expect(formatRankTime(Number.POSITIVE_INFINITY)).toBe('--:--')
  })
})

describe('resolveDisplayName', () => {
  it('rankShowName=username/realname/nickname 各取对应字段', () => {
    const row = makeRow()
    expect(resolveDisplayName(row, 'username')).toBe('alice')
    expect(resolveDisplayName(row, 'realname')).toBe('张三')
    expect(resolveDisplayName(row, 'nickname')).toBe('Alice')
  })

  it('目标字段为空时回退 username', () => {
    const row = makeRow({ realname: '', nickname: '  ' })
    expect(resolveDisplayName(row, 'realname')).toBe('alice')
    expect(resolveDisplayName(row, 'nickname')).toBe('alice')
  })

  it('username 也为空时回退 uid', () => {
    const row = makeRow({ realname: '', username: '', uid: 'uid-42' })
    expect(resolveDisplayName(row, 'realname')).toBe('uid-42')
  })

  it('未知 rankShowName 按 username 处理', () => {
    expect(resolveDisplayName(makeRow(), 'school')).toBe('alice')
  })
})

describe('resolveRowKind', () => {
  it('回归：rank=-1 为打星队伍', () => {
    expect(resolveRowKind(makeRow({ rank: -1 }))).toBe('star')
  })

  it('star 优先级最高：打星且 female 仍按 star 渲染', () => {
    expect(resolveRowKind(makeRow({ rank: -1, gender: 'female' }))).toBe('star')
  })

  it('gender=female → female（前端加背景色）', () => {
    expect(resolveRowKind(makeRow({ gender: 'female' }))).toBe('female')
  })

  it('其余 → normal', () => {
    expect(resolveRowKind(makeRow())).toBe('normal')
  })
})

describe('dedupeRankRows', () => {
  it('回归：服务端把当前用户/关注用户前置复制一份，第 1 页按 uid 去重保留首次出现', () => {
    const me = makeRow({ uid: 'me', rank: 7, username: 'me' })
    const first = makeRow({ uid: 'u1', rank: 1 })
    const second = makeRow({ uid: 'u2', rank: 2 })
    // HOJ 前置的重复行内容与原行相同，此处以同 uid 不同对象模拟
    const rows = [me, first, second, { ...me }, { ...first }]
    const deduped = dedupeRankRows(rows)
    expect(deduped).toHaveLength(3)
    expect(deduped.map((r) => r.uid)).toEqual(['me', 'u1', 'u2'])
    // 保留的是首次出现的那个对象
    expect(deduped[0]).toBe(me)
  })

  it('无重复时保持原顺序；空数组返回空', () => {
    const rows = [makeRow({ uid: 'b' }), makeRow({ uid: 'a' })]
    expect(dedupeRankRows(rows).map((r) => r.uid)).toEqual(['b', 'a'])
    expect(dedupeRankRows([])).toEqual([])
  })

  it('不修改入参数组', () => {
    const rows = [makeRow({ uid: 'a' }), makeRow({ uid: 'a' })]
    dedupeRankRows(rows)
    expect(rows).toHaveLength(2)
  })
})

describe('resolveMyRow', () => {
  const rows = [makeRow({ uid: 'u1' }), makeRow({ uid: 'u2' })]

  it('按 uid 定位我的行', () => {
    expect(resolveMyRow(rows, 'u2')?.uid).toBe('u2')
  })

  it('未找到 / uid 为 null / uid 为空串 → null', () => {
    expect(resolveMyRow(rows, 'nobody')).toBeNull()
    expect(resolveMyRow(rows, null)).toBeNull()
    expect(resolveMyRow(rows, '')).toBeNull()
  })
})

describe('resolveParticipantCount', () => {
  it('回归：不用分页 total（含前置重复条目而偏大），取非打星行最大 rank', () => {
    const rows = [
      makeRow({ uid: 'u1', rank: 1 }),
      makeRow({ uid: 'u2', rank: 2 }),
      makeRow({ uid: 'u3', rank: 3 }),
    ]
    // 服务端 total 因前置复制「我」与关注用户而虚高
    expect(resolveParticipantCount(rows, 120)).toBe(3)
  })

  it('打星行 rank=-1 不计入参与人数', () => {
    const rows = [
      makeRow({ uid: 'u1', rank: 1 }),
      makeRow({ uid: 'star', rank: -1 }),
      makeRow({ uid: 'u2', rank: 2 }),
    ]
    expect(resolveParticipantCount(rows, 99)).toBe(2)
  })

  it('前置重复行不影响结果（内部先去重）', () => {
    const rows = [makeRow({ uid: 'u1', rank: 5 }), makeRow({ uid: 'u1', rank: 5 })]
    expect(resolveParticipantCount(rows, 50)).toBe(5)
  })

  it('空榜单或整页打星时才回退 total', () => {
    expect(resolveParticipantCount([], 88)).toBe(88)
    expect(resolveParticipantCount([makeRow({ rank: -1 })], 88)).toBe(88)
  })
})

describe('formatPenaltyMinutes', () => {
  it('ACM 总罚时秒 → 分钟整数字符串（向下取整）', () => {
    expect(formatPenaltyMinutes(2700)).toBe('45')
    expect(formatPenaltyMinutes(0)).toBe('0')
    expect(formatPenaltyMinutes(59)).toBe('0')
    expect(formatPenaltyMinutes(1820)).toBe('30')
  })

  it('非法/负值返回 --', () => {
    expect(formatPenaltyMinutes(-1)).toBe('--')
    expect(formatPenaltyMinutes(Number.NaN)).toBe('--')
    expect(formatPenaltyMinutes(Number.POSITIVE_INFINITY)).toBe('--')
  })
})

describe('resolveParticipantCountFromPage', () => {
  /// 构造一页榜单：rows 为去重前的原始 records
  const page = (
    rows: ContestRankRow[],
    total: number,
    current = 1,
    size = 50,
  ): ContestRankPage => ({
    records: rows,
    total,
    size,
    current,
    pages: Math.ceil(total / size),
  })

  // 口径修正说明：旧口径只做「total − 本页重复数」，当「我」的自然名次不在本页时
  // 前置副本不构成重复，分母会偏大 1。新口径额外识别「单条出现且 rank 不在本页
  // 名次窗口内」的前置副本并再减 1（含打星 myRow，rank=-1 无自然位置）。

  it('用 total 减去本页重复行数修正参与人数（服务端会前置复制当前用户）', () => {
    // total=361 含 1 条前置副本；本页 me 出现两次（副本 + 自然行）→ 修正为 360
    const me = makeRow({ uid: 'me', rank: 42 })
    const rows = [me, makeRow({ uid: 'u1', rank: 1 }), me]
    expect(resolveParticipantCountFromPage(page(rows, 361), 'me')).toBe(360)
  })

  it('本页无重复且无 myRow 时直接采用 total', () => {
    const rows = [makeRow({ uid: 'u1', rank: 1 }), makeRow({ uid: 'u2', rank: 2 })]
    expect(resolveParticipantCountFromPage(page(rows, 120), null)).toBe(120)
    expect(resolveParticipantCountFromPage(page(rows, 120), 'nobody')).toBe(120)
  })

  it('不会退化成「本页最大 rank」（第 1 页 limit=50 时那只有 50）', () => {
    const rows = Array.from({ length: 50 }, (_, i) => makeRow({ uid: `u${i}`, rank: i + 1 }))
    expect(resolveParticipantCountFromPage(page(rows, 360), null)).toBe(360)
  })

  it('total 缺失时退回最大 rank 口径', () => {
    const rows = [makeRow({ uid: 'u1', rank: 7 }), makeRow({ uid: 'star', rank: -1 })]
    expect(resolveParticipantCountFromPage(page(rows, 0), null)).toBe(7)
  })

  it('我的前置副本在本页而自然名次在别页 → 额外减 1（修正旧口径分母偏大）', () => {
    // 第 1 页窗口 [1,50]，me rank=300 只出现一次 → 该条是前置副本，361-0-1=360
    const rows = [makeRow({ uid: 'me', rank: 300 }), makeRow({ uid: 'u1', rank: 1 })]
    expect(resolveParticipantCountFromPage(page(rows, 361), 'me')).toBe(360)
  })

  it('打星 myRow（rank=-1，自然位置无定义）→ 额外减 1', () => {
    const rows = [makeRow({ uid: 'me', rank: -1 }), makeRow({ uid: 'u1', rank: 1 })]
    expect(resolveParticipantCountFromPage(page(rows, 101), 'me')).toBe(100)
  })

  it('myUid 单条出现且 rank 恰在本页名次窗口内 → 视为自然行，不额外减', () => {
    // 第 2 页窗口 [51,100]，me rank=60 只出现一次 → 服务端未前置副本，total 未因我偏大
    const rows = [makeRow({ uid: 'me', rank: 60 }), makeRow({ uid: 'u1', rank: 51 })]
    expect(resolveParticipantCountFromPage(page(rows, 120, 2), 'me')).toBe(120)
  })

  it('窗口边界：rank 恰为窗口末位不额外减，越出 1 位则减', () => {
    // 第 1 页窗口 [1,50]
    const inWindow = [makeRow({ uid: 'me', rank: 50 }), makeRow({ uid: 'u1', rank: 1 })]
    expect(resolveParticipantCountFromPage(page(inWindow, 120), 'me')).toBe(120)
    const outWindow = [makeRow({ uid: 'me', rank: 51 }), makeRow({ uid: 'u1', rank: 1 })]
    expect(resolveParticipantCountFromPage(page(outWindow, 121), 'me')).toBe(120)
  })

  it('多关注用户前置副本：同页重复被去重计数捕获，不重复扣减', () => {
    // me 与 follow 各被前置复制一份且自然行也在本页：duplicates=2 → 362-2=360
    const me = makeRow({ uid: 'me', rank: 42 })
    const follow = makeRow({ uid: 'follow', rank: 43 })
    const rows = [me, follow, makeRow({ uid: 'u1', rank: 1 }), me, follow]
    expect(resolveParticipantCountFromPage(page(rows, 362), 'me')).toBe(360)
  })

  it('多关注用户前置副本但自然行不在本页：me 减 1，follow 的副本无法识别（口径只修「我」）', () => {
    // me(rank 300) 与 follow(rank 301) 各单条出现（都是前置副本），duplicates=0
    // 口径只对 myUid 修正 → 362-1=361（follow 的副本是已知残余误差，文档口径如此）
    const rows = [
      makeRow({ uid: 'me', rank: 300 }),
      makeRow({ uid: 'follow', rank: 301 }),
      makeRow({ uid: 'u1', rank: 1 }),
    ]
    expect(resolveParticipantCountFromPage(page(rows, 362), 'me')).toBe(361)
  })
})

// ── 全量快照模式纯函数 ──

describe('mergeRankPages', () => {
  it('跨页按 uid 去重合并：已存在的行保留，新行按序追加', () => {
    // 服务端在每页都前置复制当前用户 → 页与页之间存在同 uid 重复行
    const me = makeRow({ uid: 'me', rank: 42 })
    const existing = [me, makeRow({ uid: 'u1', rank: 1 })]
    const incoming = [makeRow({ uid: 'me', rank: 42 }), makeRow({ uid: 'u2', rank: 2 })]
    const merged = mergeRankPages(existing, incoming)
    expect(merged.map((r) => r.uid)).toEqual(['me', 'u1', 'u2'])
  })

  it('重复时保留已存在的对象（首次出现优先，与 dedupeRankRows 一致）', () => {
    const first = makeRow({ uid: 'u1', ac: 1 })
    const second = makeRow({ uid: 'u1', ac: 9 })
    const merged = mergeRankPages([first], [second])
    expect(merged).toHaveLength(1)
    expect(merged[0]).toBe(first)
  })

  it('incoming 内部重复也只保留一条', () => {
    const merged = mergeRankPages([], [
      makeRow({ uid: 'a' }),
      makeRow({ uid: 'a' }),
      makeRow({ uid: 'b' }),
    ])
    expect(merged.map((r) => r.uid)).toEqual(['a', 'b'])
  })

  it('不修改入参数组；空入参边界', () => {
    const existing = [makeRow({ uid: 'a' })]
    const incoming = [makeRow({ uid: 'b' })]
    mergeRankPages(existing, incoming)
    expect(existing).toHaveLength(1)
    expect(incoming).toHaveLength(1)
    expect(mergeRankPages([], [])).toEqual([])
    expect(mergeRankPages(existing, [])).toEqual(existing)
  })
})

describe('filterRankRowsByGroup', () => {
  const rows = [
    makeRow({ uid: 'u1', rank: 1 }),
    makeRow({ uid: 'star', rank: -1 }),
    makeRow({ uid: 'f1', rank: 2, gender: 'female' }),
    makeRow({ uid: 'star-f', rank: -1, gender: 'female' }),
  ]

  it('star → 只留 rank === -1 的行（打星且女生也按 star 命中）', () => {
    expect(filterRankRowsByGroup(rows, 'star').map((r) => r.uid)).toEqual(['star', 'star-f'])
  })

  it('female → 只留 gender === female 的行（含打星女生队）', () => {
    expect(filterRankRowsByGroup(rows, 'female').map((r) => r.uid)).toEqual(['f1', 'star-f'])
  })

  it('all / official 原样返回（official 的过滤在服务端 removeStar 完成）', () => {
    expect(filterRankRowsByGroup(rows, 'all')).toBe(rows)
    expect(filterRankRowsByGroup(rows, 'official')).toBe(rows)
  })
})

describe('paginateRankRows', () => {
  const rows = Array.from({ length: 120 }, (_, i) => makeRow({ uid: `u${i}`, rank: i + 1 }))

  it('按页切片：第 1 页取前 size 条，第 2 页取其后 size 条', () => {
    expect(paginateRankRows(rows, 1, 50)).toHaveLength(50)
    expect(paginateRankRows(rows, 1, 50)[0].uid).toBe('u0')
    expect(paginateRankRows(rows, 2, 50)[0].uid).toBe('u50')
    expect(paginateRankRows(rows, 3, 50)).toHaveLength(20)
  })

  it('current < 1 钳到第 1 页；越界页返回空数组', () => {
    expect(paginateRankRows(rows, 0, 50)[0].uid).toBe('u0')
    expect(paginateRankRows(rows, -3, 50)[0].uid).toBe('u0')
    expect(paginateRankRows(rows, 4, 50)).toEqual([])
  })

  it('size <= 0 视为不分页；空数组安全', () => {
    expect(paginateRankRows(rows, 1, 0)).toBe(rows)
    expect(paginateRankRows([], 1, 50)).toEqual([])
  })
})

// ── resolveOiRankCell ──
//
// OI 的 submissionInfo 值是整数得分，走 ACM 判据会全部落到 none（整张榜单变成一片 '-'），
// 因此必须有独立映射，且档位口径与 HOJ 文档 §8 一致。

describe('resolveOiRankCell', () => {
  function oiCell(score: number): RankCell {
    return {
      errorNum: 0,
      tryNum: 0,
      isAc: false,
      isFirstAc: false,
      acTime: 0,
      isAfterContest: false,
      score,
    }
  }

  it('无提交时显示占位符且不显示耗时', () => {
    expect(resolveOiRankCell(undefined, undefined)).toEqual({
      kind: 'none',
      scoreText: '-',
      timeText: null,
      hint: '暂无提交',
    })
  })

  it('满分档：score >= 100', () => {
    const cell = resolveOiRankCell(oiCell(100), 125_000)
    expect(cell.kind).toBe('full')
    expect(cell.scoreText).toBe('100')
    expect(cell.timeText).toBe('02:05')
    expect(cell.hint).toBe('满分 100')
  })

  it('部分分档：0 < score < 100', () => {
    const cell = resolveOiRankCell(oiCell(60), 90_000)
    expect(cell.kind).toBe('partial')
    expect(cell.hint).toBe('部分分 60')
    expect(cell.timeText).toBe('01:30')
  })

  it('零分档：score <= 0', () => {
    expect(resolveOiRankCell(oiCell(0), 0).kind).toBe('zero')
    expect(resolveOiRankCell(oiCell(0), 0).hint).toBe('未得分')
  })

  it('耗时缺失或非正数时不显示时间（OI 的 timeInfo 单位是毫秒）', () => {
    expect(resolveOiRankCell(oiCell(100), 0).timeText).toBeNull()
    expect(resolveOiRankCell(oiCell(100), undefined).timeText).toBeNull()
    // 1500ms 应向下取整为 1s，而不是被当成 1500 秒
    expect(resolveOiRankCell(oiCell(100), 1500).timeText).toBe('00:01')
  })
})
