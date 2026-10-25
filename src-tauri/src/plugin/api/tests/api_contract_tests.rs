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
        cursor: None,
        limit: 100,
    };
    query.validate().unwrap();
    query.limit = 101;
    assert_eq!(query.validate(), Err(PluginError::LimitExceeded));
    query.limit = 100;
    query.cursor = Some(String::new());
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
        // The host owns Content-Type (it serializes the body as JSON); a plugin-supplied
        // value would otherwise be appended alongside the host's, sending a duplicate.
        ("content-type", "text/plain"),
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

#[test]
fn log_messages_are_bounded_and_control_characters_are_rejected() {
    log::validate_log_message("stage=dispatch sealed=false count=3").unwrap();
    // \n/\t pass validation; they are flattened only at write time.
    log::validate_log_message("line\nbreak\ttab").unwrap();
    assert_eq!(
        log::validate_log_message(""),
        Err(PluginError::InvalidInput)
    );
    assert_eq!(
        log::validate_log_message("   "),
        Err(PluginError::InvalidInput)
    );
    assert_eq!(
        log::validate_log_message("bad\0char"),
        Err(PluginError::InvalidInput)
    );
    assert_eq!(
        log::validate_log_message("bad\rchar"),
        Err(PluginError::InvalidInput)
    );
    assert_eq!(
        log::validate_log_message(&"x".repeat(log::MAX_LOG_BYTES)),
        Ok(())
    );
    assert_eq!(
        log::validate_log_message(&"x".repeat(log::MAX_LOG_BYTES + 1)),
        Err(PluginError::LimitExceeded)
    );
    // Multi-byte boundary: 342 CJK chars are 1026 UTF-8 bytes.
    assert_eq!(
        log::validate_log_message(&"气".repeat(342)),
        Err(PluginError::LimitExceeded)
    );
    assert_eq!(log::sanitize_log_message("a\nb\tc").as_ref(), "a b c");
    // No flattening needed: the original string is borrowed, not re-allocated.
    assert!(matches!(
        log::sanitize_log_message("plain"),
        std::borrow::Cow::Borrowed("plain")
    ));
    for (level, wire) in [
        (log::PluginLogLevel::Debug, "debug"),
        (log::PluginLogLevel::Info, "info"),
        (log::PluginLogLevel::Warn, "warn"),
        (log::PluginLogLevel::Error, "error"),
    ] {
        assert_eq!(serde_json::to_value(level).unwrap(), json!(wire));
    }
    assert!(serde_json::from_value::<log::PluginLogLevel>(json!("verbose")).is_err());
}

#[test]
fn rank_query_rejects_unknown_fields_like_the_other_input_dtos() {
    // The type-level "no contestId" guarantee must also hold at the wire level:
    // serde silently ignoring unknown fields would make the TS check meaningless.
    assert!(
        serde_json::from_value::<contest::PluginRankQuery>(serde_json::json!({
            "page": 1, "limit": 50, "contestId": "c1"
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<contest::PluginRankQuery>(serde_json::json!({
            "page": 1, "limit": 50
        }))
        .is_ok()
    );
}

#[test]
fn plugin_error_wire_shape_matches_the_bridge_contract() {
    use crate::plugin::host::manifest::PluginPermission;
    // Exhaustive match forces future error variants to update this wire contract test.
    fn expected_fields(error: &PluginError) -> Vec<&'static str> {
        match error {
            PluginError::PermissionDenied { .. } => vec!["code", "permission"],
            PluginError::InvalidInput
            | PluginError::ContextExpired
            | PluginError::Unauthenticated
            | PluginError::NotFound
            | PluginError::ProviderError
            | PluginError::LimitExceeded
            | PluginError::Timeout
            | PluginError::Unavailable
            | PluginError::ScriptError
            | PluginError::Unsupported => vec!["code"],
        }
    }
    let errors = [
        PluginError::InvalidInput,
        PluginError::ContextExpired,
        PluginError::Unauthenticated,
        PluginError::NotFound,
        PluginError::ProviderError,
        PluginError::LimitExceeded,
        PluginError::Timeout,
        PluginError::Unavailable,
        PluginError::ScriptError,
        PluginError::Unsupported,
    ];
    for error in errors {
        let value = serde_json::to_value(&error).unwrap();
        assert_eq!(
            value
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            expected_fields(&error)
        );
        assert!(value["code"].is_string());
    }
    let denied = serde_json::to_value(PluginError::PermissionDenied {
        permission: PluginPermission::EventsSubscribe,
    })
    .unwrap();
    assert_eq!(
        denied.as_object().unwrap().keys().collect::<Vec<_>>(),
        vec!["code", "permission"]
    );
    assert!(denied["permission"].is_string());
    assert_eq!(
        denied,
        json!({"code": "permission_denied", "permission": "events.subscribe"})
    );
}
