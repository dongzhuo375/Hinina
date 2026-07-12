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

## 直接依赖
- `thiserror::Error`（derive 宏）

## 被依赖
- `adapter/hoj/mod.rs`
