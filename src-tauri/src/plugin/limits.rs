//! Shared host limits. Ready timeout deliberately allows a small grace period beyond activation.
use std::time::Duration;

pub(crate) const ACTIVATION_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const EVENT_TIMEOUT: Duration = Duration::from_secs(30);
pub(crate) const START_TIMEOUT: Duration = Duration::from_secs(ACTIVATION_TIMEOUT.as_secs() + 2);
pub(crate) const STOP_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const PROVIDER_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const SUBMISSION_LOOKUP_TIMEOUT: Duration = Duration::from_secs(5);
/// Return host-call errors early enough for a JS catch/finally before the hard watchdog.
pub(crate) const API_ERROR_GRACE: Duration = Duration::from_secs(1);
pub(crate) const MAX_BRIDGE_RESPONSE_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_CURSOR_PAGE_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_CURSOR_BUFFER_BYTES: usize = 4 * 1024 * 1024;
