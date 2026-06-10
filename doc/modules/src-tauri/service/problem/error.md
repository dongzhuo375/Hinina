# error

## 职责
定义题目服务相关的错误类型，使用 `thiserror` 派生 `Error` trait，供题目服务各操作返回统一错误。

## 核心类型/函数
- `enum ProblemError` — 题目错误枚举
  - `FetchFailed(String)` — 获取题目失败，携带失败原因
  - `NotFound(String)` — 题目未找到，携带题目标识

## 直接依赖
- `thiserror::Error`（第三方 crate）

## 被依赖
无（当前阶段尚无外部模块直接引用 `ProblemError`）

## 逻辑流程
无（仅类型定义）
