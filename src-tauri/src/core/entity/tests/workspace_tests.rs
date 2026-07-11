use super::*;

#[test]
fn new_workspace_has_timestamps_set() {
    let ws = Workspace::new(
        "contest-1".into(),
        "problem-A".into(),
        "/tmp/ws".into(),
    );

    assert!(ws.created_at > 0);
    assert!(ws.updated_at > 0);
    assert_eq!(ws.created_at, ws.updated_at);
    assert!(!ws.is_dirty);
    assert!(ws.files.is_empty());
}

#[test]
fn touch_updates_only_updated_at() {
    let mut ws = Workspace::new("c1".into(), "p1".into(), "/tmp/ws".into());
    let original_created = ws.created_at;
    let original_updated = ws.updated_at;

    // 短暂等待确保时间戳不同（系统时间分辨率）
    std::thread::sleep(std::time::Duration::from_secs(1));

    ws.touch();

    assert_eq!(ws.created_at, original_created);
    assert!(ws.updated_at >= original_updated);
    assert!(!ws.is_dirty); // touch 不改变 is_dirty
}

#[test]
fn mark_dirty_and_clean() {
    let mut ws = Workspace::new("c1".into(), "p1".into(), "/tmp/ws".into());
    assert!(!ws.is_dirty);

    ws.mark_dirty();
    assert!(ws.is_dirty);

    ws.mark_clean();
    assert!(!ws.is_dirty);
}

#[test]
fn mark_dirty_triggers_touch() {
    let mut ws = Workspace::new("c1".into(), "p1".into(), "/tmp/ws".into());
    let original_updated = ws.updated_at;

    std::thread::sleep(std::time::Duration::from_secs(1));

    ws.mark_dirty();

    assert!(ws.updated_at >= original_updated);
    assert!(ws.is_dirty);
}

#[test]
fn workspace_id_format() {
    let ws = Workspace::new("c42".into(), "pB".into(), "/ws".into());

    assert!(ws.id.starts_with("ws-c42-pB-"));
    assert_eq!(ws.contest_id, "c42");
    assert_eq!(ws.problem_id, "pB");
    assert_eq!(ws.root_path, "/ws");
}
