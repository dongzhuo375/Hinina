# error

## 职责
定义主题服务相关的错误类型，使用 `thiserror` 派生 `Error` trait，供主题服务各操作返回统一错误。

## 核心类型/函数
- `enum ThemeError` — 主题错误枚举
  - `NotFound(String)` — 主题未找到，携带主题名称

## 直接依赖
- `thiserror::Error`（第三方 crate）

## 被依赖
无（当前阶段尚无外部模块直接引用 `ThemeError`）

## 逻辑流程
无（仅类型定义）
