# mod

## 职责
infra 模块的入口，声明并导出所有基础设施子模块（http、storage、cache、logger、repository 实现、provider_registry 实现、audit 只读审计消费者）。子模块清单：`audit`、`cache`、`data_dir`、`fs_config_repo`、`fs_plugin_repo`、`fs_session_repo`、`fs_workspace_repo`、`http`、`logger`、`provider_registry_impl`、`storage`。

## 核心类型/函数
无（仅 `pub mod` 声明，无自定义类型或函数）

## 直接依赖
无（模块声明文件，不引入外部依赖）

## 被依赖
- `core::context`（通过 `use crate::infra::*` 引用各子模块类型）
- infra 内所有子模块（通过父模块路径 `crate::infra::*` 互引用）

## 逻辑流程
无（仅模块层级声明）
