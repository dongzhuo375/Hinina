use super::*;

/// 协议形状是插件作者直接面对的契约：改字段名 / 改 tag 都是破坏性变更。
#[test]
fn envelope_serializes_with_camel_case_and_event_tag() {
    let envelope = PluginEventEnvelope {
        version: PLUGIN_EVENT_PROTOCOL_VERSION,
        sequence: 7,
        occurred_at: 1_700_000_000_000,
        event: PluginEvent::WorkspaceSaved {
            workspace_id: "ws-1".into(),
            revision: 3,
            automatic: true,
        },
    };

    let json = serde_json::to_string(&envelope).unwrap();
    assert!(json.contains("\"version\":1"), "{json}");
    assert!(json.contains("\"sequence\":7"), "{json}");
    assert!(json.contains("\"occurredAt\""), "{json}");
    assert!(json.contains("\"type\":\"workspace_saved\""), "{json}");
    assert!(json.contains("\"workspaceId\":\"ws-1\""), "{json}");

    let back: PluginEventEnvelope = serde_json::from_str(&json).unwrap();
    assert_eq!(back, envelope);
}

/// 协议里不得出现凭证 / 源代码 / 认证头 / 内部路径等敏感内容。
///
/// 与 `CoreEvent` 的同类用例互补：这里锁的是**对外协议**，一旦有人往
/// `PluginEvent` 里加 `token` / `path` 之类的字段，插件侧就会直接看到。
#[test]
fn protocol_payload_contains_no_sensitive_fields() {
    const FORBIDDEN: [&str; 8] = [
        "token",
        "password",
        "authorization",
        "bearer",
        "secret",
        "sourcecode",
        "cookie",
        "filepath",
    ];

    let events = [
        PluginEvent::OjSwitched {
            oj_id: "HOJ".into(),
        },
        PluginEvent::ContestSelected {
            contest_id: "1011".into(),
        },
        PluginEvent::AnnouncementChanged {
            contest_id: "1011".into(),
            new_count: 2,
        },
        PluginEvent::ProblemOpened {
            contest_id: "1011".into(),
            problem_id: "p-1".into(),
        },
        PluginEvent::SubmissionCreated {
            submission_id: "s-1".into(),
        },
        PluginEvent::SubmissionJudged {
            submission_id: "s-1".into(),
            status: "Accepted".into(),
        },
        PluginEvent::WorkspaceSaved {
            workspace_id: "ws-1".into(),
            revision: 1,
            automatic: false,
        },
        PluginEvent::ConfigChanged,
        PluginEvent::ThemeChanged,
        PluginEvent::SessionLoggedIn {
            oj_id: "HOJ".into(),
        },
        PluginEvent::SessionLoggedOut {
            oj_id: "HOJ".into(),
        },
        PluginEvent::SessionExpired {
            oj_id: "HOJ".into(),
        },
        PluginEvent::ResyncRequired { lost: 3 },
    ];

    for event in events {
        let json = serde_json::to_string(&event).unwrap();
        let lower = json.to_lowercase();
        for needle in FORBIDDEN {
            assert!(
                !lower.contains(needle),
                "插件事件 {json} 疑似包含敏感字段 `{needle}`"
            );
        }
    }
}

/// 协议**不含**凭证轮换事件：白名单之外的事实不进插件侧。
#[test]
fn token_rotation_is_not_part_of_the_protocol() {
    let json = serde_json::to_string(&PluginEvent::SessionLoggedIn {
        oj_id: "HOJ".into(),
    })
    .unwrap();
    assert!(!json.contains("token"));
    assert!(!json.contains("rotat"));
}

/// 协议版本是显式契约：破坏性变更必须自增（此用例拦住「静默改协议」）。
#[test]
fn protocol_version_is_pinned() {
    assert_eq!(PLUGIN_EVENT_PROTOCOL_VERSION, 1);
}
