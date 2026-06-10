# error

## 职责
定义 HOJ 适配器的错误类型。目前仅包含一个通用的 API 错误变体，用于封装 HOJ API 调用过程中产生的错误。

## 核心类型/函数
- `enum HOJError` — HOJ 适配器错误枚举
  - `ApiError(String)` — 表示 HOJ API 调用错误，携带错误描述字符串

## 直接依赖
- `thiserror::Error`（derive 宏，用于自动生成 `Display` 和 `std::error::Error` 实现）

## 被依赖
- `adapter/hoj/mod.rs`（`pub mod error`）

## 逻辑流程
无（仅错误类型定义）
