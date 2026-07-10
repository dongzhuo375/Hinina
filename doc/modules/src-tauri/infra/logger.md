# logger

## 职责
日志追踪模块（基于 Tracing），提供分级日志输出，支持 debug/release 自适应级别与 RUST_LOG 覆盖。

## 核心类型/函数
- **`Logger`** — 日志 struct（单元结构体）
- **`Logger::init()`** — 初始化日志系统，配置 tracing-subscriber：
  - debug 构建默认 `debug` 级别，release 默认 `info`，可通过 `RUST_LOG` 环境变量覆盖
  - 输出 span enter/exit、target、线程 ID、文件名、行号到 stderr

## 直接依赖
- `tracing_subscriber::{fmt, EnvFilter}`

## 被依赖
- `core::context`（`AppContext` 持有 `Arc<Logger>` 并在 `init()` 序列中首先初始化）

## 逻辑流程
`Logger::init()` 在应用启动时首先调用，根据编译模式设定默认日志级别，支持 `RUST_LOG` 环境变量动态覆盖。
