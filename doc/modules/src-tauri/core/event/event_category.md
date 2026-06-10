# event_category

## 职责
定义事件类别枚举 `EventCategory`，用于 EventBus 的分类订阅。订阅者可按 `Auth`、`Contest`、`Problem` 等领域精确订阅，或使用 `All` 接收全部事件。

## 核心类型/函数
- **`EventCategory`** — 事件类别枚举：`Auth`, `Contest`, `Problem`, `Submission`, `Workspace`, `System`, `All`

## 直接依赖
无外部依赖。

## 被依赖
- `core::event::event_bus`（EventBus 使用 EventCategory 作为订阅键）

## 逻辑流程
无（纯类型定义）。
