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

    ws.touch();

    assert_eq!(ws.created_at, original_created);
    // 毫秒级时间戳确保 touch 后更新（无需 sleep）
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

    ws.mark_dirty();

    // 毫秒级时间戳，无需 sleep
    assert!(ws.updated_at >= original_updated);
    assert!(ws.is_dirty);
}

#[test]
fn workspace_id_format() {
    let ws = Workspace::new("c42".into(), "pB".into(), "/ws".into());

    assert!(ws.id.starts_with("ws-c42-pB-"));
    // ID 格式: ws-{contest}-{problem}-{timestamp_ms}-{4hex}
    let parts: Vec<&str> = ws.id.split('-').collect();
    assert_eq!(parts.len(), 5, "id 应包含 5 段: ws / contest / problem / timestamp / hex");
    assert_eq!(parts[1], "c42");
    assert_eq!(parts[2], "pB");
    // 时间戳为 13 位毫秒级，随机后缀为 4 位十六进制
    assert_eq!(parts[3].len(), 13, "时间戳应为 13 位毫秒级");
    assert_eq!(parts[4].len(), 4, "随机后缀应为 4 位十六进制");
    assert!(parts[4].chars().all(|c| c.is_ascii_hexdigit()));

    assert_eq!(ws.contest_id, "c42");
    assert_eq!(ws.problem_id, "pB");
    assert_eq!(ws.root_path, "/ws");
}
