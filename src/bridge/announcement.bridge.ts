import type { UnlistenFn } from '@tauri-apps/api/event'
import { listen } from '@tauri-apps/api/event'
import type { AnnouncementPage } from '@/types/announcement'
import { ipcInvoke } from '@/bridge'

/**
 * 公告 Bridge — 比赛公告列表、本地已读状态与新公告事件的 IPC 薄封装。
 *
 * 已读状态是客户端特性（HOJ 无已读概念），由 Rust 端按 比赛+用户 持久化到本地文件。
 * 新公告事件由 Rust 侧比对基线后发布（`CoreEvent::AnnouncementChanged`），
 * 经 `src-tauri/src/main.rs` 的事件桥转发到 `announcements-published` 通道。
 */

/** 新公告事件的前端通道名（与 `src-tauri/src/main.rs` 的 emit 一致） */
export const ANNOUNCEMENTS_PUBLISHED_EVENT = 'announcements-published'

/** 新公告事件载荷 */
export interface AnnouncementsPublishedPayload {
  contestId: string
  /// 本次新出现的公告 ID
  newIds: string[]
}

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

/**
 * 订阅「检测到新公告」事件。
 *
 * 返回取消订阅函数；调用方（组合根）应保留并在必要时调用，避免重复注册。
 * 事件只在 Rust 侧确认出现**新 ID** 时下发 —— 首次拉取不发，故订阅方不必自己去重。
 *
 * 事件只是**刷新触发**：真实公告内容始终由 `listContestAnnouncements` 经 IPC 查询，
 * 事件丢失时下一次轮询（60s±10s）会补齐。
 */
export async function onAnnouncementsPublished(
  handler: (payload: AnnouncementsPublishedPayload) => void,
): Promise<UnlistenFn> {
  return listen<AnnouncementsPublishedPayload>(ANNOUNCEMENTS_PUBLISHED_EVENT, (event) => {
    const payload = event.payload
    if (!payload || typeof payload.contestId !== 'string') return
    handler({ contestId: payload.contestId, newIds: payload.newIds ?? [] })
  })
}
