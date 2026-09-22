use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::core::event::consumer::ConsumerExit;

/// 审计消费者对全部事件变体都能安全处理（覆盖 `log_event` 的每个分支）。
#[tokio::test]
async fn audit_consumer_handles_every_variant() {
    let bus = Arc::new(CoreEventBus::new());
    let task = spawn_audit_consumer(&bus);

    for event in [
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
            revision: 1,
            automatic: false,
        },
    ] {
        bus.publish(event);
    }

    // 给消费者一点时间排空（审计不改变状态，无法直接断言计数，
    // 这里只锁定「不 panic、能继续运行」）
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!task.is_finished(), "审计消费者不应异常退出");

    drop(bus);
    assert_eq!(task.await.unwrap(), ConsumerExit::Closed);
}

/// 审计消费者缺席不影响发布方：没有消费者时发布照常返回。
#[test]
fn publishing_without_audit_consumer_is_fine() {
    let bus = CoreEventBus::new();
    bus.publish(CoreEvent::ConfigChanged);
}
