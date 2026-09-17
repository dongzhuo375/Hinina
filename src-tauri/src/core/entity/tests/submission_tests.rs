// core/entity/submission.rs 单元测试：终态判据（三处对齐的权威表）

use super::*;

/// 全部 18 个变体 + 期望终态性（穷尽表：新增状态时必须同步本表，
/// 否则「非终态」集合漂移会让评测中的提交被缓存、界面停在「评测中」）
fn all_statuses() -> Vec<(JudgementStatus, bool)> {
    vec![
        (JudgementStatus::Pending, false),
        (JudgementStatus::Compiling, false),
        (JudgementStatus::Running, false),
        (JudgementStatus::Accepted, true),
        (JudgementStatus::WrongAnswer, true),
        (JudgementStatus::TimeLimitExceeded, true),
        (JudgementStatus::MemoryLimitExceeded, true),
        (JudgementStatus::RuntimeError, true),
        (JudgementStatus::CompilationError, true),
        (JudgementStatus::PresentationError, true),
        (JudgementStatus::OutputLimitExceeded, true),
        (JudgementStatus::SystemError, true),
        (JudgementStatus::RemoteJudgeError, true),
        (JudgementStatus::SubmitFailed, true),
        (JudgementStatus::PartiallyAccepted, true),
        (JudgementStatus::FrequentLimit, true),
        (JudgementStatus::UnknownError, true),
        // Unknown 必须是终态：无法识别的状态码若被当非终态，轮询将永不停止
        (JudgementStatus::Unknown, true),
    ]
}

#[test]
fn is_terminal_full_table() {
    for (status, expected) in all_statuses() {
        assert_eq!(
            status.is_terminal(),
            expected,
            "{:?} 的终态判据与预期不符（三处判据：core entity / HOJ adapter / 前端 utils）",
            status
        );
    }
}

#[test]
fn only_pending_compiling_running_are_non_terminal() {
    let non_terminal: Vec<JudgementStatus> = all_statuses()
        .into_iter()
        .filter(|(status, _)| !status.is_terminal())
        .map(|(status, _)| status)
        .collect();

    assert_eq!(non_terminal.len(), 3, "非终态集合只应有 3 个变体");
    assert!(matches!(non_terminal[0], JudgementStatus::Pending));
    assert!(matches!(non_terminal[1], JudgementStatus::Compiling));
    assert!(matches!(non_terminal[2], JudgementStatus::Running));
}
