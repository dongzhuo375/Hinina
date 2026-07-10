# main

## 职责
应用程序入口点。按顺序初始化所有子系统，构造 `AppContext`，启动 Tauri 桌面窗口。

## 核心类型/函数
- `fn main()` — 程序入口，创建 Tokio runtime，阻塞式调用 `AppContext::init()` 后启动 Tauri App

## 直接依赖
- `hinina_lib::commands`
- `hinina_lib::core::context::AppContext`
- `tauri`（隐式通过 `tauri::Builder` 等）
- `tokio::runtime::Runtime`

## 被依赖
- 无 — 顶层入口点，不被其他模块引用

## 逻辑流程
1. 创建 Tokio runtime
2. 阻塞式调用 `AppContext::init(temp_dir)` — 当前使用临时目录，base_dir 在 `init()` 中自动创建
3. 装配 Tauri Builder，注入 `AppContext` 到 State
4. `setup` 中调用 `commands::register_commands()` 注册 IPC
5. 启动 Tauri 桌面应用
