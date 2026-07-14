import type { JudgementResult } from '@/types/submission'
import { ipcInvoke } from '@/bridge'

/** 提交代码，返回 submissionId */
export async function submitCode(
  contestId: string,
  problemId: string,
  language: string,
  sourceCode: string,
): Promise<string> {
  return ipcInvoke<string>('submit_code', { contestId, problemId, language, sourceCode })
}

/** 获取评测结果 */
export async function getJudgement(submissionId: string): Promise<JudgementResult> {
  return ipcInvoke<JudgementResult>('get_judgement', { submissionId })
}
