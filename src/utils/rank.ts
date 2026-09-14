/**
 * 榜单渲染的纯映射（无副作用，View 层直接消费）。
 *
 * 规则严格照 `doc/HOJ/HOJ-Contest-Rank-API.md` §8「客户端需要自行补充计算」与
 * §9「客户端实现注意事项」：HOJ 服务端只返回原始聚合（errorNum/tryNum/isAC/ACTime…），
 * 单元格文案、样式类别、显示名回退、去重与真实参与人数都要客户端自行推导。
 * 把这些推导集中在此处，避免散落在多个 View 里各自实现产生分歧。
 */

import type { ContestRankPage, ContestRankRow, RankCell } from '@/types/rank'

/**
 * 单元格样式类别（对应 HOJ 前端 cellClassName 的取值语义）：
 * - `first-ac` 一血；`ac` 赛中通过；`after-ac` 赛后提交通过（显示加 `*`）
 * - `sealed` 封榜期间只有尝试次数，不显示通过状态
 * - `wa` 尝试未通过（ICPC 习惯显示 `-罚时次数`）；`none` 无任何记录（UI 显示 `-`）
 */
export type RankCellKind = 'first-ac' | 'ac' | 'after-ac' | 'sealed' | 'wa' | 'none'

/// resolveRankCell 的结果：样式类别 + 两行文案（null 表示该行不渲染）
export interface ResolvedRankCell {
  kind: RankCellKind
  timeText: string | null
  triesText: string | null
}

/**
 * 把原始 RankCell 映射为渲染所需的类别与文案（HOJ 文档 §8 单元格规则）。
 *
 * HOJ 的坑：
 * - `errorNum` 不含本次 AC，前端显示尝试数要 `+1`（把 AC 那次计入）；
 * - 封榜期间服务端只累加 `tryNum`、不写 `isAC`，因此 `tryNum != null` 即封榜行，
 *   显示 `{errorNum}+{tryNum} tries`（封榜前的失败次数 + 封榜后的盲提交次数）；
 * - 未出现在 `submissionInfo` 中的题（cell 为 undefined）渲染空格子。
 */
export function resolveRankCell(cell: RankCell | undefined): ResolvedRankCell {
  if (!cell) return { kind: 'none', timeText: null, triesText: null }

  if (cell.isAc) {
    const kind: RankCellKind = cell.isAfterContest
      ? 'after-ac'
      : cell.isFirstAc
        ? 'first-ac'
        : 'ac'
    // acTime 理论上 AC 后必有；缺失时按非法值渲染 '--:--' 而非崩溃
    const time = formatRankTime(cell.acTime ?? Number.NaN)
    return {
      kind,
      timeText: cell.isAfterContest ? `*${time}` : time,
      triesText: `${cell.errorNum + 1} 试`,
    }
  }

  // 封榜行优先于 wa 行：封榜期间 errorNum 可能同时 >0（封榜前的失败）
  if (cell.tryNum != null) {
    return { kind: 'sealed', timeText: null, triesText: `${cell.errorNum}+${cell.tryNum} tries` }
  }

  if (cell.errorNum > 0) {
    return { kind: 'wa', timeText: null, triesText: `-${cell.errorNum}` }
  }

  return { kind: 'none', timeText: null, triesText: null }
}

/**
 * 格式化榜单时间。`acTime` 是**相对开赛时刻的进度秒数**（不是墙钟时间），
 * 因此按时长格式渲染：不足 1 小时 `mm:ss`（如 8 秒 → `00:08`、8 分钟 → `08:00`），
 * 超过 1 小时 `h:mm:ss`（如 3720 秒 → `1:02:00`）。
 *
 * 负数 / NaN / Infinity 等非法值返回 `--:--`（服务端脏数据不应让 UI 崩溃）。
 */
export function formatRankTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return '--:--'
  const total = Math.floor(seconds)
  const h = Math.floor(total / 3600)
  const m = Math.floor((total % 3600) / 60)
  const s = total % 60
  const mm = String(m).padStart(2, '0')
  const ss = String(s).padStart(2, '0')
  return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`
}

/**
 * 按比赛配置 `contest.rankShowName`（`username`/`realname`/`nickname`）取榜单显示名。
 *
 * HOJ 允许 realname/nickname 为空（未填写），为空时回退 `username`，
 * 再为空回退 `uid`，保证单元格永远有可辨识文本；未知取值按 `username` 处理。
 */
export function resolveDisplayName(row: ContestRankRow, rankShowName: string): string {
  const candidate =
    rankShowName === 'realname'
      ? row.realname
      : rankShowName === 'nickname'
        ? row.nickname
        : row.username
  if (candidate && candidate.trim()) return candidate
  if (row.username && row.username.trim()) return row.username
  return row.uid
}

/// 行样式类别：打星行（不参与排名）> 女生行（背景色）> 普通行
export type RankRowKind = 'star' | 'female' | 'normal'

/**
 * 行样式判定（HOJ 文档 §8 userCellClassName）：`rank === -1` 为打星队伍，
 * 优先级最高（打星且 female 时仍按 star 渲染）；`gender === 'female'` 加背景色。
 */
export function resolveRowKind(row: ContestRankRow): RankRowKind {
  if (row.rank === -1) return 'star'
  if (row.gender === 'female') return 'female'
  return 'normal'
}

/**
 * 按 `uid` 去重，保留首次出现的行。
 *
 * HOJ 会把「当前登录用户」与「关注列表用户」复制一份插到 records 最前面
 * （文档 §9.2），第 1 页因此可能出现同一 uid 的重复行，直接渲染会显示两遍。
 */
export function dedupeRankRows(rows: ContestRankRow[]): ContestRankRow[] {
  const seen = new Set<string>()
  const result: ContestRankRow[] = []
  for (const row of rows) {
    if (seen.has(row.uid)) continue
    seen.add(row.uid)
    result.push(row)
  }
  return result
}

/** 在当前页中定位「我的行」（用于置顶高亮）；uid 为空或未找到返回 null */
export function resolveMyRow(
  rows: ContestRankRow[],
  uid: string | null,
): ContestRankRow | null {
  if (!uid) return null
  return rows.find((row) => row.uid === uid) ?? null
}

/**
 * 推算真实参与人数：取**去重后非打星行**的最大 `rank`。
 *
 * 不能直接用分页的 `total`（文档 §9.2）：服务端把当前用户与关注用户前置复制了一份，
 * `total` 会比真实人数偏大；打星队伍 `rank === -1` 不参与排名也必须剔除。
 * 仅当本页取不到任何有效 rank（空榜单/整页都是打星行）时才回退 `total`。
 */
export function resolveParticipantCount(rows: ContestRankRow[], total: number): number {
  let maxRank = Number.NEGATIVE_INFINITY
  for (const row of dedupeRankRows(rows)) {
    if (row.rank === -1) continue
    if (row.rank > maxRank) maxRank = row.rank
  }
  return Number.isFinite(maxRank) ? maxRank : total
}

/**
 * ACM 总罚时（秒）→ 分钟整数字符串（如 2700 → `45`），向下取整。
 * 非法/负值返回 `--`。注意 OI 赛制的 totalTime 是毫秒，不适用本函数。
 */
export function formatPenaltyMinutes(totalTimeSeconds: number): string {
  if (!Number.isFinite(totalTimeSeconds) || totalTimeSeconds < 0) return '--'
  return String(Math.floor(totalTimeSeconds / 60))
}

/**
 * 从整页响应推算真实参与人数（统计卡的「#42 / 360」分母）。
 *
 * 为什么不能直接用 `resolveParticipantCount(page.records, page.total)`：
 * 该函数取的是「传入行的最大 rank」，在第 1 页（limit=50）上只能得到 ~50，
 * 而不是全场人数。
 *
 * 这里改用 `total - 本页重复行数`：`total` 的偏大量恰好等于服务端前置复制的条目数
 * （文档 §9.2），而前置副本与其自然位置同时出现在本页时即可被去重计数捕获。
 * 残余误差：当「我」的自然名次不在本页时，前置副本不构成重复，分母会偏大 1 —— 
 * 对展示无实质影响，故不再为此额外请求末页。
 * `total` 缺失（<=0）时才退回最大 rank 口径。
 */
export function resolveParticipantCountFromPage(page: ContestRankPage): number {
  const duplicates = page.records.length - dedupeRankRows(page.records).length
  const corrected = page.total - duplicates
  if (corrected > 0) return corrected
  return resolveParticipantCount(page.records, page.total)
}

// ── OI 赛制单元格 ──

/// OI 档位，对应 HOJ 文档 §8 的 `oi-100` / `oi-between` / `oi-0`
export type OiCellKind = 'full' | 'partial' | 'zero' | 'none'

export interface ResolvedOiCell {
  kind: OiCellKind
  /** 得分文本；无提交时为 `-` */
  scoreText: string
  /** 最优耗时文本（毫秒换算而来）；无有效耗时为 null */
  timeText: string | null
  /** 悬浮提示 */
  hint: string
}

/**
 * OI 赛制单元格映射。
 *
 * 不能复用 `resolveRankCell`：OI 的 `submissionInfo` 值是整数得分（Adapter 归一到
 * `RankCell.score`，其余字段取默认值），走 ACM 判据会全部落到 `none`，
 * 整张 OI 榜单会渲染成一片 `-`。
 *
 * @param cell       该题的归一单元格（undefined = 无提交）
 * @param timeInfoMs `timeInfo` 中该题的最优耗时（**毫秒**，与 ACM 的秒不同）
 */
export function resolveOiRankCell(
  cell: RankCell | undefined,
  timeInfoMs: number | undefined,
): ResolvedOiCell {
  const score = cell?.score ?? null
  const timeText =
    typeof timeInfoMs === 'number' && timeInfoMs > 0
      ? formatRankTime(Math.floor(timeInfoMs / 1000))
      : null

  if (score === null) {
    return { kind: 'none', scoreText: '-', timeText: null, hint: '暂无提交' }
  }
  if (score >= 100) {
    return { kind: 'full', scoreText: String(score), timeText, hint: `满分 ${score}` }
  }
  if (score > 0) {
    return { kind: 'partial', scoreText: String(score), timeText, hint: `部分分 ${score}` }
  }
  return { kind: 'zero', scoreText: String(score), timeText, hint: '未得分' }
}
