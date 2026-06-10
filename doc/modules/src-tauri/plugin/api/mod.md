# mod

## 职责
插件统一 API 入口模块，声明并公开 `workspace`、`problem`、`contest`、`ui`、`event` 五个子模块，作为插件调用宿主能力的统一命名空间。

## 核心类型/函数
- 无（仅模块声明）

## 直接依赖
- 无

## 被依赖
- `plugin/mod.rs` — 通过 `pub mod api` 声明子模块

## 逻辑流程
无（仅模块声明）
