use super::*;
use std::{
    io::Write,
    sync::{Arc, Mutex},
};
use tracing::instrument::WithSubscriber;

#[derive(Clone)]
struct Capture(Arc<Mutex<Vec<u8>>>);
impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn capture() -> (Arc<Mutex<Vec<u8>>>, tracing::Dispatch, tracing::Dispatch) {
    // Two live dispatchers prevent parallel tests caching this callsite as never.
    let sink = tracing::Dispatch::new(
        tracing_subscriber::fmt()
            .with_writer(std::io::sink)
            .finish(),
    );
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let writer = Capture(bytes.clone());
    let dispatch = tracing::Dispatch::new(
        tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish(),
    );
    (bytes, dispatch, sink)
}

#[test]
fn invalid_ids_are_replaced_and_atomic_failures_keep_safe_io_categories() {
    let (bytes, dispatch, _sink) = capture();
    tracing::dispatcher::with_default(&dispatch, || {
        for id in ["", "INJECT\nWARN secret", &"a".repeat(129)] {
            failure(Some(id), "fixture.invalid_id", "invalid");
        }
        failure(None, "fixture.host", "fixture");
        atomic_write(
            Some("test.atomic"),
            &atomicwrites::Error::Internal(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "private filesystem path",
            )),
        );
        atomic_write(
            Some("test.atomic"),
            &atomicwrites::Error::User(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "private storage content",
            )),
        );
    });
    let log = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
    assert_eq!(log.matches("<invalid>").count(), 3);
    for expected in [
        "<host>",
        "PermissionDenied",
        "WriteZero",
        "atomic_replace",
        "write_callback",
    ] {
        assert!(log.contains(expected), "missing: {expected}");
    }
    for secret in ["INJECT", "secret", "private filesystem", "private storage"] {
        assert!(!log.contains(secret));
    }
}

#[tokio::test]
async fn repository_and_storage_failures_are_actionable_and_redacted() {
    use crate::{
        infra::{fs_plugin_repo::FsPluginRepository, plugin_storage::JsonPluginStorage},
        plugin::{registry::PluginRepository, tests::support::authority},
    };
    let (bytes, dispatch, _sink) = capture();
    let dir = crate::test_support::TempDir::unique("private-diagnostic-path");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("plugins.json"), "private configuration").unwrap();
    let repository = FsPluginRepository::new(dir.to_path_buf());
    tracing::dispatcher::with_default(&dispatch, || {
        assert!(repository.configuration().is_err());
    });

    // Initialize before installing this task's subscriber; the failure occurs inside spawn_blocking.
    let storage = JsonPluginStorage::new(dir.join("data"), authority("test.diagnostics"));
    storage.set("key", serde_json::Value::Null).await.unwrap();
    let namespace = std::fs::read_dir(dir.join("data"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    std::fs::write(namespace.join("storage.json"), "{private token secret").unwrap();
    assert!(storage
        .get("key")
        .with_subscriber(dispatch.clone())
        .await
        .is_err());
    tracing::dispatcher::with_default(&dispatch, || {
        io(
            Some("test.diagnostics"),
            "fixture.io",
            &std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Authorization: private token C:/private",
            ),
        );
        failure(Some("test.diagnostics"), "notification.emit", "emit_failed");
    });
    let log = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
    for expected in [
        "repository.configuration_decode",
        "storage.decode",
        "PermissionDenied",
        "notification.emit",
        "WARN",
    ] {
        assert!(log.contains(expected), "missing diagnostic: {expected}");
    }
    for secret in [
        "private configuration",
        "private token",
        "Authorization:",
        "C:/private",
        "private-diagnostic-path",
    ] {
        assert!(!log.contains(secret), "sensitive diagnostic: {secret}");
    }
}

#[tokio::test]
async fn http_error_classification_drops_url_and_query_credentials() {
    let (bytes, dispatch, _sink) = capture();
    let error = reqwest::Client::new()
        .get("http://user:private-password@invalid.example/?token=private-token")
        .header("x-invalid", "invalid\nheader")
        .send()
        .await
        .unwrap_err();
    tracing::dispatcher::with_default(&dispatch, || http(Some("test.http"), "http.send", &error));
    let log = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
    assert!(log.contains("http_build"));
    for secret in [
        "private-password",
        "private-token",
        "invalid.example",
        "header",
    ] {
        assert!(!log.contains(secret));
    }
}
