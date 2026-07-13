// HOJ types.rs 单元测试：map_status / is_terminal_status

use super::*;

// ── map_status ──

#[test]
fn map_status_pending() {
    assert!(matches!(map_status(0), crate::core::entity::submission::JudgementStatus::Running));
}

#[test]
fn map_status_judging() {
    assert!(matches!(map_status(1), crate::core::entity::submission::JudgementStatus::Running));
}

#[test]
fn map_status_ce() {
    assert!(matches!(map_status(2), crate::core::entity::submission::JudgementStatus::CompilationError));
}

#[test]
fn map_status_pe() {
    // PE → WrongAnswer（无对应枚举）
    assert!(matches!(map_status(3), crate::core::entity::submission::JudgementStatus::WrongAnswer));
}

#[test]
fn map_status_wa() {
    assert!(matches!(map_status(4), crate::core::entity::submission::JudgementStatus::WrongAnswer));
}

#[test]
fn map_status_ac() {
    assert!(matches!(map_status(5), crate::core::entity::submission::JudgementStatus::Accepted));
}

#[test]
fn map_status_tle() {
    assert!(matches!(map_status(6), crate::core::entity::submission::JudgementStatus::TimeLimitExceeded));
}

#[test]
fn map_status_mle() {
    assert!(matches!(map_status(7), crate::core::entity::submission::JudgementStatus::MemoryLimitExceeded));
}

#[test]
fn map_status_ole() {
    // OLE → Unknown（无对应枚举）
    assert!(matches!(map_status(8), crate::core::entity::submission::JudgementStatus::Unknown));
}

#[test]
fn map_status_re() {
    assert!(matches!(map_status(9), crate::core::entity::submission::JudgementStatus::RuntimeError));
}

#[test]
fn map_status_se() {
    // SE → Unknown
    assert!(matches!(map_status(10), crate::core::entity::submission::JudgementStatus::Unknown));
}

#[test]
fn map_status_rje() {
    // RJE → Unknown
    assert!(matches!(map_status(11), crate::core::entity::submission::JudgementStatus::Unknown));
}

#[test]
fn map_status_sf() {
    // SF → WrongAnswer
    assert!(matches!(map_status(12), crate::core::entity::submission::JudgementStatus::WrongAnswer));
}

#[test]
fn map_status_pa() {
    // Partial AC → Accepted（保守映射）
    assert!(matches!(map_status(13), crate::core::entity::submission::JudgementStatus::Accepted));
}

#[test]
fn map_status_freq() {
    // FREQ → Unknown
    assert!(matches!(map_status(14), crate::core::entity::submission::JudgementStatus::Unknown));
}

#[test]
fn map_status_ue() {
    // UE → Unknown
    assert!(matches!(map_status(15), crate::core::entity::submission::JudgementStatus::Unknown));
}

#[test]
fn map_status_negative() {
    assert!(matches!(map_status(-1), crate::core::entity::submission::JudgementStatus::Unknown));
}

#[test]
fn map_status_out_of_range() {
    assert!(matches!(map_status(999), crate::core::entity::submission::JudgementStatus::Unknown));
}

// ── is_terminal_status ──

#[test]
fn is_terminal_pending() {
    assert!(!is_terminal_status(0));
}

#[test]
fn is_terminal_judging() {
    assert!(!is_terminal_status(1));
}

#[test]
fn is_terminal_ce() {
    assert!(is_terminal_status(2));
}

#[test]
fn is_terminal_pe() {
    assert!(is_terminal_status(3));
}

#[test]
fn is_terminal_ac() {
    assert!(is_terminal_status(5));
}

#[test]
fn is_terminal_out_of_range() {
    assert!(is_terminal_status(999));
}
