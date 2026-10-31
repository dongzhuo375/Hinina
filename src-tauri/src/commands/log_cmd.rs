use crate::core::error::AppResult;

/// 前端日志落 Rust tracing（panic 钩子的前端半边）。
///
/// 前端 invoke 签名: `frontend_log`({ level, scope, message })
///
/// **薄转发**（CLAUDE.md：commands 保持轻量、委托底层）：收敛（级别/scope
/// 白名单、消息截断与控制字符剥离）、会话级字节预算与 tracing 落盘全部在
/// [`crate::infra::frontend_log`]（写法与落位沿 `plugin_diagnostics` 的先例）。
/// 安全口径见该模块文档：只收三个标量、无载荷参数，敏感面从一开始就不存在。
#[tauri::command]
pub async fn frontend_log(level: String, scope: String, message: String) -> AppResult<()> {
    crate::infra::frontend_log::emit(&level, &scope, &message);
    Ok(())
}
