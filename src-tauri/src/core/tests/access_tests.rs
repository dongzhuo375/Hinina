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

#[test]
fn oj_scope_switch_does_not_publish_new_oj_with_old_contest() {
    use crate::core::provider::{oj_id::OjId, registry::ProviderRegistry};
    use crate::infra::provider_registry_impl::ProviderRegistryImpl;
    let access = Arc::new(AccessRevision::default());
    let registry =
        Arc::new(ProviderRegistryImpl::new(OjId::new("old")).with_access(access.clone()));
    let contest = Arc::new(Mutex::new(Some("old-contest")));
    let (reached, inspect) = (
        Arc::new(std::sync::Barrier::new(2)),
        Arc::new(std::sync::Barrier::new(2)),
    );
    let reader = {
        let (access, registry, contest, reached, inspect) = (
            access.clone(),
            registry.clone(),
            contest.clone(),
            reached.clone(),
            inspect.clone(),
        );
        std::thread::spawn(move || {
            reached.wait();
            assert_eq!(registry.current_id().as_str(), "new");
            assert_eq!(*contest.lock().unwrap(), Some("old-contest"));
            assert!(access.snapshot().is_none());
            inspect.wait();
        })
    };
    crate::commands::oj_cmd::switch_scope(&access, registry.as_ref(), OjId::new("new"), || {
        reached.wait();
        inspect.wait();
        *contest.lock().unwrap() = None;
    });
    reader.join().unwrap();
    assert!(access.snapshot().is_some());
    assert_eq!(*contest.lock().unwrap(), None);
}
