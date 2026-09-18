# mod

## 职责
`core/provider/` 模块入口，声明 OJ Provider trait 子模块：`auth`（认证）、`contest`（比赛）、`problem`（题目）、`submission`（提交）、`oj_id`（OJ 身份标识 newtype）、`registry`（Provider 注册中心，含能力集合 `ProviderSet`）。

## 核心类型/函数
无（仅模块声明）。

## 直接依赖
无外部依赖，仅声明子模块。

## 被依赖
- `core::mod`（通过 `pub mod provider` 声明）

## 逻辑流程
无（仅模块声明）。
