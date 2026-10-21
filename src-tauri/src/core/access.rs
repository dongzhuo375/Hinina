//! Generic access revision fence. Explicit mutation paths invalidate in-flight readers;
//! correctness never depends on event delivery. Nested/concurrent transitions are supported.
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct State {
    revision: u64,
    changing: usize,
    cancellation: CancellationToken,
}

/// Shared by services that can change the effective user/OJ/contest access scope.
#[derive(Default)]
pub struct AccessRevision(Mutex<State>);

/// A stable revision plus cancellation for an in-flight operation.
#[derive(Clone)]
pub struct AccessSnapshot {
    pub revision: u64,
    pub cancellation: CancellationToken,
}

impl AccessRevision {
    /// No new reads are authorized while a transition is in progress.
    pub fn snapshot(&self) -> Option<AccessSnapshot> {
        let state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        (state.changing == 0).then(|| AccessSnapshot {
            revision: state.revision,
            cancellation: state.cancellation.clone(),
        })
    }

    /// Call before changing effective access, including fallible async mutations.
    pub fn begin(self: &Arc<Self>) -> AccessTransition {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        state.cancellation.cancel();
        state.changing += 1;
        state.revision = state.revision.wrapping_add(1);
        AccessTransition(self.clone())
    }
}

/// Drop completes a transition even when its future is cancelled or returns an error.
#[must_use = "hold the transition guard until all related access changes are complete"]
pub struct AccessTransition(Arc<AccessRevision>);
impl Drop for AccessTransition {
    fn drop(&mut self) {
        let mut state = self.0 .0.lock().unwrap_or_else(|e| e.into_inner());
        state.changing -= 1;
        state.revision = state.revision.wrapping_add(1);
        if state.changing == 0 {
            state.cancellation = CancellationToken::new();
        }
    }
}

#[cfg(test)]
#[path = "tests/access_tests.rs"]
mod tests;
