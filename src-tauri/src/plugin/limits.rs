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

/// Plugin log outlet: one message bound and one write budget per activation. The bound
/// keeps a single line greppable; the budget stops a script from flooding the host log.
pub(crate) const PLUGIN_LOG_BYTES: usize = 1024;
pub(crate) const PLUGIN_LOGS_PER_MINUTE: u32 = 30;

/// Rank page bounds: one page request, one page of rows, and one row's problem
/// columns. Hydro returns the whole board as a single page, so an oversized page
/// must fail as limit_exceeded instead of being truncated by the bridge cap.
pub(crate) const MAX_RANK_PAGE: i64 = 100;
pub(crate) const MAX_RANK_ROWS: usize = 200;
pub(crate) const MAX_RANK_PROBLEMS: usize = 64;
/// Total cells across one rank page. Rows × columns must combine: 200 × 64 cells
/// would encode well past the 1 MiB bridge cap, so the product gets its own bound.
/// Row names are byte-bounded (96 UTF-8 bytes, char-boundary safe), so a full
/// 200 × 30 page with maxed-out names still encodes with margin; adversarial
/// content beyond that (oversized display ids, type-max numbers) fails closed at
/// the bridge cap instead of being truncated.
pub(crate) const MAX_RANK_CELLS: usize = 6_000;
/// Rank row text bound in UTF-8 BYTES, not chars: 256 CJK chars would be 768
/// bytes and, multiplied by 200 rows, break the encodable-page guarantee above.
pub(crate) const MAX_RANK_NAME_BYTES: usize = 96;
/// Problem-list text bounds in UTF-8 bytes. Titles are legitimately longer than
/// rank names, but byte bounds keep a 1,000-problem contest's list encodable
/// where char bounds (256 CJK chars = 768 B) would hit the bridge cap.
pub(crate) const MAX_PROBLEM_TITLE_BYTES: usize = 256;
pub(crate) const MAX_PROBLEM_COLOR_BYTES: usize = 64;
/// HOJ recomputes the whole board on every rank fetch; a dedicated per-activation
/// budget keeps plugin polling at the frontend's floor (one call per 10s) instead
/// of the generic 60/min, so two plugins cannot starve other queries. The burst
/// equals the quota, which also covers an initial multi-page board sync.
pub(crate) const PLUGIN_RANK_QUERIES_PER_MINUTE: u32 = 6;
