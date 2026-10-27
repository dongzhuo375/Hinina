# error

## 职责
定义 HOJ 适配器专用错误类型。

## 核心类型/函数
- `enum HOJError`：
  - `ApiError(i32, String)` — API 返回非 200 状态码
  - `HttpError(String)` — HTTP 请求失败
  - `JsonError(String)` — JSON 解析失败
  - `Unauthorized(String)` — Token 缺失
  - `UnknownStatus(i32)` — 未知评测状态码
- `impl From<HOJError> for AppError` — 逐变体映射：`ApiError`/`HttpError` → `AppError::Network`、`JsonError` → `Serialization`、`Unauthorized` → `Auth`、`UnknownStatus` → `Unknown`

## 直接依赖
- `thiserror::Error`（derive 宏）
- `core::error::AppError`（From 转换目标）

## 被依赖
- 暂无（`HOJError` 目前未被适配器本体引用：`adapter/hoj/mod.rs` 直接构造 `AppError`，本模块保留为专用错误类型的预留位）
