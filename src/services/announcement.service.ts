import type { AnnouncementPage } from '@/types/announcement'
import * as announcementBridge from '@/bridge/announcement.bridge'

/// 公告列表单页容量：比赛公告总量通常 < 30 条，一页取满免去翻页交互
const ANNOUNCEMENT_PAGE_SIZE = 100

/**
 * 公告服务 — 比赛公告获取与本地已读状态编排。
 *
 * 已读状态是客户端特性（HOJ 无已读概念）：Rust 端按「比赛 + 用户」持久化
 * 已读 ID 集合，本服务只负责透传与轻量组装，不做缓存（公告必须实时）。
 */
export class AnnouncementService {
  /** 拉取公告列表（单页大容量，覆盖常规比赛公告总量） */
  async listAnnouncements(contestId: string): Promise<AnnouncementPage> {
    return announcementBridge.listContestAnnouncements(contestId, 1, ANNOUNCEMENT_PAGE_SIZE)
  }

  /** 当前用户在该比赛下的已读公告 ID 集合 */
  async getReadIds(contestId: string): Promise<Set<string>> {
    const ids = await announcementBridge.getReadAnnouncementIds(contestId)
    return new Set(ids)
  }

  /**
   * 将给定公告标记为已读并返回持久化后的完整已读集合。
   *
   * 后端做合并去重，前端拿到的是合并后的权威集合，直接替换本地状态即可。
   */
  async markRead(contestId: string, ids: string[]): Promise<Set<string>> {
    if (ids.length === 0) return this.getReadIds(contestId)
    await announcementBridge.markAnnouncementsRead(contestId, ids)
    return this.getReadIds(contestId)
  }
}

export const announcementService = new AnnouncementService()
