# lib

## 职责
Crate 库根（`hinina_lib`）。声明并公开所有顶层模块，构成整个 Rust 后端的模块树入口。

## 核心类型/函数
- 无 — 仅包含 `pub mod` 声明

## 直接依赖
- 无外部依赖 — 仅声明子模块：
  - `pub mod core`
  - `pub mod service`
  - `pub mod adapter`
  - `pub mod infra`
  - `pub mod plugin`
  - `pub mod commands`

## 被依赖
- `src-tauri/src/main.rs` — 作为 `hinina_lib` crate 被导入，use 其 `commands` 和 `core::context::AppContext`
- 项目中所有 `use crate::...` 的实际 crate 根

## 逻辑流程
无（仅模块声明）
