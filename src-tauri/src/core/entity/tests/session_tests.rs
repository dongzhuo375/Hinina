use super::*;

/// schema 与旧版一致：既有 `sessions/{oj_id}.json` 必须能被原样读出。
#[test]
fn deserializes_legacy_session_file() {
    let legacy = r#"{
        "user_id": "u-1",
        "username": "alice",
        "token": "t-1",
        "oj_id": "HOJ"
    }"#;
    let session: Session = serde_json::from_str(legacy).unwrap();
    assert_eq!(session.user_id, "u-1");
    assert_eq!(session.username, "alice");
    assert_eq!(session.token, "t-1");
    assert_eq!(session.oj_id, "HOJ");
}

/// 旧版文件用 `oj_type` 作键名（历史枚举命名），经 alias 兼容读取。
#[test]
fn deserializes_legacy_oj_type_key() {
    let legacy = r#"{ "username": "alice", "token": "t-1", "oj_type": "QDUOJ" }"#;
    let session: Session = serde_json::from_str(legacy).unwrap();
    assert_eq!(session.oj_id, "QDUOJ");
    // 升级前不含 user_id 的文件反序列化为空串，而不是整体解析失败
    assert_eq!(session.user_id, "");
}

/// 序列化键名保持 snake_case（与既有文件一致，改动会让所有人的会话「凭空消失」）。
#[test]
fn serializes_with_snake_case_keys() {
    let session = Session::new("HOJ", "u-1", "alice", "t-1");
    let json = serde_json::to_string(&session).unwrap();
    assert!(json.contains("\"user_id\""), "{json}");
    assert!(json.contains("\"username\""), "{json}");
    assert!(json.contains("\"token\""), "{json}");
    assert!(json.contains("\"oj_id\""), "{json}");
    assert!(!json.contains("oj_type"), "{json}");
}

/// 构造器与字段一一对应（避免「参数顺序写反」这类静默错误）。
#[test]
fn constructor_maps_arguments_in_order() {
    let session = Session::new("OJ-A", "id-1", "name-1", "tok-1");
    assert_eq!(
        session,
        Session {
            user_id: "id-1".to_string(),
            username: "name-1".to_string(),
            token: "tok-1".to_string(),
            oj_id: "OJ-A".to_string(),
        }
    );
}
