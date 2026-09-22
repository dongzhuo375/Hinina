use super::*;

/// 全部事件变体（新增变体时必须同步加入 —— 下面的契约测试都以此为准）。
fn all_variants() -> Vec<CoreEvent> {
    vec![
        CoreEvent::LoggedIn {
            oj_id: "HOJ".into(),
            user_id: "u-1".into(),
        },
        CoreEvent::LoggedOut {
            oj_id: "HOJ".into(),
        },
        CoreEvent::SessionExpired {
            oj_id: "HOJ".into(),
        },
        CoreEvent::TokenRotated {
            oj_id: "HOJ".into(),
        },
        CoreEvent::ConfigChanged,
        CoreEvent::ThemeChanged,
        CoreEvent::OjSwitched {
            oj_id: "QDUOJ".into(),
        },
        CoreEvent::ContestSelected {
            contest_id: "1011".into(),
        },
        CoreEvent::AnnouncementChanged {
            contest_id: "1011".into(),
            new_ids: vec!["a-1".into()],
        },
        CoreEvent::ProblemOpened {
            contest_id: "1011".into(),
            problem_id: "p-1".into(),
        },
        CoreEvent::SubmissionCreated {
            submission_id: "s-1".into(),
        },
        CoreEvent::SubmissionJudged {
            submission_id: "s-1".into(),
            status: "Accepted".into(),
        },
        CoreEvent::WorkspaceSaved {
            workspace_id: "ws-1".into(),
            revision: 3,
            automatic: true,
        },
    ]
}

/// 事件名是插件协议与日志排障的稳定契约：改动必须是有意识的（本用例会拦住手滑）。
#[test]
fn kind_is_unique_and_stable() {
    let expected = [
        "auth.logged_in",
        "auth.logged_out",
        "auth.session_expired",
        "auth.token_rotated",
        "system.config_changed",
        "system.theme_changed",
        "system.oj_switched",
        "contest.selected",
        "contest.announcement_changed",
        "problem.opened",
        "submission.created",
        "submission.judged",
        "workspace.saved",
    ];

    let actual: Vec<&'static str> = all_variants().iter().map(CoreEvent::kind).collect();
    assert_eq!(actual, expected);

    let mut sorted = actual.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), actual.len(), "事件名必须唯一");
}

/// 载荷里不得出现凭证 / 源代码 / 认证头等敏感内容。
///
/// 结构性保证来自类型本身（`CoreEvent` 没有任何能装下 token、Session、
/// 领域实体或源代码的字段）；本用例是**回归防线**：若将来有人为了图省事
/// 往变体里加 `token: String` 之类的字段，这里会立刻失败。
#[test]
fn payload_contains_no_sensitive_fields() {
    const FORBIDDEN: [&str; 7] = [
        "token",
        "password",
        "authorization",
        "bearer",
        "secret",
        "sourcecode",
        "cookie",
    ];

    for event in all_variants() {
        let debug = format!("{event:?}");
        for needle in FORBIDDEN {
            assert!(
                !debug.contains(needle),
                "事件 {} 的载荷疑似包含敏感字段 `{}`：{}",
                event.kind(),
                needle,
                debug
            );
        }
    }
}

/// 事件可 Clone（`broadcast` 为每个 receiver 克隆一份）。
#[test]
fn events_are_cloneable_and_comparable() {
    for event in all_variants() {
        assert_eq!(event.clone(), event);
    }
}

/// 载荷只带 ID / 修订号 / 状态摘要：`WorkspaceSaved` 不带文件内容，
/// `SubmissionJudged` 不带评测结果明细，`ProblemOpened` 不带题面。
#[test]
fn heavy_domain_payloads_are_absent() {
    let saved = CoreEvent::WorkspaceSaved {
        workspace_id: "ws-1".into(),
        revision: 7,
        automatic: false,
    };
    assert_eq!(
        saved,
        CoreEvent::WorkspaceSaved {
            workspace_id: "ws-1".into(),
            revision: 7,
            automatic: false,
        }
    );

    let judged = CoreEvent::SubmissionJudged {
        submission_id: "s-1".into(),
        status: "WrongAnswer".into(),
    };
    // 只带状态摘要：序列化后不可能出现 time/memory/cases 等明细字段
    let debug = format!("{judged:?}");
    assert!(!debug.contains("time_ms"));
    assert!(!debug.contains("memory"));
}
