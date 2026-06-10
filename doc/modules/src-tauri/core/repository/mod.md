# mod

## 职责
`core/repository/` 模块入口，声明持久化仓库 trait 子模块：`workspace_repo`（工作区仓库）、`config_repo`（配置仓库）、`plugin_repo`（插件仓库）。

## 核心类型/函数
无（仅模块声明）。

## 直接依赖
无外部依赖，仅声明子模块。

## 被依赖
- `core::mod`（通过 `pub mod repository` 声明）

## 逻辑流程
无（仅模块声明）。
