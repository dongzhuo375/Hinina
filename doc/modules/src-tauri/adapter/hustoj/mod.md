# mod

## 职责
HUSTOJ 适配器的根文件。声明 `types` 和 `error` 两个子模块。本适配器计划按需组合实现核心 provider trait。

## 核心类型/函数
- `pub mod types` — 声明 HUSTOJ 类型定义子模块
- `pub mod error` — 声明 HUSTOJ 错误类型子模块

## 直接依赖
无（仅声明子模块）

## 被依赖
- `adapter/mod.rs`（`pub mod hustoj`）

## 逻辑流程
无（仅模块声明）
