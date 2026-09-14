/// 用户实体，对应 Rust `core::entity::user::User`。
export interface User {
  id: string
  username: string
  token: string
}

/**
 * 会话校验结果，对应 Rust `service::auth::SessionValidity`（serde snake_case）。
 *
 * - `valid`   服务端确认有效
 * - `invalid` 本地无会话或服务端已判定失效（磁盘会话已被清除），须重新登录
 * - `unknown` 网络异常等无法判定，本地会话保留，应稍后重试而非踢出用户
 */
export type SessionValidity = 'valid' | 'invalid' | 'unknown'
