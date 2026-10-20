// core/entity/submission.rs 单元测试：终态判据（三处对齐的权威表）

use super::*;

/// 全部 20 个变体 + 期望终态性（穷尽表：新增状态时必须同步本表，
/// 否则「非终态」集合漂移会让评测中的提交被缓存、界面停在「评测中」）
fn all_statuses() -> Vec<(JudgementStatus, bool)> {
    vec![
        // 仅出现在「我的题目状态」查询中的两态（服务端评测码，非本轮评测结果）
        (JudgementStatus::NotSubmitted, true),
        (JudgementStatus::Cancelled, true),
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

/// 状态字符串名必须与 serde 序列化值**逐变体一致**（穷尽覆盖）。
///
/// `as_str()` 供 `CoreEvent::SubmissionJudged` 携带「状态摘要」；前端与插件都按
/// 这些确切名称做文案与配色映射（`JudgementStatus` 的序列化值即 IPC 契约），
/// 因此它与序列化值漂移会直接导致显示错误 —— 本用例是那条契约的回归防线。
#[test]
fn as_str_matches_serde_output_for_all_variants() {
    let mut seen: Vec<&'static str> = Vec::new();
    for (status, _) in all_statuses() {
        let serialized = serde_json::to_string(&status).expect("状态序列化失败");
        assert_eq!(
            serialized,
            format!("\"{}\"", status.as_str()),
            "{status:?} 的 as_str 与 serde 序列化值不一致"
        );
        seen.push(status.as_str());
    }

    // 变体总数与 as_str 的 match 分支数必须同步（新增变体时此处会失败）
    assert_eq!(seen.len(), 20, "状态变体数变化时须同步 as_str 与穷尽表");
    let mut deduped = seen.clone();
    deduped.sort_unstable();
    deduped.dedup();
    assert_eq!(deduped.len(), seen.len(), "状态名必须唯一");
}

/// `owner_id` 是宿主私有的归属证据：既不跨 IPC，也不随 JSON 缓存往返保留。
///
/// 这条契约决定插件查询必须直连 Provider；若将来把 `SubmissionHistoryPage`
/// 写入缓存再读回，owner 会变成 `None`，插件侧会整页拒绝而不是误信展示名。
#[test]
fn submission_record_never_serializes_host_private_owner() {
    let record = SubmissionRecord {
        owner_id: Some("uid-alice".into()),
        submit_id: "1".into(),
        pid: "2".into(),
        display_pid: "HOJ-2".into(),
        title: "A + B".into(),
        display_id: "A".into(),
        username: "alice".into(),
        submit_time: 1,
        status: JudgementStatus::Accepted,
        time_ms: 0,
        memory_kb: 0,
        score: None,
        length: 0,
        language: "C++".into(),
    };
    let json = serde_json::to_value(&record).expect("序列化失败");
    assert!(
        json.get("ownerId").is_none(),
        "owner_id 不得跨 IPC 暴露：{json}"
    );
    let round_tripped: SubmissionRecord = serde_json::from_value(json).expect("反序列化失败");
    assert_eq!(round_tripped.owner_id, None);
}
