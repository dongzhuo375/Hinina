import type { Contest, ContestProblem } from '@/types/contest'
import * as contestBridge from '@/bridge/contest.bridge'
import { getConfig } from '@/bridge/config.bridge'

/**
 * 登录页匿名比赛简报的加载结果。
 *
 * - `ok`           已取到配置的比赛 ID 并完成列表筛选（`contest` 仍可能为 null：ID 不在列表中）
 * - `unconfigured` 配置未设置比赛 ID，调用方应提示完成配置
 *
 * 网络/IPC 异常不在此枚举内，直接向上抛出由调用方归一化处理。
 */
export type ContestBriefResult =
  | { status: 'ok'; contest: Contest | null; baseUrl: string }
  | { status: 'unconfigured'; baseUrl: string }

/**
 * 比赛服务 — 管理比赛数据的加载。
 */
export class ContestService {
  /**
   * 加载已配置的比赛及其题目列表（需有效会话）。
   */
  async loadConfiguredContest(): Promise<{ contest: Contest; problems: ContestProblem[] }> {
    return contestBridge.loadConfiguredContest()
  }

  /**
   * 加载登录页展示用的比赛简报（匿名接口，无需会话）。
   *
   * 编排：读取配置 → 取 contestId → 拉取匿名比赛列表 → 按 ID 筛选，
   * 同时返回 OJ 基址，供题面/简介中的相对图片 URL 改写使用。
   */
  async loadContestBrief(): Promise<ContestBriefResult> {
    const config = await getConfig()
    const baseUrl = config.oj.hojUrl
    const contestId = config.oj.contestId
    if (!contestId) return { status: 'unconfigured', baseUrl }

    const contests = await contestBridge.listContests()
    const contest = contests.find((c) => c.id === String(contestId)) ?? null
    return { status: 'ok', contest, baseUrl }
  }
}

export const contestService = new ContestService()
