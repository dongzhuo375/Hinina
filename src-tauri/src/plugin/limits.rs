//! Shared host limits. Ready timeout deliberately allows a small grace period beyond activation.
use std::time::Duration;

pub(crate) const ACTIVATION_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const EVENT_TIMEOUT: Duration = Duration::from_secs(30);
pub(crate) const START_TIMEOUT: Duration = Duration::from_secs(ACTIVATION_TIMEOUT.as_secs() + 2);
pub(crate) const STOP_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const PROVIDER_TIMEOUT: Duration = Duration::from_secs(5);
/// Whole submission.get scan: one worst-case provider page plus headroom for scope
/// checks and fast follow-up pages, still catchable before the activation deadline.
pub(crate) const SUBMISSION_LOOKUP_TIMEOUT: Duration = Duration::from_secs(8);
/// Return host-call errors early enough for a JS catch/finally before the hard watchdog.
pub(crate) const API_ERROR_GRACE: Duration = Duration::from_secs(1);
/// A persistently failing event handler must not warn on every delivery. The first
/// failure for a (plugin, code) warns immediately; repeats are suppressed until this
/// interval elapses, then reported once with the accumulated count.
pub(crate) const HANDLER_WARN_INTERVAL: Duration = Duration::from_secs(60);
pub(crate) const MAX_BRIDGE_RESPONSE_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_CURSOR_PAGE_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_CURSOR_BUFFER_BYTES: usize = 4 * 1024 * 1024;
/// Room reserved for the bridge `{"ok":true,"value":…}` envelope so that a page accepted
/// here can still be encoded and delivered; otherwise the plugin would never learn the
/// cursor id of a continuation the host has already published.
pub(crate) const BRIDGE_ENVELOPE_HEADROOM: usize = 256;
pub(crate) const MAX_CURSOR_RESPONSE_BYTES: usize =
    MAX_BRIDGE_RESPONSE_BYTES - BRIDGE_ENVELOPE_HEADROOM;

/// Bridge request bounds shared by the JS host function and `call_json`; the two checks
/// must agree or one layer would silently accept what the other rejects.
pub(crate) const MAX_BRIDGE_INPUT_BYTES: usize = 140_000;
pub(crate) const MAX_BRIDGE_OPERATION_BYTES: usize = 64;

/// Query handles and published cursors share one lifetime and one count cap.
pub(crate) const QUERY_HANDLE_TTL: Duration = Duration::from_secs(300);
pub(crate) const MAX_QUERY_CONTEXTS: usize = 32;
pub(crate) const MAX_CURSORS: usize = 32;
/// An unexpired handle for the same scope is reused only while younger than this, so a
/// reused handle always keeps most of its lifetime and handlers re-acquiring a context
/// per event cannot exhaust `MAX_QUERY_CONTEXTS`.
pub(crate) const QUERY_HANDLE_REUSE_AGE: Duration = Duration::from_secs(150);

/// Provider (OJ) request budgets: one per activation, one shared across all plugins.
pub(crate) const PLUGIN_QUERIES_PER_MINUTE: u32 = 60;
pub(crate) const PLUGIN_QUERY_CONCURRENCY: usize = 2;
pub(crate) const SHARED_QUERIES_PER_MINUTE: u32 = 120;
pub(crate) const SHARED_QUERY_CONCURRENCY: usize = 4;
