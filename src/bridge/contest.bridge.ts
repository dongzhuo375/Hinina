import type { Contest, ContestProblem } from '@/types/contest'
import { ipcInvoke } from '@/bridge'

/** 加载已配置的比赛及其题目列表 */
export async function loadConfiguredContest(): Promise<{ contest: Contest; problems: ContestProblem[] }> {
  return ipcInvoke<{ contest: Contest; problems: ContestProblem[] }>('load_configured_contest')
}
