# oj_type

## 职责
定义 OJ 类型标识枚举 `OJType`，列出支持的 OJ 平台（HOJ、QDUOJ、HUSTOJ），用于 Provider 注册与切换时的键值。

## 核心类型/函数
- **`OJType`** — OJ 类型枚举：`HOJ`, `QDUOJ`, `HUSTOJ`

## 直接依赖
- `serde::{Deserialize, Serialize}`

## 被依赖
- `core::provider::registry`（ProviderRegistry 使用 OJType 作为注册/查找键）
- `core::event::app_event`（SystemEvent::OJSwitched 携带 OJType）
- `infra::provider_registry_impl`

## 逻辑流程
无（纯类型定义）。
