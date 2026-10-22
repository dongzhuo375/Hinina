# mod

## 职责
`core` 模块的入口文件，声明 `core/` 下的所有子模块：`access`（显式访问修订与取消栅栏）、`entity`（领域实体）、`provider`（OJ Provider trait）、`event`（事件系统）、`repository`（持久化仓库 trait）、`error`（错误类型）、`context`（应用上下文）。`AccessTransition` 必须持有到一组关联变更全部完成；`#[must_use]` 提醒调用者避免立即丢弃 guard。

## 核心类型/函数
无（仅模块声明）。

## 直接依赖
无外部依赖，仅声明子模块。

## 被依赖
- `main.rs` / `lib.rs`（顶层 crate 入口）

## 逻辑流程
无（仅模块声明）。
