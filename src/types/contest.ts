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
  problems: string[]
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
}
