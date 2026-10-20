use super::*;
use crate::core::error::AppError;
use crate::plugin::host::api_error::redact_app_error;
use serde_json::json;

#[test]
fn projections_only_serialize_public_fields() {
    let record = submission::PluginSubmission {
        submission_id: "s1".into(),
        contest_id: "c1".into(),
        problem_id: "p1".into(),
        problem_display_id: "A".into(),
        status: "Accepted".into(),
        submitted_at: 1_780_000_000_000,
    };
    let encoded = serde_json::to_value(record).unwrap();
    assert_eq!(
        encoded,
        json!({"submissionId":"s1","contestId":"c1","problemId":"p1","problemDisplayId":"A","status":"Accepted","submittedAt":1_780_000_000_000i64})
    );
    let identity = identity::PluginIdentity {
        oj_id: "hoj".into(),
        user_key: "opaque-key".into(),
    };
    assert_eq!(
        serde_json::to_value(identity).unwrap(),
        json!({"ojId":"hoj","userKey":"opaque-key"})
    );
}

#[test]
fn list_supports_resync_without_unbounded_history_queries() {
    let mut query = submission::PluginSubmissionQuery {
        contest_id: "c1".into(),
        problem_id: None,
        page: 1,
        page_size: 100,
    };
    query.validate().unwrap();
    query.page_size = 101;
    assert_eq!(query.validate(), Err(PluginError::LimitExceeded));
    query.page_size = 100;
    query.page = 0;
    assert_eq!(query.validate(), Err(PluginError::InvalidInput));
    assert!(serde_json::from_value::<submission::PluginSubmissionQuery>(
        json!({"contestId":"c1","problemId":null,"page":1,"pageSize":20,"onlyMine":false})
    )
    .is_err());
}

#[test]
fn storage_missing_and_json_null_are_distinct_on_wire() {
    use storage::PluginStorageValue;
    let missing = serde_json::to_value(PluginStorageValue::Missing).unwrap();
    let null = serde_json::to_value(PluginStorageValue::Value { value: json!(null) }).unwrap();
    assert_ne!(missing, null);
    assert_eq!(missing, json!({"state":"missing"}));
    assert_eq!(null, json!({"state":"value","value":null}));
    storage::validate_key("hoj:user:contest:problem").unwrap();
    assert!(storage::validate_key("bad\0key").is_err());
    assert!(storage::validate_key(&"x".repeat(257)).is_err());
    assert!(storage::validate_value(&json!("x".repeat(65_536))).is_err());
}

#[test]
fn http_shape_rejects_header_injection_transport_controls_and_limits() {
    let mut request = http::PluginHttpRequest {
        method: http::PluginHttpMethod::Post,
        url: "https://balloon.example.com/api".into(),
        headers: Default::default(),
        body: Some(json!({"id":"s1"})),
        timeout_ms: Some(5000),
    };
    request.validate_shape().unwrap();
    for (name, value) in [
        ("Host", "other.example.com"),
        ("Cookie", "secret"),
        ("proxy-authorization", "secret"),
        ("x-id", "a\r\nb"),
    ] {
        request.headers.clear();
        request.headers.insert(name.into(), value.into());
        assert_eq!(request.validate_shape(), Err(PluginError::InvalidInput));
    }
    request.headers.clear();
    request.timeout_ms = Some(5001);
    assert_eq!(request.validate_shape(), Err(PluginError::LimitExceeded));
    request.timeout_ms = Some(5000);
    request.body = Some(json!("x".repeat(http::MAX_HTTP_BODY_BYTES)));
    assert_eq!(request.validate_shape(), Err(PluginError::LimitExceeded));
    let encoded = json!({"method":"POST","url":"https://example.com","body":null,"timeoutMs":null,"proxy":"http://127.0.0.1"});
    assert!(serde_json::from_value::<http::PluginHttpRequest>(encoded).is_err());
}

#[test]
fn notifications_are_bounded_literal_text_and_errors_drop_internal_data() {
    notification::validate_message("<script>alert(1)</script>").unwrap(); // renderer must treat as literal text
    assert!(notification::validate_message(&"气".repeat(200)).is_err());
    let secret = "token=secret C:/private/code.cpp Authorization: secret";
    for internal in [
        AppError::Auth(secret.into()),
        AppError::Network(secret.into()),
        AppError::Submission(secret.into()),
    ] {
        let error = redact_app_error(&internal);
        let serialized = serde_json::to_string(&error).unwrap();
        assert!(!serialized.contains("secret"));
        assert!(!error.to_string().contains("private"));
    }
}
