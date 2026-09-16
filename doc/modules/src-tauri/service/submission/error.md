# error

## 职责
定义提交服务相关的错误类型，使用 `thiserror` 派生 `Error` trait，供提交服务各操作返回统一错误。

## 核心类型/函数
- `enum SubmissionError` — 提交错误枚举
  - `SubmitFailed(String)` — 提交失败，携带失败原因

## 直接依赖
- `thiserror::Error`（第三方 crate）

## 被依赖
无（当前阶段尚无外部模块直接引用 `SubmissionError`）

## 逻辑流程
无（仅类型定义）
