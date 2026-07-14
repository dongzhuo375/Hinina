# 已知问题与待决策项

> 本文档记录代码审查中发现但目前暂不修复的问题。修复时机在「处理时机」栏说明。

---

## 问题 3：AppError 的 `#[derive(Serialize)]` 与 `thiserror` 的序列化行为

**状态**：✅ 已验证（阶段 6）

**描述**：`AppError` 同时 derive 了 `thiserror::Error` 和 `serde::Serialize`。`#[derive(Serialize)]` 序列化为 `{"Auth": "消息内容"}` 格式，Tauri IPC 可直接传递此 JSON。前端收到 `Err({ Auth: "消息" })` 可通过字段名判断错误类别，通过值获取用户可读消息。当前格式满足需求，无需自定义 `Serialize`。

**处理时机**：已确认无需修改。

---

## 问题 9：main.rs 用 `setup` 注册 Command 而非 `invoke_handler`

**状态**：✅ 已修复（阶段 6）

**描述**：`main.rs` 已从 `setup` 手动注册改为标准 `tauri::generate_handler!` + `invoke_handler()` 方式。编译通过，IPC 路由正常。

**处理时机**：已完成。

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

<!-- ════════════════════════════════════════════════════════════════ -->
<!-- PR #7 Review（阶段 2 — 存储抽象层）以下为新增内容             -->
<!-- ════════════════════════════════════════════════════════════════ -->

## 问题 P4：测试临时目录未在测试结束后清理

**来源**：PR #7 Review

**状态**：低优先级

**文件**：`src-tauri/src/infra/tests/fs_workspace_repo_tests.rs`、`fs_config_repo_tests.rs`

**描述**：测试用 `std::env::temp_dir()` 创建目录，仅在测试开始时 `remove_dir_all`，结束后不清理。长期运行测试会堆积临时文件。

**建议修复**：在每个测试末尾清理，或实现 `Drop` guard 自动清理。

**处理时机**：后续测试规范化时统一处理。

---

## 问题 P6：`FsConfigRepository` 的 `config_path` 错误类型语义不精确

**来源**：PR #7 Review

**状态**：低优先级

**文件**：`src-tauri/src/infra/fs_config_repo.rs`

**描述**：`config_path` 直接传给 `Storage::read_to_string`，若路径含 `..`，`Storage::resolve` 会返回 `AppError::Io` 而非 `AppError::Config`，错误语义不够精确。由于 `config_path` 由开发者控制（不接受用户输入），实际风险极低。

**处理时机**：无需主动修复，记录备查。

---

<!-- ════════════════════════════════════════════════════════════════ -->
<!-- PR #8 Review（阶段 3 — 领域核心）以下为新增内容               -->
<!-- ════════════════════════════════════════════════════════════════ -->

## 问题 P9：`subscribe` 与 `publish` 之间存在 ID 分配与写入的 TOCTOU 窗口

**来源**：PR #8 Review

**状态**：低优先级，记录备查

**文件**：`src-tauri/src/core/event/event_bus.rs` — `subscribe()` 方法

**描述**：`subscribe` 先释放 `next_id` 的 Mutex，再获取 `subscribers` 的写锁。两个线程同时 subscribe 时，线程 A 可能先拿到较小 ID 但后写入 subscribers，导致 ID 递增顺序与实际注册顺序不一致。当前场景无害（ID 唯一性由 Mutex 保证），但如果未来需要"ID 递增 = 注册顺序"语义，会有问题。

**建议修复**：如需严格顺序，用一把锁同时保护 `next_id` 和 `subscribers`。

**处理时机**：当前可接受，记录备查。

---

## 问题 P11：`utc_now_secs` 的 `unwrap_or_default()` 在时钟回拨时静默返回 0

**来源**：PR #8 Review

**状态**：低优先级，记录备查

**文件**：`src-tauri/src/core/entity/workspace.rs` — `utc_now_secs()` 函数

**描述**：`duration_since(UNIX_EPOCH).unwrap_or_default()` 在系统时间早于 UNIX_EPOCH（极端情况）时返回 `Duration::ZERO`，即时间戳为 0。`unwrap_or_default()` 隐藏了错误，实际中几乎不可能发生。

**处理时机**：无需主动修复，记录备查。

---

<!-- ════════════════════════════════════════════════════════════════ -->
<!-- PR #9 Review（阶段 4 — 服务层）以下为新增内容                   -->
<!-- ════════════════════════════════════════════════════════════════ -->

## 问题 P20：`random_hex_suffix()` 使用 `RandomState` 非标准随机方式

**来源**：PR #9 Review

**状态**：低优先级，记录备查

**文件**：`src-tauri/src/core/entity/workspace.rs` — `random_hex_suffix()`

**描述**：使用 `RandomState::new().build_hasher().finish()` 生成随机后缀。`finish()` 的低 16 位分布可能不均匀，不是标准的随机数生成方式。对于 4 位十六进制防碰撞后缀，当前实现风险可接受，但不够规范。

**建议修复**：如后续引入 `rand` crate，替换为标准随机数生成；或使用 `SystemTime` 的纳秒部分取模。

**处理时机**：无需主动修复，记录备查。

---

<!-- ════════════════════════════════════════════════════════════════ -->
<!-- PR #10 Review（阶段 5 — HOJ Adapter）以下为新增内容            -->
<!-- ════════════════════════════════════════════════════════════════ -->

## 问题 P23：登录密码未做 MD5 散列

**来源**：PR #10 Review

**状态**：✅ 已修复

**文件**：`src-tauri/src/adapter/hoj/mod.rs` — `login()`

**描述**：HOJ API 文档（`doc/HOJ/HOJ-API-Documentation.md` 第 117 行）明确写 "密码使用 MD5（非加盐）传递给服务端比对"。`types.rs` 中 `LoginRequest` 的注释也写了 "密码提交前需 MD5 散列"。但 `login()` 实现直接发送明文密码：

```rust
let body = LoginRequest {
    username: username.to_string(),
    password: password.to_string(),  // 明文，应为 MD5(password)
};
```

HOJ 服务端比对的是 MD5 哈希值，发送明文密码会导致**登录必然失败**。

**建议修复**：在 `login()` 中对 password 做 MD5 散列后再放入请求体。可引入 `md5` crate 或使用 `sha2` + 手动实现 MD5。

**处理时机**：合并前必须修复。

---

## 问题 P24：评测状态码映射与 API 文档不一致

**来源**：PR #10 Review

**状态**：✅ 已修复

**文件**：`src-tauri/src/adapter/hoj/types.rs` — `map_status()`

**描述**：API 文档（第 812-829 行）定义了 16 个状态码（0-15），但 `map_status()` 只映射了 0-11，且从状态码 8 开始全部偏移了一位：

| 文档状态码 | 文档含义 | 代码实际映射 | 正确映射 |
|-----------|---------|------------|---------|
| 8 | Output Limit Exceeded (OLE) | RuntimeError ❌ | WrongAnswer（无 OLE 枚举） |
| 9 | Runtime Error (RE) | WrongAnswer ❌ | RuntimeError |
| 10 | System Error (SE) | WrongAnswer ❌ | WrongAnswer 或 Unknown |
| 11 | Remote Judge Error (RJE) | WrongAnswer ❌ | Unknown |
| 12 | Submitted Failed (SF) | Unknown ❌ | WrongAnswer |
| 13 | Partially Accepted (PA) | Unknown ❌ | Accepted 或 WrongAnswer |
| 14 | Submit Frequent Limit (FREQ) | Unknown ❌ | Unknown |
| 15 | Unknown Error (UE) | Unknown ❌ | Unknown |

根因：代码注释将状态码 8 标注为 RuntimeError，但文档中 8 是 OLE、9 才是 RE，整体偏移了一位。

**建议修复**：按 API 文档重新映射全部 0-15 状态码。`JudgementStatus` 枚举可考虑新增 `OutputLimitExceeded` 变体，或将 OLE 归入 WrongAnswer。

**处理时机**：合并前必须修复。

---

## 问题 P25：`get_judgement` 非终态返回 `Err` 而非 `Ok(Running)`

**来源**：PR #10 Review

**状态**：✅ 已修复

**文件**：`src-tauri/src/adapter/hoj/mod.rs` — `get_judgement()`

**描述**：评测进行中（Pending/Judging）时，`get_judgement` 返回 `Err(AppError::Submission("评测进行中"))`。但上层 `SubmissionService::poll_judgement` 的轮询逻辑是：`Ok` 表示拿到结果，`Err` 表示查询失败会重试。评测进行中不是错误，返回 `Err` 会导致上层日志不断打印 "评测查询失败，等待重试" 警告，语义错误且日志误导。

```rust
// 当前实现（语义错误）：
if matches!(status, JudgementStatus::Running) {
    return Err(AppError::Submission(format!("评测进行中: {}", submission_id)));
}
```

**建议修复**：返回 `Ok(JudgementResult { status: JudgementStatus::Running, .. })`，让上层 `SubmissionService::poll_judgement` 根据 `JudgementStatus::Running` 判断是否继续轮询。需同步修改 `SubmissionService` 的轮询条件。

**处理时机**：合并前修复。

---

## 问题 P26：`login()` 中 `User.token` 赋值为 `uid` 而非 JWT token

**来源**：PR #10 Review

**状态**：✅ 已修复

**文件**：`src-tauri/src/adapter/hoj/mod.rs` — `login()`

**描述**：`login()` 从响应头正确提取了 JWT token 并通过 `set_token()` 存到 adapter 内部，但返回给 Service 层的 `User.token` 却赋值为 `uid`（用户 UUID）：

```rust
let uid = user_info.uid;
Ok(User {
    id: uid.clone(),
    username: user_info.username,
    token: uid,  // ← 应为 JWT token，不是 uid
})
```

`AuthService` 会将 `User.token` 持久化到 session 文件。后续恢复 session 时拿到的 token 是 uid 而非 JWT，无法用于 API 认证。

**建议修复**：`token` 字段应填入从响应头提取的 JWT token（即 `self.get_token()` 的值），而非 `uid`。

**处理时机**：合并前修复。

---

## 问题 P27：`list_contests` 未传分页参数

**来源**：PR #10 Review

**状态**：✅ 已修复

**文件**：`src-tauri/src/adapter/hoj/mod.rs` — `list_contests()`

**描述**：`list_contests` 请求 `/api/get-contest-list` 时未传 `limit` 和 `currentPage` 参数。HOJ 默认分页可能只返回前 10-20 条比赛，用户无法看到完整比赛列表。

```rust
let url = self.api_url("/get-contest-list");
// 未传 ?limit=100&currentPage=1
```

**建议修复**：传入足够大的 `limit`（如 1000）或循环分页获取全部比赛。

**处理时机**：合并前修复。

---

## 问题 P28：`get_judgement` 绕过 HttpClient 重试逻辑

**来源**：PR #10 Review

**状态**：✅ 已修复（PR #10 Review 修正）

**文件**：`src-tauri/src/adapter/hoj/mod.rs` — `get_judgement()`

**描述**：`get_judgement` 直接使用 `self.http.client().get(url).send()` 发送请求，绕过了 `HttpClient::get_json()` 的 5xx 自动重试逻辑。虽然此端点是 `@AnonApi`（无需认证），但其他方法（如 `list_contests`、`get_contest`）都通过 `http.get_json()` 统一走重试路径。行为不一致。

**建议修复**：统一使用 `self.http.get_json()` 发送 GET 请求。

**处理时机**：✅ 已修复。改为调用 `self.http.get_json()` 统一走重试逻辑。

---

## 问题 P29：`HOJError` 枚举定义但从未使用

**来源**：PR #10 Review

**状态**：✅ 已修复（PR #10 Review 修正）

**文件**：`src-tauri/src/adapter/hoj/error.rs`

**描述**：`error.rs` 定义了 5 变体的 `HOJError` 枚举（ApiError / HttpError / JsonError / Unauthorized / UnknownStatus），但 `mod.rs` 中所有错误都直接使用 `AppError` 变体。`HOJError` 是死代码，编译器未报 warning 仅因为它是 `pub` 的。

**建议修复**：要么删除 `HOJError`（直接用 `AppError`），要么在 Adapter 内部使用 `HOJError` 后通过 `From<HOJError> for AppError` 转换。

**处理时机**：✅ 已修复。新增 `From<HOJError> for AppError` 转换，可通过 `?` 操作符使用。

---

## 问题 P30：`parse_time` 未处理时区和非零填充格式

**来源**：PR #10 Review

**状态**：低优先级，记录备查

**文件**：`src-tauri/src/adapter/hoj/mod.rs` — `parse_time()`

**描述**：手写的日期解析算法有两个潜在问题：

1. **时区**：HOJ 返回的 `startTime` / `endTime` 可能是服务器本地时间而非 UTC。代码按 UTC 解析，如果服务端非 UTC 时区，时间戳会有偏差。
2. **非零填充**：`s.len() < 19` 检查后按固定字节偏移取值（如 `d(0)` 取前两位年份）。如果格式为 `2024-1-1 8:00:00`（非零填充），长度不足 19 会直接返回 0。

**建议修复**：加注释说明假设输入为零填充 ISO 格式（`YYYY-MM-DDTHH:MM:SS`）。时区问题需实际测试 HOJ 返回值后确认。

**处理时机**：无需主动修复，记录备查。实际对接 HOJ 时验证。

---

## 问题 P31：`parse_samples` HTML 解析过于简单

**来源**：PR #10 Review

**状态**：✅ 已修复（PR #10 Review 修正）

**文件**：`src-tauri/src/adapter/hoj/mod.rs` — `parse_samples()`

**描述**：仅匹配 `<pre>...</pre>` 固定标签，无法处理：
- 带属性的 `<pre>` 标签（如 `<pre class="input">`）
- 其他 HTML 实体（如 `&nbsp;`、`&#39;`）
- `<br>` 换行标签

对于 MVP 可接受，但如果 HOJ 题目样例格式变化，样例提取会失败。

**建议修复**：如后续需要更完善的 HTML 解析，引入 `scraper` 或 `html5ever` crate。

**处理时机**：✅ 已修复。支持 `<pre>` 标签属性（`<pre class="input">`）、`<br>` → 换行、`&nbsp;` 实体。

---

## 问题 P32：HOJ Adapter 零单元测试

**来源**：PR #10 Review

**状态**：✅ 已修复（阶段 6）

**文件**：`src-tauri/src/adapter/hoj/tests/types_tests.rs`、`mod_tests.rs`

**描述**：已为 4 个纯函数补充单元测试：
- `map_status()` — 全部 16 个状态码映射 + 负数/越界边界
- `is_terminal_status()` — 终态判断边界
- `parse_time()` — 正常格式、闰年、短字符串、空字符串
- `parse_samples()` — 空/mono/dual/multi pre 块、属性标签、HTML 实体、`<br>` 换行

**处理时机**：已完成。

---

<!-- ════════════════════════════════════════════════════════════════ -->
<!-- PR #11 Review（阶段 6 — Tauri Command）以下为新增内容          -->
<!-- ════════════════════════════════════════════════════════════════ -->

## 问题 P33：`workspace_cmd` 中 `root_path` 为硬编码占位符

**来源**：PR #11 Review

**状态**：✅ 已修复（PR #11 Review）

**描述**：`load_workspace` 和 `switch_workspace` 均传入硬编码字符串作为 `root_path`：
```rust
let root_path = format!("workspaces/{{id}}"); // 占位，实际路径由 create 内部填充
```

注释声称"实际路径由 create 内部填充"，但 `WorkspaceManager::create` 只是将该值原样存入 `WorkspaceMeta.root_path`，并未做任何路径推导。而 `FsWorkspaceRepository` 实际通过 `workspace_id`（而非 `root_path`）定位文件目录。因此 `root_path` 字段当前是语义模糊的死值——既不用于文件定位，也不是真实文件系统路径。

**建议修复**：二选一：
1. 从 `Storage::base_dir` 推导真实路径（如 `{base_dir}/workspaces/{workspace_id}`），使 `root_path` 字段具有实际意义。
2. 在 `WorkspaceManager` 层面移除 `root_path` 参数（因为 repo 已用 `workspace_id` 定位），同步清理 `Workspace` 实体中的 `root_path` 字段或将其标记为废弃。

**处理时机**：✅ 已修复。改为传入 `""` 并添加注释说明 `root_path` 为语义性字段，实际文件定位由 `FsWorkspaceRepository` 通过 `workspace_id` 完成。

---

## 问题 P34：`auth_cmd::login` 中 OJType 字符串解析硬编码

**来源**：PR #11 Review

**状态**：低优先级，记录备查

**文件**：`src-tauri/src/commands/auth_cmd.rs` — `login()`

**描述**：`login` 命令中通过 `match ot.to_uppercase().as_str()` 手动将字符串映射到 `OJType` 枚举变体：

```rust
"HOJ" => OJType::HOJ,
"QDUOJ" => OJType::QDUOJ,
"HUSTOJ" => OJType::HUSTOJ,
```

`OJType` 已 derive `Deserialize`，可直接反序列化。当前硬编码方式在新增 OJ 类型（如问题 #11 提到的 `Custom(String)` 变体）时需要同步修改此处，违反开闭原则。

**建议修复**：为 `OJType` 实现 `FromStr` trait，或提供 `OJType::from_str_ignore_case(s: &str) -> Option<Self>` 方法，集中字符串到枚举的映射逻辑。

**处理时机**：实现第二个 OJ Adapter（QDUOJ）时一并处理。

---

## 问题 P35：`get_session` 返回的 `User.id` 为空字符串

**来源**：PR #11 Review

**状态**：✅ 已修复（PR #11 Review）

**描述**：`get_session` 从本地 `Session` 恢复 `User` 时，`id` 字段被设为空字符串。根本原因：`Session` 结构体只有 `username`、`token`、`oj_type` 三个字段，不包含 `user_id`。`AuthService::login` 在构建 `Session` 时也未保存 `user.id`。

已修复：
1. `Session` 结构体增加 `user_id: String` 字段，带 `#[serde(default)]` 兼容旧版 session 文件。
2. `AuthService::login` 构建 `Session` 时填入 `user.id`。
3. `get_session` 恢复时用 `session.user_id` 填充 `User.id`。

---

## 问题 P36：`load_workspace` 始终 `create` 而非恢复已有工作区

**来源**：PR #11 Review

**状态**：✅ 已修复（阶段 7）

**文件**：`src-tauri/src/commands/workspace_cmd.rs` — `load_workspace()`

**描述**：命令名为 `load_workspace`，但实际调用的是 `WorkspaceManager::create()`。每次打开同一题目都会创建一个全新的 Workspace（生成新的 `workspace_id`，`files` 为空），而非恢复已有的工作区。对于"重新打开之前做过的题目"场景，用户之前的代码不会自动恢复。

**已修复**：阶段 7 实现了 `WorkspaceManager::find_or_create(contest_id, problem_id)` 方法——扫描所有已有工作区元数据按 `contest_id` + `problem_id` 匹配，命中则 `load`（恢复代码），未命中则 `create`。`load_workspace` Command 已改为调用 `find_or_create`。`WorkspaceRepository` 新增 `list_workspace_ids` 方法支持扫描。

---

## 问题 P37：`todo.md` 阶段 6 任务未勾选

**来源**：PR #11 Review

**状态**：✅ 已修复（PR #11 Review）

**文件**：`doc/todo.md` 第 79-86 行

**描述**：PR #11 声明"阶段 6 已完成"，`todo.md` 顶部状态行也已更新为"阶段 6 已完成，进入阶段 7"。但阶段 6 的 8 项任务复选框仍全部为 `- [ ]` 未勾选状态，与实际完成情况不符。

**处理时机**：✅ 已修复。8 项全部改为 `- [x]`。

---

## 问题 P38：`login` 命令未知 OJ 类型时的冗余 `set_current_oj` 调用

**来源**：PR #11 Review

**状态**：✅ 已修复（PR #11 Review）

**文件**：`src-tauri/src/commands/auth_cmd.rs` — `login()`

**描述**：当传入未知 OJ 类型字符串时，原代码先 warn 并回退到 `ctx.provider_registry.current_oj()`，然后执行 `set_current_oj(parsed)` 把同一个值设回——冗余操作。

**处理时机**：✅ 已修复。未知 OJ 类型分支改为 `return ctx.auth.login(...).await` 提前进入登录流程，跳过无意义的 `set_current_oj`。

---

## 问题 P39：`main.rs` 中 Tokio runtime 生命周期与 auto-save 冲突风险

**来源**：PR #11 Review

**状态**：✅ 已修复（阶段 7）

**文件**：`src-tauri/src/main.rs`；`src-tauri/src/commands/workspace_cmd.rs`

**描述**：`main.rs` 手动创建 `tokio::runtime::Runtime` 并通过 `rt.block_on()` 初始化 `AppContext`。但 `rt` 变量在 `block_on` 返回后即离开作用域被 drop。如果在初始化阶段启动了 `WorkspaceManager::start_auto_save()`，其内部的 `tokio::spawn` 任务绑定在这个 runtime 上，runtime 被 drop 后 auto-save 任务会被取消。

**已修复**：阶段 7 将 auto-save 改为懒启动——在 `load_workspace` Command 首次被调用时通过 `tokio::spawn` 启动。此时执行上下文已在 Tauri 的 async runtime 上，auto-save 生命周期与 Tauri App 一致，不会被提前 drop。使用 `static AtomicBool` 确保仅启动一次。

---

## 问题 P40：Command 层零测试

**来源**：PR #11 Review

**状态**：中等优先级

**文件**：`src-tauri/src/commands/`（全部 7 个模块）

**描述**：7 个 Command 模块、18 个 IPC 端点，没有任何单元测试或集成测试。虽然 Command 是 Service 的薄封装，但以下场景存在 Command 层独有的逻辑，值得测试：

1. **`auth_cmd::login` 的 `oj_type` 参数解析**：`Option<String>` 的各种输入（`None`、合法值、非法值、大小写混合）。
2. **`workspace_cmd` 的 `workspace_manager` 为 `None` 时的降级路径**：`load_workspace` 应返回 `Err`，`current_workspace` 应返回 `Ok(None)`。
3. **`config_cmd::update_config` 的闭包整体替换语义**。

**建议修复**：可通过 Tauri 的 `test::mock_app()` 构建 mock 上下文进行测试，或提取 Command 内部逻辑为可测试的纯函数。

**处理时机**：阶段 7 前端对接前补充关键路径测试。

---

## 问题 P41：`map_status` 中 PA（Partially Accepted）→ Accepted 的映射策略

**来源**：PR #11 Review

**状态**：低优先级，待讨论

**文件**：`src-tauri/src/adapter/hoj/types.rs` — `map_status()`；测试文件 `tests/types_tests.rs`

**描述**：状态码 13（Partially Accepted）被映射为 `JudgementStatus::Accepted`。测试注释标注"保守映射"但未说明决策理由。对于 XCPC 竞赛场景，部分通过通常意味着该题未完全通过（有错误的测试点），映射为 `Accepted` 可能误导用户认为题目已完全通过。

**建议修复**：根据实际业务语义讨论：
- 若 HOJ 的 PA 表示"获得部分分数但题目状态为通过"→ 保持 `Accepted`，在 `JudgementResult` 中补充分数字段。
- 若 PA 表示"部分通过但整体未通过"→ 改为 `WrongAnswer` 或新增 `PartiallyAccepted` 枚举变体。

**处理时机**：实际对接 HOJ 测试后根据业务需求确认。

---

## 问题 P42：Bridge 层 IPC 命令名与 Rust 端注册名完全不匹配（致命）

**来源**：PR #12 Review

**状态**：🔴 致命，阻断合并

**文件**：`src/bridge/*.bridge.ts`（全部 7 个模块）；`src-tauri/src/main.rs` — `generate_handler!`

**描述**：前端 Bridge 层所有 `ipcInvoke` 调用使用自定义冒号格式命令名（如 `auth:login`、`problem:get`、`submission:submit`、`workspace:load`），但 Rust 端 `#[tauri::command]` 注册的是 snake_case 函数名（如 `login`、`get_problem`、`submit_code`、`load_workspace`）。Tauri 2 默认以函数名作为 invoke 命令名，项目中没有任何全局重命名机制。

完整不匹配清单：

| Bridge 调用 | Rust 实际注册名 |
|---|---|
| `auth:login` | `login` |
| `auth:logout` | `logout` |
| `auth:get_session` | `get_session` |
| `contest:load_configured` | `load_configured_contest` |
| `problem:get` | `get_problem` |
| `problem:list` | `list_problems` |
| `submission:submit` | `submit_code` |
| `submission:get_judgement` | `get_judgement` |
| `workspace:load` | `load_workspace` |
| `workspace:save` | `save_workspace` |
| `workspace:current` | `current_workspace` |

**后果**：前端所有 IPC 调用都会在运行时报 `command not found`，整个应用完全不可用。PR 描述声称 `npm run tauri dev` 启动正常，但启动成功 ≠ 功能可用——表明从未进行过实际的前后端联调。

**建议修复**（二选一）：
1. 修改 Bridge 层使用真实函数名（如 `login`、`get_problem`、`submit_code`）。
2. 在 Rust 端为每个 command 函数添加显式命名：`#[tauri::command(name = "auth:login")]`。

**处理时机**：合并 PR #12 前必须修复。

---

## 问题 P43：Rust 实体 serde 字段名（snake_case）与前端 TS 类型（camelCase）不匹配（致命）

**来源**：PR #12 Review

**状态**：🔴 致命，阻断合并

**文件**：
- `src-tauri/src/core/entity/contest.rs` — `Contest`, `ContestProblem`
- `src-tauri/src/core/entity/problem.rs` — `Problem`, `Sample`
- `src-tauri/src/core/entity/workspace.rs` — `Workspace`
- `src-tauri/src/core/entity/submission.rs` — `JudgementResult`
- `src-tauri/src/core/entity/user.rs` — `User`
- `src/types/*.ts`（对应前端类型）

**描述**：Rust 实体结构体没有 `#[serde(rename_all = "camelCase")]`，Tauri IPC 返回的 JSON 使用 snake_case 字段名。但前端 TS 类型全部定义为 camelCase。所有多词字段在反序列化后均为 `undefined`。

不匹配字段清单（部分）：

| Rust 字段 (snake_case) | TS 字段 (camelCase) |
|---|---|
| `contest_id` | `contestId` |
| `problem_id` | `problemId` |
| `root_path` | `rootPath` |
| `is_dirty` | `isDirty` |
| `created_at` / `updated_at` | `createdAt` / `updatedAt` |
| `start_time` / `end_time` | `startTime` / `endTime` |
| `contest_type` | `contestType` |
| `display_id` / `display_title` | `displayId` / `displayTitle` |
| `time_limit` / `memory_limit` | `timeLimit` / `memoryLimit` |
| `input_description` / `output_description` | `inputDescription` / `outputDescription` |
| `time_ms` / `memory_kb` | `timeMs` / `memoryKb` |

**后果**：即使修复 P42，前端拿到的数据所有多词字段都是 `undefined`，页面渲染全部异常（题目无时间/内存限制、比赛无时间、工作区无法恢复代码等）。

**建议修复**：在所有跨 IPC 边界的 Rust 实体上添加 `#[serde(rename_all = "camelCase")]`。

**处理时机**：合并 PR #12 前必须修复。

---

## 问题 P44：前端编辑器代码未同步到后端 Workspace（数据丢失风险）

**来源**：PR #12 Review

**状态**：🔴 高优先级

**文件**：`src/stores/workspaceStore.ts` — `updateCode()`；`src-tauri/src/service/workspace/manager.rs` — `update_file()`

**描述**：`workspaceStore.updateCode()` 只更新了前端 store 的 `this.code` 和 `this.isDirty`，没有调用任何 IPC 将代码同步到 Rust 端的 WorkspaceManager 内存。后端 `WorkspaceManager::update_file()` 是真正写入内存+磁盘的方法，但前端从未调用它（也没有对应的 Bridge/Command 端点）。

当前数据流断裂：
1. 用户在 Monaco 编辑器打字 → `updateCode` 仅更新前端 `state.code`
2. 后端 auto-save 运行，但后端 Workspace 的 `files` 始终为空
3. 用户切换题目 → `saveWorkspace()` 调用后端 `save`，但后端 Workspace 没有代码可存
4. **用户代码丢失**

**建议修复**：
1. 新增 `workspace:update_file` Command + Bridge 端点。
2. `updateCode` 在防抖后（或切换题目前）调用该端点将代码推送到后端。

**处理时机**：合并 PR #12 前必须修复，否则核心功能不可用。

---

## 问题 P45：`openProblem` 传给后端的 `problemId` 语义不一致

**来源**：PR #12 Review

**状态**：🟡 中优先级

**文件**：`src/components/problem/ProblemSidebar.vue`；`src-tauri/src/adapter/hoj/mod.rs` — `get_problem()`

**描述**：`ProblemSidebar` emit 的是 `ContestProblem.problemId`（HOJ 的 pid 数字字符串），但 HOJ `get_problem` 的实际 API 参数是 `displayId`（如 "A"、"B"）：

```rust
let url = self.api_url(&format!(
    "/get-contest-problem-details?cid={}&displayId={}",
    contest_id, problem_id  // ← 期望 displayId，实际收到 pid
));
```

前端 `ContestView.openProblem` 将 `problemId`（pid）传给 `problem.openProblem(contestId, problemId)`，最终到达 HOJ API 的 `displayId` 参数位。**题目详情请求会失败或返回错误题目。**

**建议修复**：
- 方案 A：`ProblemSidebar` emit `displayId` 用于获取题目详情。
- 方案 B：后端 `get_problem` 区分 displayId 和 pid，使用不同参数。

**处理时机**：合并 PR #12 前修复。

---

## 问题 P46：提交后无评测轮询，状态永远停留在 Pending

**来源**：PR #12 Review

**状态**：🟡 中优先级

**文件**：`src/views/ContestView.vue` — `handleSubmit()`；`src/stores/submissionStore.ts` — `pollResult()`

**描述**：`ContestView.handleSubmit` 调用 `submission.submitCode()` 后，提交记录状态永远停留在 `Pending`。`submissionStore` 有 `pollResult` 方法但从未被调用，前端没有轮询机制。

**建议修复**：提交成功后启动定时轮询（`setInterval` + `pollResult`），直到状态变为终态（Accepted/WA/TLE 等），然后停止轮询。轮询参数应从 Config 读取（`oj.poll_interval_secs`）。

**处理时机**：合并 PR #12 前修复。

---

## 问题 P47：`AppHeader` 比赛倒计时不会自动刷新

**来源**：PR #12 Review

**状态**：🟡 低优先级

**文件**：`src/components/layout/AppHeader.vue` — `timeStatus` computed

**描述**：`timeStatus` 是 `computed`，依赖 `contest.contest` 的静态字段，不会随时间推移自动更新。比赛进行中剩余时间会"冻结"在页面加载时的值，用户看到的倒计时始终不变。

**建议修复**：添加 `setInterval`（每秒）触发响应式更新，如维护一个 `now` ref 并在 computed 中引用。

**处理时机**：阶段 8 或合并前修复。

---

## 问题 P48：P39 修复的 `AtomicBool` 一次性标记不可重置

**来源**：PR #12 Review

**状态**：🟡 低优先级

**文件**：`src-tauri/src/commands/workspace_cmd.rs` — `start_auto_save_if_needed()`

**描述**：`start_auto_save_if_needed` 使用 `static STARTED: AtomicBool` 做一次性懒启动。如果用户通过 `update_config` 关闭了 `auto_save` 再重新打开，这个 static 标记不会重置，auto-save 永远无法重启。此外 `WorkspaceManager` 的 `auto_save_running` 标志实际上从未被读取（auto-save 循环不检查它），是死代码。

**建议修复**：
1. 将 auto-save 启动状态改为 `AppContext` 或 `WorkspaceManager` 上的实例字段（非 static），随配置变更可重置。
2. 移除未使用的 `auto_save_running` 字段，或在 auto-save 循环中检查它以支持优雅停止。

**处理时机**：阶段 8。

---

## 问题 P49：`ProblemStatement` 使用 `v-html` 渲染远端 HTML（XSS 风险）

**来源**：PR #12 Review

**状态**：🟡 低优先级

**文件**：`src/components/problem/ProblemStatement.vue`

**描述**：`v-html="problem.description"` 直接渲染 HOJ 返回的 HTML，存在 XSS 风险。竞赛场景下 OJ 服务端可信度较高，风险相对较低，但如果未来支持多 OJ 或用户自定义题目源，风险会增大。

**建议修复**：引入 DOMPurify 等 HTML sanitize 库对远端 HTML 做过滤后再渲染。

**处理时机**：阶段 8 或支持多 OJ 前处理。

---

## 问题 P50：`Contest` 实体 `problems: Vec<String>` 字段始终为空

**来源**：PR #12 Review

**状态**：🟡 低优先级

**文件**：`src-tauri/src/core/entity/contest.rs`；`src-tauri/src/adapter/hoj/mod.rs`

**描述**：`Contest` 实体有 `problems: Vec<String>` 字段，但 HOJ adapter 的 `list_contests` 和 `get_contest` 从未填充此字段（始终 `Vec::new()`），前端也从未使用。阶段 7 已新增独立的 `ContestProblem` 实体和 `list_contest_problems` 接口，该字段已无存在意义。

**建议修复**：移除 `Contest.problems` 字段，或标记为 deprecated。

**处理时机**：阶段 8。

---

## 问题 P51：前端零测试，P40 Command 测试仍未补充

**来源**：PR #12 Review

**状态**：🟡 中优先级

**文件**：`src/stores/*.ts`、`src/services/*.ts`、`src/bridge/*.ts`；`src-tauri/src/commands/`

**描述**：
1. 前端 5 个 Store、5 个 Service、7 个 Bridge 模块无任何单元测试。
2. P40（问题 P40）要求"阶段 7 前端对接前补充关键路径测试"，但 PR #12 声明留待阶段 8。todo.md 第 103 行已勾选完成，但 P40 的处理时机要求未满足。
3. 前后端 IPC 联调测试完全缺失——这正是 P42（命令名不匹配）和 P43（字段名不匹配）两个致命 bug 未被发现的原因。

**关键缺失测试场景**：
- `find_or_create` 的核心逻辑（匹配已有 workspace → 恢复代码 vs 新建）
- auto-save 懒启动的并发安全性
- IPC 命令名映射的正确性
- serde 序列化字段名的一致性
- `workspaceStore.updateCode` → 后端 `update_file` 的数据同步链路

**建议修复**：至少补充以上关键路径的集成测试。

**处理时机**：阶段 8 初期。

---

## 问题 P52：死代码与冗余逻辑

**来源**：PR #12 Review

**状态**：🟢 代码质量

**文件**：多个前端组件

**描述**：以下代码声明后从未使用：
1. `CodeEditor.vue` — `isReadonly` ref 声明后从未使用。
2. `CodeEditor.vue` — 注册了两个 `onMounted`（一个创建 Monaco 实例，一个注册 keydown 监听），应合并为一个。
3. `ProblemSidebar.vue` — `getStatusIcon` 函数声明后从未在模板中使用，且导入了 `CheckmarkCircle`、`CloseCircle`、`HelpCircle` 图标但未使用。
4. `ContestView.vue` — `startDrag` 中的 `isDragging` 变量赋值后从未被读取（没有用于条件渲染或 cursor 样式）。
5. `ContestView.vue` — 包裹了 `<n-message-provider>` 但没有任何组件使用 `useMessage()`，Naive UI 的消息提示功能完全闲置。

**建议修复**：清理上述死代码，合并冗余生命周期钩子。

**处理时机**：合并 PR #12 前清理。

---
