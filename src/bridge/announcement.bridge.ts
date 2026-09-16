import type { AnnouncementPage } from '@/types/announcement'
import { ipcInvoke } from '@/bridge'

/**
 * 公告 Bridge — 比赛公告列表与本地已读状态的 IPC 薄封装。
 *
 * 已读状态是客户端特性（HOJ 无已读概念），由 Rust 端按 比赛+用户 持久化到本地文件。
 */

/** 获取比赛公告列表（分页） */
export async function listContestAnnouncements(
  contestId: string,
  currentPage: number,
  limit: number,
): Promise<AnnouncementPage> {
  return ipcInvoke<AnnouncementPage>('list_contest_announcements', {
    contestId,
    currentPage,
    limit,
  })
}

/** 获取当前用户在该比赛下已读的公告 ID 列表（未登录返回空） */
export async function getReadAnnouncementIds(contestId: string): Promise<string[]> {
  return ipcInvoke<string[]>('get_read_announcement_ids', { contestId })
}

/** 将指定公告标记为已读（后端与既有已读集合合并去重持久化） */
export async function markAnnouncementsRead(contestId: string, ids: string[]): Promise<void> {
  return ipcInvoke<void>('mark_announcements_read', { contestId, ids })
}
