# mod

## 职责
Commands 模块入口。声明所有子命令模块，并提供 `register_commands()` 函数将全部 Command 注册到 Tauri App 的 invoke handler。

## 核心类型/函数
- `pub fn register_commands(app: &mut tauri::App)` — 注册所有 Command handler 到 Tauri App（TODO：尚未实现具体注册逻辑）

## 直接依赖
- `tauri::Manager`

## 被依赖
- `src-tauri/src/main.rs` — 在 `setup` 闭包中调用 `commands::register_commands(app)`

## 逻辑流程
1. `main.rs` 在 Tauri `setup` 阶段调用 `register_commands()`
2. 该函数负责将各个子模块中的 `#[tauri::command]` 函数注册到 Tauri IPC 系统
3. 当前为占位实现（`todo!()`），后续需逐一注册
