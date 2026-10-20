use super::*;

#[test]
fn transitions_revoke_old_readers_and_prevent_aba_and_mid_transition_reads() {
    let access = Arc::new(AccessRevision::default());
    let old = access.snapshot().unwrap();
    let a = access.begin();
    let b = access.begin();
    assert!(old.cancellation.is_cancelled());
    assert!(access.snapshot().is_none());
    drop(a);
    assert!(access.snapshot().is_none());
    drop(b);
    let new = access.snapshot().unwrap();
    assert!(new.revision > old.revision);
    assert!(!new.cancellation.is_cancelled());
}
