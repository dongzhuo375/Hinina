# 已知问题与待决策项

> 本文档记录代码审查中发现但目前暂不修复的问题。修复时机在「处理时机」栏说明。

---

## 问题 3：AppError 的 `#[derive(Serialize)]` 与 `thiserror` 的序列化行为

**状态**：待验证

**描述**：`AppError` 同时 derive 了 `thiserror::Error` 和 `serde::Serialize`。`thiserror` 的 `#[error("认证错误: {0}")]` 生成 `Display` 实现，而 `Serialize` 序列化的是枚举变体结构（如 `{"Auth": "消息内容"}`）。前端通过 Tauri IPC 收到的 JSON 格式是 `{"Auth": "..."}` 而非 Display 输出 `"认证错误: ..."`。

**处理时机**：前端 Bridge 层实现时，验证 Tauri IPC 对 `AppError` 的实际序列化行为，确认前端 UI 能否正确展示错误信息。如不满足需求，需自定义 `Serialize` 实现。

---

## 问题 9：main.rs 用 `setup` 注册 Command 而非 `invoke_handler`

**状态**：待验证

**描述**：Tauri 2 标准 Command 注册方式是在 Builder 上调用 `.invoke_handler(tauri::generate_handler![...])`。当前使用 `.setup(|app| { commands::register_commands(app); ... })` 手动注册。在 Tauri 2 某些版本中 `setup` 内注册 Command 可能不会生效，或与 `invoke_handler` 行为有差异。

**处理时机**：阶段 6（Tauri Command）填充 Command 实现时，实际测试 IPC 调用是否正常。如 `invoke` 无法路由到 Command，切换为标准 `invoke_handler` 方式。

---

## 问题 10：Service 层 `mod.rs` 仅含注释和 `pub mod error`

**状态**：骨架阶段可接受

**描述**：7 个 Service 模块中，除 `workspace/manager.rs` 外，其余只有 `mod.rs`（注释说明职责）和 `error.rs`（领域错误枚举）。缺少 Service 核心 struct 和公开方法签名。

**处理时机**：阶段 4（Service 层）填充各 Service 业务逻辑时自然解决。

---

## 问题 11：OJType 缺少 `Custom(String)` 变体

**状态**：低优先级

**描述**：当前 `OJType` 枚举只有 `HOJ / QDUOJ / HUSTOJ` 三个变体。未来如果需要支持校内自建 OJ 或其他非标准变体，必须修改枚举定义（Breaking Change）。

**处理时机**：阶段 5 实现第二个 OJ Adapter（QDUOJ）时，评估是否有自定义 OJ 的需求。如有，添加 `Custom(String)` 变体。

---

## 问题 12：PluginManifest 中 `permissions` 使用 `Vec` 而非 `HashSet`

**状态**：低优先级

**描述**：`PluginManifest.permissions: Vec<PluginPermission>` —— 权限检查需要快速 `contains()` 查询，`Vec` 是 O(n) 而 `HashSet` 是 O(1)。此外 `Vec` 允许重复声明同一权限（语义上无意义）。

**处理时机**：v1.0 实现插件运行时（`plugin/runtime/`）时改为 `HashSet<PluginPermission>`。当前 v0.x 仅预留接口，无需立即修改。

---