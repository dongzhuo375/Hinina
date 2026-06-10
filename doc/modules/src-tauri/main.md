# main

## 职责
应用程序入口点。按顺序初始化所有子系统，构造 `AppContext`，启动 Tauri 桌面窗口。

## 核心类型/函数
- `fn main()` — 程序入口，执行 9 步初始化流程后启动 Tauri App

## 直接依赖
- `hinina_lib::commands`
- `hinina_lib::core::context::AppContext`
- `tauri`（隐式通过 `tauri::Builder` 等）

## 被依赖
- 无 — 顶层入口点，不被其他模块引用

## 逻辑流程
1. **Logger** — 最先初始化，后续步骤可记录日志
2. **ConfigService** — 加载配置，决定后续行为
3. **Storage** — 文件系统根目录
4. **HttpClient** — Reqwest 客户端
5. **EventBus** — 事件总线（纯内存，可较早初始化）
6. **ProviderRegistry** — 注册各 OJ Adapter
7. **WorkspaceManager** — 扫描并恢复工作区
8. **AppContext** — 装配上述所有组件，通过 `AppContext::init()` 阻塞式完成
9. **Tauri App** — 注入 `AppContext` 到 State，在 `setup` 中调用 `commands::register_commands()`
