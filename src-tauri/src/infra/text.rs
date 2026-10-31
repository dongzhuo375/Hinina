//! 按字符截断的共享文本工具。
//!
//! 「按**字符**而非字节截断 + 超长加 `…`」此前在 `infra/http`（错误响应摘录）、
//! `adapter/hydro/types`（响应体预览）各自实现，本模块收敛为单一实现 —— 新的
//! 截断需求一律复用，避免多份判据漂移。按字符截断保证中文字符不落 UTF-8 序列
//! 中间（字节截断会产生无效序列）。
//!
//! 注意：`plugin/execution_api.rs` / `plugin/query.rs` 的 `chars().take(N)` 是
//! **硬截断**（操作名 / 标题，无省略号、语义为「标识符钳制」），不属于本工具的
//! 语义范围，刻意不迁移。

/// 按字符截断到 `max`；发生截断时以 `…` 结尾，未截断原样返回（不拷贝语义由
/// 调用方无感）。
pub fn truncate_ellipsis(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push('…');
    out
}

#[cfg(test)]
#[path = "tests/text_tests.rs"]
mod tests;
