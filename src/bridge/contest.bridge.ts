import type { Contest, ContestProblem } from '@/types/contest'
import { ipcInvoke } from '@/bridge'

/** 加载已配置的比赛及其题目列表 */
export async function loadConfiguredContest(): Promise<{ contest: Contest; problems: ContestProblem[] }> {
  return ipcInvoke<{ contest: Contest; problems: ContestProblem[] }>('load_configured_contest')
}

/** 获取比赛列表（匿名接口，登录页展示比赛信息/倒计时用） */
export async function listContests(): Promise<Contest[]> {
  return ipcInvoke<Contest[]>('list_contests')
}
