# logger

## 职责
日志追踪模块（基于 Tracing），提供分级日志输出与敏感信息过滤。当前 `init()` 方法为 TODO 占位。

## 核心类型/函数
- **`Logger`** — 日志 struct（单元结构体）
- **`Logger::init()`** — 初始化日志系统，计划配置 tracing-subscriber 的日志级别与输出目标

## 直接依赖
无（当前 stub 未引入外部 crate）

## 被依赖
- `core::context`（`AppContext` 持有 `Arc<Logger>` 并在 `init()` 序列中首先初始化）

## 逻辑流程
`Logger::init()` 将在应用启动时首先调用，配置 tracing-subscriber 设置日志级别（如 debug/info/warn/error）与输出目标（文件/控制台），同时过滤敏感信息（如密码、Token）。
