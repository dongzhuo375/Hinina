# event

## 职责
提供事件订阅入口，负责将宿主 `AppEvent` 转发为 `PluginEvent` 供插件消费。v0.x 为预留接口。

## 核心类型/函数
- 无（v1.0 待实现）

## 直接依赖
- 无

## 被依赖
- `plugin/api/mod.rs` — 通过 `pub mod event` 声明子模块

## 逻辑流程
无（TODO 占位，v1.0 实现）
