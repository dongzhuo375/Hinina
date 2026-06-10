# mod

## 职责
`core/event/` 模块入口，声明事件系统子模块：`app_event`（事件枚举定义）、`event_bus`（事件总线）、`event_category`（事件分类）。

## 核心类型/函数
无（仅模块声明）。

## 直接依赖
无外部依赖，仅声明子模块。

## 被依赖
- `core::mod`（通过 `pub mod event` 声明）

## 逻辑流程
无（仅模块声明）。
