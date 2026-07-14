import type { Contest, ContestProblem } from '@/types/contest'
import * as contestBridge from '@/bridge/contest.bridge'

/**
 * 比赛服务 — 管理比赛数据的加载。
 */
export class ContestService {
  /**
   * 加载已配置的比赛及其题目列表。
   */
  async loadConfiguredContest(): Promise<{ contest: Contest; problems: ContestProblem[] }> {
    return contestBridge.loadConfiguredContest()
  }
}

export const contestService = new ContestService()
