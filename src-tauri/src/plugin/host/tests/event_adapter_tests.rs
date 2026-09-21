use super::*;
use crate::core::event::core_event::CoreEvent;

/// 白名单内的每个核心事件都必须映射到对应的插件事件，且**字段被裁剪**。
#[test]
fn whitelisted_events_map_with_trimmed_fields() {
    let mut adapter = PluginEventAdapter::new();

    let cases = [
        (
            CoreEvent::OjSwitched {
                oj_id: "HOJ".into(),
            },
            PluginEvent::OjSwitched {
                oj_id: "HOJ".into(),
            },
        ),
        (
            CoreEvent::ContestSelected {
                contest_id: "1011".into(),
            },
            PluginEvent::ContestSelected {
                contest_id: "1011".into(),
            },
        ),
        (
            // 公告只暴露数量，不暴露 ID 列表
            CoreEvent::AnnouncementChanged {
                contest_id: "1011".into(),
                new_ids: vec!["a-1".into(), "a-2".into()],
            },
            PluginEvent::AnnouncementChanged {
                contest_id: "1011".into(),
                new_count: 2,
            },
        ),
        (
            CoreEvent::ProblemOpened {
                contest_id: "1011".into(),
                problem_id: "p-1".into(),
            },
            PluginEvent::ProblemOpened {
                contest_id: "1011".into(),
                problem_id: "p-1".into(),
            },
        ),
        (
            CoreEvent::SubmissionCreated {
                submission_id: "s-1".into(),
            },
            PluginEvent::SubmissionCreated {
                submission_id: "s-1".into(),
            },
        ),
        (
            CoreEvent::SubmissionJudged {
                submission_id: "s-1".into(),
                status: "Accepted".into(),
            },
            PluginEvent::SubmissionJudged {
                submission_id: "s-1".into(),
                status: "Accepted".into(),
            },
        ),
        (
            CoreEvent::WorkspaceSaved {
                workspace_id: "ws-1".into(),
                revision: 4,
                automatic: true,
            },
            PluginEvent::WorkspaceSaved {
                workspace_id: "ws-1".into(),
                revision: 4,
                automatic: true,
            },
        ),
        (CoreEvent::ConfigChanged, PluginEvent::ConfigChanged),
        (CoreEvent::ThemeChanged, PluginEvent::ThemeChanged),
        (
            // 登录只暴露 OJ 标识，用户 UUID 不进协议
            CoreEvent::LoggedIn {
                oj_id: "HOJ".into(),
                user_id: "u-secret".into(),
            },
            PluginEvent::SessionLoggedIn {
                oj_id: "HOJ".into(),
            },
        ),
        (
            CoreEvent::LoggedOut {
                oj_id: "HOJ".into(),
            },
            PluginEvent::SessionLoggedOut {
                oj_id: "HOJ".into(),
            },
        ),
        (
            CoreEvent::SessionExpired {
                oj_id: "HOJ".into(),
            },
            PluginEvent::SessionExpired {
                oj_id: "HOJ".into(),
            },
        ),
    ];

    for (core, expected) in cases {
        let envelope = adapter
            .adapt(&core)
            .unwrap_or_else(|| panic!("{} 应在白名单内", core.kind()));
        assert_eq!(envelope.event, expected, "映射结果不符：{}", core.kind());
        assert_eq!(envelope.version, PLUGIN_EVENT_PROTOCOL_VERSION);
    }
}

/// 白名单外：凭证轮换事实不进插件协议，且**不消耗序号**。
#[test]
fn token_rotation_is_filtered_out_without_consuming_sequence() {
    let mut adapter = PluginEventAdapter::new();

    assert!(adapter
        .adapt(&CoreEvent::TokenRotated {
            oj_id: "HOJ".into()
        })
        .is_none());
    assert_eq!(adapter.sequence(), 0, "被过滤的事件不应消耗序号");

    // 过滤之后序号仍然连续（插件不会看到假缺口）
    let first = adapter.adapt(&CoreEvent::ConfigChanged).unwrap();
    assert_eq!(first.sequence, 1);
}

/// 序号单调递增、从 1 开始（插件据此自查缺口）。
#[test]
fn sequence_is_monotonic_from_one() {
    let mut adapter = PluginEventAdapter::new();
    for expected in 1..=5u64 {
        let envelope = adapter.adapt(&CoreEvent::ThemeChanged).unwrap();
        assert_eq!(envelope.sequence, expected);
    }
    assert_eq!(adapter.sequence(), 5);
}

/// 重新同步通知是协议内事件，同样带版本与序号。
#[test]
fn resync_notice_is_versioned_and_sequenced() {
    let mut adapter = PluginEventAdapter::new();
    let notice = adapter.resync_notice(3);

    assert_eq!(notice.version, PLUGIN_EVENT_PROTOCOL_VERSION);
    assert_eq!(notice.sequence, 1);
    assert_eq!(notice.event, PluginEvent::ResyncRequired { lost: 3 });
}

/// 适配产物里不得出现任何敏感内容（对外协议的回归防线）。
#[test]
fn adapted_envelopes_contain_no_sensitive_content() {
    const FORBIDDEN: [&str; 6] = [
        "token",
        "password",
        "authorization",
        "secret",
        "source",
        "cookie",
    ];

    let mut adapter = PluginEventAdapter::new();
    let events = [
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
        CoreEvent::AnnouncementChanged {
            contest_id: "1011".into(),
            new_ids: vec!["a-1".into()],
        },
        CoreEvent::WorkspaceSaved {
            workspace_id: "ws-1".into(),
            revision: 1,
            automatic: false,
        },
        CoreEvent::SubmissionJudged {
            submission_id: "s-1".into(),
            status: "Accepted".into(),
        },
    ];

    for event in events {
        let envelope = adapter.adapt(&event).expect("事件应在白名单内");
        let json = serde_json::to_string(&envelope).unwrap().to_lowercase();
        for needle in FORBIDDEN {
            assert!(
                !json.contains(needle),
                "适配产物疑似含敏感内容 `{needle}`：{json}"
            );
        }
    }
}

/// 发生时间是真实的 UTC 毫秒时间戳（不是 0、不是秒）。
#[test]
fn occurred_at_is_millisecond_timestamp() {
    let mut adapter = PluginEventAdapter::new();
    let envelope = adapter.adapt(&CoreEvent::ConfigChanged).unwrap();
    // 2020-01-01 之后的毫秒时间戳
    assert!(envelope.occurred_at > 1_577_836_800_000);
}
