//! Tier 1: mirrors every case of test/cmocka/test_source_weak_refs.c that
//! does not need a live core, through the safe API.
use obs_source::weak_refs::{addref, release};
use obs_util::platform::CLongAtomic;
use std::sync::atomic::Ordering::SeqCst;

#[test]
fn null_handles() {
    addref(None);
    assert!(!release(None));
}

#[test]
fn counts_only_the_weak_refs() {
    let weak = CLongAtomic::new(3);
    for i in 1..=3 {
        addref(Some(&weak));
        assert_eq!(weak.load(SeqCst), 3 + i);
    }
    for i in 1..=6 {
        assert!(!release(Some(&weak)));
        assert_eq!(weak.load(SeqCst), 6 - i);
    }
    assert_eq!(weak.load(SeqCst), 0);
}

#[test]
fn final_release_frees() {
    let weak = CLongAtomic::new(1);
    assert!(!release(Some(&weak)));
    assert_eq!(weak.load(SeqCst), 0);
    addref(Some(&weak));
    assert!(!release(Some(&weak)));
    assert!(release(Some(&weak)));
    assert_eq!(weak.load(SeqCst), -1);
}

/// Characterized, not endorsed: only the exact -1 frees.
#[test]
fn below_zero_never_frees() {
    let weak = CLongAtomic::new(-1);
    assert!(!release(Some(&weak)));
    assert_eq!(weak.load(SeqCst), -2);
    addref(Some(&weak));
    addref(Some(&weak));
    assert_eq!(weak.load(SeqCst), 0);
    assert!(release(Some(&weak)));
}
