/// 比赛实体，对应 Rust `core::entity::contest::Contest`。
export interface Contest {
  id: string
  title: string
  startTime: number
  endTime: number
  description: string
  contestType: number
  status: number
  auth: number
  /// 榜单显示名规则：`username` / `realname` / `nickname`；空串时回退 username
  rankShowName: string
  /// 是否封榜（封榜期间榜单只显示尝试次数，不显示通过状态）
  sealRank: boolean
  /// 封榜起始时间（UTC 秒）；未设置为 null
  sealRankTime: number | null
  /// 是否允许赛后提交（决定榜单查询 containsEnd 是否真正生效）
  allowEndSubmit: boolean
}

/// 比赛题目摘要，对应 Rust `core::entity::contest::ContestProblem`。
export interface ContestProblem {
  id: number
  displayId: string
  cid: number
  problemId: string
  displayTitle: string
  ac: number
  total: number
  /// 气球色（如 "#FF0000"），驱动题目卡片字母徽章与榜单列头配色；可能为空串
  color: string
}
