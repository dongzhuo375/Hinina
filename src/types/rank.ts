/**
 * 榜单与题目限制的前后端跨端契约。
 *
 * 字段形状与 Rust 侧 `core::entity::rank` 严格对齐（camelCase 序列化），
 * 已把 HOJ 的 ACM/OI 两套 VO（`ACMContestRankVO` / `OIContestRankVO`）
 * 以及原始字段命名差异（`isAC`/`isFirstAC`/`ACTime`）归一为 OJ 无关结构，
 * 前端渲染只依赖本文件的形状，不感知适配器差异。
 */

/// 榜单单元格（ACM 与 OI 归一后的 OJ 无关结构），对应 Rust core::entity::rank::RankCell
export interface RankCell {
  errorNum: number            // 未通过次数（含罚时）；AC 行显示尝试数时需 +1
  tryNum: number | null       // 封榜期间的提交次数；非封榜为 null
  isAc: boolean
  isFirstAc: boolean
  acTime: number | null       // AC 时的比赛进度（秒，相对 startTime）
  isAfterContest: boolean     // 赛后提交且 containsEnd=true
  score: number | null        // OI 赛制该题得分
}

/// 榜单行，对应 Rust ContestRankRow
export interface ContestRankRow {
  rank: number                // -1 表示打星队伍（不参与排名）
  uid: string
  username: string
  realname: string
  nickname: string
  school: string
  gender: string              // 'female' 时前端加背景色
  avatar: string
  ac: number                  // ACM：AC 题数
  total: number               // ACM：总提交数
  totalTime: number           // ACM：秒；OI：毫秒
  totalScore: number | null   // OI：总得分
  submissionInfo: Record<string, RankCell>  // key = displayId（"A"/"B"...）
  timeInfo: Record<string, number>          // OI：key = displayId，value = 最优耗时(ms)
}

/// 分页榜单，对应 Rust ContestRankPage（MyBatis-Plus IPage）
export interface ContestRankPage {
  records: ContestRankRow[]
  total: number
  size: number
  current: number
  pages: number
}

/**
 * 榜单查询参数（前端调用态）。
 *
 * 与 Rust 的 `RankQuery` **同名但字段集不同**，这是刻意的：
 * Rust 侧 `ContestProvider::get_contest_rank(contest_id, &query)` 把比赛 ID 作为独立形参
 * （Provider 方法一律以 `&str` 传 ID，与 get_contest / list_contest_problems 保持一致），
 * 而前端把 contestId 一并装进查询对象，让 store → service → bridge 只传一个参数；
 * `bridge/rank.bridge.ts` 在 IPC 边界把它摊平回独立字段，与 Command 签名对齐。
 */
export interface RankQuery {
  contestId: string
  currentPage: number
  limit: number
  keyword?: string | null
  removeStar?: boolean
  containsEnd?: boolean
}

/// 题目限制，对应 Rust ProblemLimits
export interface ProblemLimits {
  displayId: string
  timeLimit: number    // 毫秒
  memoryLimit: number  // MB
}

/// 我的题目状态：0=未提交，1=已AC，2=尝试过
export type UserProblemStatus = 0 | 1 | 2
