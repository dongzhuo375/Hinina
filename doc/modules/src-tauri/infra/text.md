# text（共享文本工具）

> 源文件：`src-tauri/src/infra/text.rs`

## 职责

「按**字符**而非字节截断 + 超长加 `…`」的单一实现：此前 `infra/http`（错误响应摘录）与 `adapter/hydro/types`（响应体预览）各自实现同一形态，收敛于此防止多份判据漂移。按字符截断保证中文不落 UTF-8 序列中间（字节截断会产生无效序列）。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `truncate_ellipsis` | `pub fn truncate_ellipsis(text: &str, max: usize) -> String` | 字符数 ≤ `max` 原样返回；超出取前 `max` 个字符并以 `…` 结尾 |

## 直接依赖

无（纯 std）。

## 被依赖

- `infra/http.rs`（`read_error_excerpt` 的错误响应摘录）
- `adapter/hydro/types.rs`（`preview` 的响应体预览）
- `infra/frontend_log.rs`（前端日志消息截断，`MAX_MESSAGE_CHARS = 2000`）

注意：`plugin/execution_api.rs` / `plugin/query.rs` 的 `chars().take(N)` 是**硬截断**（操作名 / 标题，无省略号、语义为「标识符钳制」），不属于本工具语义，刻意不迁移。

## 逻辑流程

`chars().count() > max` → `chars().take(max).collect()` + 追加 `…`；否则原样 `to_string()`。

## 测试

`src/infra/tests/text_tests.rs`：短串原样、超长截断加 `…`（中文按字符计数）、恰好等于上限不截断。
