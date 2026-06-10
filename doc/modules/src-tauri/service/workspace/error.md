# error

## 职责
定义工作区服务相关的错误类型，使用 `thiserror` 派生 `Error` trait，供工作区服务各操作返回统一错误。

## 核心类型/函数
- `enum WorkspaceError` — 工作区错误枚举
  - `NotFound(String)` — 工作区未找到，携带工作区标识
  - `IoError(String)` — 文件读写错误，携带 IO 错误描述

## 直接依赖
- `thiserror::Error`（第三方 crate）

## 被依赖
无（当前阶段尚无外部模块直接引用 `WorkspaceError`）

## 逻辑流程
无（仅类型定义）
