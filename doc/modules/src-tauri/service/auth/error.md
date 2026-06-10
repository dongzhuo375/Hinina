# error

## 职责
定义认证服务相关的错误类型，使用 `thiserror` 派生 `Error` trait，供认证服务各操作返回统一错误。

## 核心类型/函数
- `enum AuthError` — 认证错误枚举
  - `LoginFailed(String)` — 登录失败，携带失败原因
  - `SessionExpired` — 会话已过期

## 直接依赖
- `thiserror::Error`（第三方 crate）

## 被依赖
无（当前阶段尚无外部模块直接引用 `AuthError`）

## 逻辑流程
无（仅类型定义）
