# error

## 职责
定义配置服务相关的错误类型，使用 `thiserror` 派生 `Error` trait，供配置服务各操作返回统一错误。

## 核心类型/函数
- `enum ConfigError` — 配置错误枚举
  - `LoadFailed(String)` — 配置加载失败，携带失败原因
  - `SaveFailed(String)` — 配置保存失败，携带失败原因

## 直接依赖
- `thiserror::Error`（第三方 crate）

## 被依赖
无（当前阶段尚无外部模块直接引用 `ConfigError`）

## 逻辑流程
无（仅类型定义）
