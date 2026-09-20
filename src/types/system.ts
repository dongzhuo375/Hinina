/// 客户端存储信息，对应 Rust `commands::config_cmd::StorageInfo`。
export interface StorageInfo {
  /// 本地存储根目录（配置/会话/工作区/缓存）
  baseDir: string
  /// 日志文件完整路径
  logPath: string
  /// 客户端版本号（构建期注入）
  version: string
}

/// 可清理的本地数据占用，对应 Rust `commands::maintenance_cmd::LocalDataUsage`。
///
/// 用途是**先看再删**：不可逆的删除动作必须让用户看到确切范围与体积。
export interface LocalDataUsage {
  /// 日志文件字节数
  logBytes: number
  /// 日志文件完整路径
  logPath: string
  /// 提交留档总条数
  snapshotTotalCount: number
  /// 提交留档总字节数
  snapshotTotalBytes: number
  /// 其中「过期」（早于保留窗口）的条数
  snapshotStaleCount: number
  /// 其中「过期」的字节数
  snapshotStaleBytes: number
  /// 留档保留窗口（天）：早于该窗口的留档视为可清理
  keepDays: number
}

/// 本地数据清理结果，对应 Rust `commands::maintenance_cmd::PurgeReport`。
export interface PurgeReport {
  /// 释放的总字节数
  freedBytes: number
  /// 删除的留档条数
  removedSnapshots: number
  /// 是否清空了日志内容（文件层不可用时为 false，不算失败）
  logCleared: boolean
}
