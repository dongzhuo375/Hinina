# mod

## 职责
`core/entity/` 模块入口，声明领域实体子模块：`announcement`（比赛公告）、`config`（应用配置）、`user`（用户）、`contest`（比赛）、`problem`（题目）、`rank`（榜单与题目限制）、`submission`（提交）、`workspace`（工作区）。

## 核心类型/函数
无（仅模块声明）。

## 直接依赖
无外部依赖，仅声明子模块。

## 被依赖
- `core::mod`（通过 `pub mod entity` 声明）

## 逻辑流程
无（仅模块声明）。
