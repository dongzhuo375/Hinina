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

# PR5 — 阶段 1 基础设施 Review 结果

> 审查对象：PR#5 `feat(infra): 阶段 1 — 基础设施实现`（分支 `feat/infrastructure`，8 commits，+1386/-47，13 文件变更）
>
> 审查时间：2026-07-09

## 总体评价

| 维度 | 评分 | 说明 |
|------|------|------|
| todo.md 完成度 | 7/10 | 3/4 项优秀完成，AppContext::init 因 `todo!()` 导致应用无法启动 |
| 代码质量 | 6/10 | 结构清晰，但目录穿越防护有安全隐患，HttpClient 缺少 4xx 处理 |
| 工程化 | 5/10 | 无单元测试，仅 `cargo check` 验证，`#[allow]` 掩盖问题 |
| 文档同步 | 10/10 | 5 个模块文档 + Architecture.md 同步完美 |

**合并建议：Request Changes（需修改后合并）**

## todo.md 阶段 1 完成度

| 任务 | 状态 | 备注 |
|------|------|------|
| Logger 初始化 | ✅ 基本完成 | 日志级别自适应、EnvFilter、span 事件均已实现；**"敏感信息过滤"未实现**（见 P5-3） |
| Storage 实现 | ✅ 超额完成 | 要求 4 个方法，实际提供 9 个（额外含 read_to_string / write_string / remove / remove_all / list） |
| HttpClient 封装 | ✅ 完成 | 30s 超时、Cookie Store、UA、5xx 重试、get_json / post_json 便捷方法 |
| AppContext::init() | ⚠️ 部分完成 | 第 7 步 WorkspaceManager 为 `todo!()`，**运行时直接 panic**（见 P5-1） |

## 🔴 严重问题（Blocking）

### P5-1：`AppContext::init()` 中 `todo!()` 导致应用无法启动

**文件**：`src-tauri/src/core/context.rs`

**描述**：第 7 步 WorkspaceManager 使用 `todo!()` 占位，运行时直接 panic。`main.rs` 中 `.expect("Failed to initialize AppContext")` 必然触发，**整个应用无法启动**。`#[allow(unreachable_code, unused_variables)]` 压制了编译器警告，掩盖了此问题。

**建议**：WorkspaceManager 应以空壳/默认状态初始化（如 `Option<WorkspaceManager>` 或 `WorkspaceManager::placeholder()`），而非用 `todo!()` 阻断启动流程。

> ✅ **已修复**（2026-07-10, commit `ddc0f37`）：`workspace_manager` 改为 `Option<Arc<WorkspaceManager>> = None`，应用可正常启动。追踪记录在 `doc/todo.md` 阶段 4.5。

### P5-2：`Storage::resolve()` 目录穿越防护存在安全隐患

**文件**：`src-tauri/src/infra/storage.rs`

**描述**：
- `canonicalize()` 在文件不存在时失败，fallback 到 `full_path.clone()`，此时 `starts_with` 检查可能被 `..` 绕过（如 `foo/../../etc/passwd` 的 clone 版本仍以 base_dir 开头）
- 返回 `full_path`（未规范化）而非 `canonical`，安全检查与实际使用路径不一致
- `resolve()` 与 `is_safe()` 逻辑高度重复但实现不一致，维护成本高

**建议**：使用 `Path::components()` 手动遍历路径组件，拒绝任何 `Component::ParentDir`，不依赖文件系统状态：

```rust
fn resolve(&self, relative_path: &str) -> AppResult<PathBuf> {
    let path = Path::new(relative_path);
    for component in path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(AppError::Io(format!("目录穿越攻击: {}", relative_path)));
        }
    }
    Ok(self.base_dir.join(relative_path))
}
```

> ✅ **已修复**（2026-07-10, commit `ddc0f37`）：已采用建议方案，删除冗余 `is_safe()`，新增 3 个单元测试。

### P5-3：Logger 文档承诺"敏感信息过滤"但未实现

**文件**：`src-tauri/src/infra/logger.rs`

**描述**：doc comment 写了 `敏感字段（password、token、cookie）自动过滤为 ***`，但实现中无任何过滤逻辑。`tracing-subscriber` 的 fmt 层不提供字段级过滤。

**建议**：修正文档，删除过滤承诺；或在后续阶段实现自定义 `Visit` / `MakeWriter` 进行字段级过滤。

> ✅ **已修复**（2026-07-10, commit `ddc0f37`）：已删除模块 doc 和 init() doc 中的过滤承诺。

## 🟡 中等问题（建议修复）

### P5-4：HttpClient 缺少 4xx 错误处理

**文件**：`src-tauri/src/infra/http.rs`

**描述**：`get_json` / `post_json` 不检查 HTTP 状态码。服务器返回 401/403/404 时，代码尝试将错误页面反序列化为 JSON，产生令人困惑的 "JSON 反序列化失败" 错误。

**建议**：在 `retry_get` 和 `post_json` 中检查 `response.status()`，对 4xx 返回对应的 `AppError`。

> ✅ **已修复**（2026-07-10, commit `beb0130`）：`retry_get` 对 4xx 直接报错不重试，`post_json` 检查 `is_success()`，均含状态码信息。

### P5-5：`main.rs` 使用 `temp_dir` 但未创建该目录

**文件**：`src-tauri/src/main.rs`

**描述**：`std::env::temp_dir().join("hinina")` 若不存在，后续 Storage 的 `base_dir.canonicalize()` 会失败，`is_safe()` 返回 `false`，导致 `exists()` 等方法行为异常。

**建议**：在 `AppContext::init` 或 `Storage::new` 中 `fs::create_dir_all(&base_dir)`。

> ✅ **已修复**（2026-07-10, commit `beb0130`）：`AppContext::init()` 和 `Storage::new()` 均添加了 `create_dir_all`。

### P5-6：`ProviderRegistryImpl::new(OJType::HOJ)` 未注册任何 Provider

**文件**：`src-tauri/src/core/context.rs`

**描述**：仅设置 `current = HOJ`，四个 provider map 均为空。任何 `get_auth(&OJType::HOJ)` 调用返回 `ProviderNotFound`。阶段 1 可理解（HOJ Adapter 在阶段 5），但应在注释中明确说明。

## 🟢 轻微问题（可选优化）

### P5-7：`Storage::list()` 返回绝对路径，与模块"相对路径"语义不一致

**建议**：返回相对 base_dir 的相对路径，避免泄露 base_dir 信息。

> ✅ **已修复**（2026-07-10, commit `3922129`）：`list()` 使用 `strip_prefix` 返回相对路径。

### P5-8：重试延迟计算重复

**文件**：`src-tauri/src/infra/http.rs`

**描述**：`RETRY_BASE_DELAY_MS * 2u64.pow(attempt)` 出现两次，可提取为 `fn retry_delay(attempt: u32) -> Duration`。

> ✅ **已修复**（2026-07-10, commit `beb0130`）：提取 `const fn retry_delay()` 和 `status_error()` 辅助函数。

## 工程化不足

### P5-9：无单元测试

**描述**：Storage 路径解析、HttpClient 重试逻辑均为适合单元测试的纯逻辑，但 PR 中无任何测试代码。建议至少为 `Storage::resolve` 目录穿越防护添加测试。

### P5-10：`cargo check` 不等于运行时验证

**描述**：PR 描述称"验证：cargo check 通过"，但 `cargo check` 不检查运行时行为。`todo!()` 能通过 check 但运行时崩溃。建议后续 PR 至少执行 `cargo run` 确认应用能启动。

> ✅ **已修复**（2026-07-10）：PR5 修正后执行了 `cargo build` + `cargo test` 双重验证，共 3 个 Storage 单元测试通过。