# error

## 职责
定义比赛服务相关的错误类型，使用 `thiserror` 派生 `Error` trait，供比赛服务各操作返回统一错误。

## 核心类型/函数
- `enum ContestError` — 比赛错误枚举
  - `FetchFailed(String)` — 获取比赛失败，携带失败原因
  - `NotFound(String)` — 比赛未找到，携带比赛标识

## 直接依赖
- `thiserror::Error`（第三方 crate）

## 被依赖
无（当前阶段尚无外部模块直接引用 `ContestError`）

## 逻辑流程
无（仅类型定义）
