import type {
  JudgementResult,
  SubmissionCases,
  SubmissionDetail,
  SubmissionListQuery,
  SubmissionPage,
} from '@/types/submission'
import { ipcInvoke } from '@/bridge'

/**
 * 提交代码，返回 submissionId。
 *
 * `problemId` 与 `displayId` 是同一道题的两个标识，必须都传：
 * - `problemId`：题目真实 ID（工作区隔离、状态查询的键）；
 * - `displayId`：比赛内展示题号（如 "A"）。
 *
 * HOJ 的提交接口收的是**展示题号**：传数字 pid 会让服务端查不到 contest_problem
 * 而抛 NPE，返回 HTTP 500（实测）。Hydro 则收真实 ID —— 由 Adapter 各取所需。
 */
export async function submitCode(
  contestId: string,
  problemId: string,
  displayId: string,
  language: string,
  sourceCode: string,
): Promise<string> {
  return ipcInvoke<string>('submit_code', {
    contestId,
    problemId,
    displayId,
    language,
    sourceCode,
  })
}

/** 获取评测结果 */
export async function getJudgement(submissionId: string): Promise<JudgementResult> {
  return ipcInvoke<JudgementResult>('get_judgement', { submissionId })
}

/**
 * 获取本人在该比赛的提交列表（分页）。
 *
 * 「只看本人」由 Rust 命令层强制（onlyMine 恒为 true），前端不传该参数。
 */
export async function listContestSubmissions(query: SubmissionListQuery): Promise<SubmissionPage> {
  return ipcInvoke<SubmissionPage>('list_contest_submissions', {
    contestId: query.contestId,
    currentPage: query.currentPage,
    limit: query.limit,
    problemDisplayId: query.problemDisplayId ?? null,
    status: query.status ?? null,
  })
}

/** 获取提交详情（含源代码与错误信息） */
export async function getSubmissionDetail(submissionId: string): Promise<SubmissionDetail> {
  return ipcInvoke<SubmissionDetail>('get_submission_detail', { submissionId })
}

/** 获取提交的测试点详情（含子任务分组） */
export async function getSubmissionCases(submissionId: string): Promise<SubmissionCases> {
  return ipcInvoke<SubmissionCases>('get_submission_cases', { submissionId })
}
