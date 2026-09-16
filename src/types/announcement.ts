/// 比赛公告实体，对应 Rust `core::entity::announcement::Announcement`。
///
/// HOJ 无「已读」概念，已读状态由客户端本地持久化（见 announcementStore）。
export interface Announcement {
  id: string
  title: string
  /// 公告正文（Markdown / HTML 混合，渲染前必须经 renderMarkdown 出口消毒）
  content: string
  /// 发布者用户名
  author: string
  /// 发布时间（epoch 秒，与 Contest.startTime 同口径）
  createdAt: number
  /// 最后编辑时间（epoch 秒）；HOJ 未返回时与 createdAt 相同或为 0
  updatedAt: number
}

/// 公告分页结果，对应 Rust `AnnouncementPage`。
export interface AnnouncementPage {
  records: Announcement[]
  total: number
  size: number
  current: number
  pages: number
}
