use super::*;

// ── AppError::context ──
//
// context() 给错误补上「哪个环节失败」，但**绝不能改变变体**：
// 前端 bridge 依据 variant 做 isAuthError 分流（会话失效全局兜底），
// 变体被改写会让 401 不再触发登出，选手被卡在比赛页反复失败。

/// 覆盖全部变体：新增变体时此表会漏测，故用穷尽 match 兜底断言。
fn variant_name(e: &AppError) -> &'static str {
    match e {
        AppError::Auth(_) => "Auth",
        AppError::Contest(_) => "Contest",
        AppError::Problem(_) => "Problem",
        AppError::Submission(_) => "Submission",
        AppError::Workspace(_) => "Workspace",
        AppError::Io(_) => "Io",
        AppError::Network(_) => "Network",
        AppError::Config(_) => "Config",
        AppError::ProviderNotFound(_) => "ProviderNotFound",
        AppError::Serialization(_) => "Serialization",
        AppError::Unknown(_) => "Unknown",
    }
}

#[test]
fn context_preserves_variant_for_every_variant() {
    let all = vec![
        AppError::Auth("token 过期".into()),
        AppError::Contest("比赛不存在".into()),
        AppError::Problem("题目不存在".into()),
        AppError::Submission("提交失败".into()),
        AppError::Workspace("无当前工作区".into()),
        AppError::Io("磁盘满".into()),
        AppError::Network("HTTP 502".into()),
        AppError::Config("配置缺失".into()),
        AppError::ProviderNotFound("QDUOJ".into()),
        AppError::Serialization("字段不匹配".into()),
        AppError::Unknown("?".into()),
    ];

    for original in all {
        let name = variant_name(&original);
        let message = original.user_message().to_string();
        let wrapped = original.context("HOJ contest list");

        assert_eq!(
            variant_name(&wrapped),
            name,
            "context() 改写了错误变体，前端 isAuthError 分流会失灵"
        );
        assert_eq!(
            wrapped.user_message(),
            format!("HOJ contest list: {}", message),
            "context() 应把环节名拼在原始消息之前"
        );
    }
}

#[test]
fn context_keeps_auth_error_recognisable() {
    // 评测轮询遇到 401 时，前端 sessionGuard 依据 variant === 'Auth' 判定会话失效并登出；
    // 若被改写成 Network，兜底不会触发，选手只会看到无意义的「网络错误」
    let err = AppError::Auth("HTTP 401 Unauthorized".into()).context("HOJ judgement");
    assert!(matches!(err, AppError::Auth(_)));
    assert_eq!(err.user_message(), "HOJ judgement: HTTP 401 Unauthorized");
    // Display 仍带变体前缀，日志里能一眼看出类别
    assert_eq!(err.to_string(), "认证错误: HOJ judgement: HTTP 401 Unauthorized");
}

#[test]
fn context_does_not_duplicate_variant_prefix() {
    // context 取的是原始消息而非 Display，否则会得到「网络错误: 网络错误: …」
    let err = AppError::Network("HTTP 502".into()).context("HOJ contest list");
    assert_eq!(err.user_message(), "HOJ contest list: HTTP 502");
    assert!(!err.user_message().contains("网络错误"));
}
