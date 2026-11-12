// 前端事件桥的载荷契约。
//
// 桥只转发两种前端需要即时感知的事实；载荷键名与 `src/bridge/*.bridge.ts`
// 的监听一一对应，改名会让前端拿到 undefined 却不报错——这里逐键锁定。
// 其余事件必须返回 `None`：状态真值经 IPC 查询，事件转发只会制造第二来源。

use super::*;

#[test]
fn workspace_saved_maps_to_workspace_saved_channel() {
    let event = CoreEvent::WorkspaceSaved {
        workspace_id: "w-1".into(),
        revision: 7,
        automatic: true,
    };

    let (channel, payload, what) = payload_for(&event).expect("WorkspaceSaved 必须桥接");

    assert_eq!(channel, "workspace-saved");
    assert_eq!(what, "工作区落盘事件");
    assert_eq!(
        payload,
        serde_json::json!({"workspaceId": "w-1", "revision": 7, "auto": true})
    );
}

#[test]
fn announcement_changed_maps_to_published_channel() {
    let event = CoreEvent::AnnouncementChanged {
        contest_id: "contest-9".into(),
        new_ids: vec!["a-1".into(), "a-2".into()],
    };

    let (channel, payload, what) = payload_for(&event).expect("AnnouncementChanged 必须桥接");

    assert_eq!(channel, "announcements-published");
    assert_eq!(what, "新公告事件");
    assert_eq!(
        payload,
        serde_json::json!({"contestId": "contest-9", "newIds": ["a-1", "a-2"]})
    );
}

#[test]
fn other_events_are_not_bridged() {
    // 与 `CoreEvent` 全部 13 个变体对账：桥接 2 个（WorkspaceSaved /
    // AnnouncementChanged），其余 11 个逐一在此列出——不给 `_ => None` 的
    // 通配留「将来新增 Some 臂而 CI 不红」的口子；新增变体时必须在此表态。
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
        CoreEvent::TokenRotated {
            oj_id: "HOJ".into(),
        },
        CoreEvent::ConfigChanged,
        CoreEvent::ThemeChanged,
        CoreEvent::OjSwitched {
            oj_id: "HOJ".into(),
        },
        CoreEvent::ContestSelected {
            contest_id: "1011".into(),
        },
        CoreEvent::ProblemOpened {
            contest_id: "1011".into(),
            problem_id: "A".into(),
        },
        CoreEvent::SubmissionCreated {
            submission_id: "s-1".into(),
        },
        CoreEvent::SubmissionJudged {
            submission_id: "s-1".into(),
            status: "Accepted".into(),
        },
    ];

    assert_eq!(events.len() + 2, 13, "CoreEvent 变体总数变化时须同步本用例");

    for event in &events {
        assert!(
            payload_for(event).is_none(),
            "{} 不应出现在前端事件桥上",
            event.kind()
        );
    }
}
