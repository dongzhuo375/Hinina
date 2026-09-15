/// 客户端存储信息，对应 Rust `commands::system_cmd::StorageInfo`。
export interface StorageInfo {
  /// 本地存储根目录（配置/会话/工作区/缓存）
  baseDir: string
  /// 日志文件完整路径
  logPath: string
  /// 客户端版本号（构建期注入）
  version: string
}
