//! Host-only diagnostics. Never format source errors: they can contain credentials, URLs or paths.
fn safe_id(id: Option<&str>) -> &str {
    match id {
        Some(id)
            if !id.is_empty()
                && id.len() <= 128
                && id.bytes().all(|b| {
                    b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-'
                }) =>
        {
            id
        }
        Some(_) => "<invalid>",
        None => "<host>",
    }
}
pub(crate) fn failure(id: Option<&str>, operation: &'static str, category: &'static str) {
    tracing::warn!(plugin_id=?safe_id(id), runtime="js", operation, category, "Plugin infrastructure failure");
}
pub(crate) fn io(id: Option<&str>, operation: &'static str, error: &std::io::Error) {
    tracing::warn!(plugin_id=?safe_id(id), runtime="js", operation, category="io", error_kind=?error.kind(), "Plugin infrastructure failure");
}
pub(crate) fn atomic_write(id: Option<&str>, error: &atomicwrites::Error<std::io::Error>) {
    // Both failure sites contain an io::Error; never format its potentially sensitive message.
    let (stage, error) = match error {
        atomicwrites::Error::Internal(error) => ("atomic_replace", error),
        atomicwrites::Error::User(error) => ("write_callback", error),
    };
    tracing::warn!(plugin_id=?safe_id(id),runtime="js",operation="storage.commit",category="atomic_write",stage,error_kind=?error.kind(),"Plugin infrastructure failure");
}
pub(crate) fn json(id: Option<&str>, operation: &'static str, error: &serde_json::Error) {
    tracing::warn!(plugin_id=?safe_id(id), runtime="js", operation, category="json", error_kind=?error.classify(), "Plugin infrastructure failure");
}
pub(crate) fn http(id: Option<&str>, operation: &'static str, error: &reqwest::Error) {
    let category = if error.is_timeout() {
        "http_timeout"
    } else if error.is_connect() {
        "http_connect"
    } else if error.is_body() {
        "http_body"
    } else if error.is_decode() {
        "http_decode"
    } else if error.is_builder() {
        "http_build"
    } else {
        "http_request"
    };
    failure(id, operation, category);
}

#[cfg(test)]
#[path = "tests/plugin_diagnostics_tests.rs"]
mod tests;
