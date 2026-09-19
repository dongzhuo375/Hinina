/// 评测状态，对应 Rust `core::entity::submission::JudgementStatus`。
///
/// 值域是 HOJ `Constants.Judge` 的（见 Rust `adapter/hoj/types.rs::map_status`）；
/// 变体名即 serde 序列化字符串，两端必须同步演进。
export type JudgementStatus =
  | 'NotSubmitted'
  | 'Cancelled'
  | 'Pending'
  | 'Compiling'
  | 'Running'
  | 'Accepted'
  | 'WrongAnswer'
  | 'TimeLimitExceeded'
  | 'MemoryLimitExceeded'
  | 'RuntimeError'
  | 'CompilationError'
  | 'PresentationError'
  | 'OutputLimitExceeded'
  | 'SystemError'
  | 'RemoteJudgeError'
  | 'SubmitFailed'
  | 'PartiallyAccepted'
  | 'FrequentLimit'
  | 'UnknownError'
  | 'Unknown'

/// 评测结果详情，对应 Rust `core::entity::submission::JudgementResult`。
export interface JudgementResult {
  status: JudgementStatus
  score: number
  timeMs: number
  memoryKb: number
  /// 失败原因（CE / SE / SF 时非空）。
  ///
  /// 轮询是选手感知评测失败的唯一自动通道，故错误信息随轮询结果一起回来，
  /// 而不是等选手点进详情页才发现。服务端的占位文案已在 Rust 侧过滤。
  errorMessage: string | null
}

/// 提交列表行，对应 Rust `SubmissionRecord`（来源 HOJ JudgeVO）。
export interface SubmissionRecord {
  submitId: string
  /// 题目数字 ID（HOJ 主键）
  pid: string
  /// 题目展示 ID（如 "HOJ-1001"）
  displayPid: string
  /// 题目标题
  title: string
  /// 比赛内展示题号（"A"/"B"…；completeProblemID=true 时服务端回填，可能为空串）
  displayId: string
  username: string
  /// 提交时间（epoch 秒，与 Contest.startTime 同口径）
  submitTime: number
  status: JudgementStatus
  timeMs: number
  memoryKb: number
  /// OI 题目得分；ACM 题目为 null
  score: number | null
  /// 代码长度（字节）
  length: number
  language: string
}

/// 提交列表分页结果，对应 Rust `SubmissionPage`。
export interface SubmissionPage {
  records: SubmissionRecord[]
  total: number
  size: number
  current: number
  pages: number
}

/// 提交详情，对应 Rust `SubmissionDetail`（来源 HOJ get-submission-detail）。
export interface SubmissionDetail {
  submitId: string
  pid: string
  displayPid: string
  username: string
  submitTime: number
  status: JudgementStatus
  timeMs: number
  memoryKb: number
  score: number | null
  length: number
  language: string
  /// 提交的源代码
  code: string
  /// 错误信息（CE 时为编译错误输出）；无错误为 null
  errorMessage: string | null
  /// 评测机节点名；未分配为 null
  judger: string | null
  /// OI 榜单计分；ACM 题目为 null
  oiRankScore: number | null
}

/// 单个测试点结果，对应 Rust `JudgeCase`。
export interface JudgeCase {
  caseId: number
  /// 测试点序号（从 1 开始）
  seq: number
  status: JudgementStatus
  timeMs: number
  memoryKb: number
  /// OI 题目该测试点得分；ACM 为 null
  score: number | null
  /// 子任务分组号；非子任务制为 null
  groupNum: number | null
}

/// 子任务分组（OI subtask 判题），对应 Rust `SubTaskCases`。
export interface SubTaskCases {
  groupNum: number
  cases: JudgeCase[]
}

/// 测试点详情聚合，对应 Rust `SubmissionCases`（来源 HOJ get-all-case-result）。
export interface SubmissionCases {
  cases: JudgeCase[]
  subTasks: SubTaskCases[]
  /// 判题模式："default" / "spj" / "subtask" 等
  mode: string
}

/// 提交列表查询参数（前端侧；onlyMine 由后端命令强制为 true，前端不传）。
export interface SubmissionListQuery {
  contestId: string
  currentPage: number
  limit: number
  /// 按比赛内展示题号筛选（"A"/"B"…）；null 为全部题目
  problemDisplayId?: string | null
  /// 按 HOJ 状态码筛选（见 utils/submission STATUS_OPTIONS）；null 为全部状态
  status?: number | null
}
