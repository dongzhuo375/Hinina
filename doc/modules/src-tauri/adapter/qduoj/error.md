# error

## 职责
定义 QDUOJ 适配器的错误类型。目前仅包含一个通用的 API 错误变体，用于封装 QDUOJ API 调用过程中产生的错误。

## 核心类型/函数
- `enum QDUOJError` — QDUOJ 适配器错误枚举
  - `ApiError(String)` — 表示 QDUOJ API 调用错误，携带错误描述字符串

## 直接依赖
- `thiserror::Error`（derive 宏）

## 被依赖
- `adapter/qduoj/mod.rs`（`pub mod error`）

## 逻辑流程
无（仅错误类型定义）
