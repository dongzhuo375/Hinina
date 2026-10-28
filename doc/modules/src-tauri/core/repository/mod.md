# mod

## 职责
`core/repository/` 模块入口，声明持久化仓库 trait 子模块：`workspace_repo`（工作区仓库）、`config_repo`（配置仓库）、`session_repo`（会话仓库 —— 应用层与适配器层共用的会话持久化契约，见 `session_repo.md`）。插件仓库 trait 已于 2026-10-20（PR #40，提交 2cef9a1）迁往 `plugin::registry`（契约）+ `infra::fs_plugin_repo`（实现），原 `plugin_repo.md` 保留为重定向说明。

## 核心类型/函数
无（仅模块声明）。

## 直接依赖
无外部依赖，仅声明子模块。

## 被依赖
- `core::mod`（通过 `pub mod repository` 声明）

## 逻辑流程
无（仅模块声明）。
