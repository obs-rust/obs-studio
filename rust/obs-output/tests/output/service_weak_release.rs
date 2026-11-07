use obs_output::service_weak_release::{strong_release, weak_addref, weak_release};
use obs_util::platform::{CLong, CLongAtomic};
use std::sync::atomic::Ordering::SeqCst;

#[test]
fn test_strong_refs_only_last_release_destroys() {
    // test_service_lifecycle.c::test_strong_refs: two owners, then one, then none.
    let refs = CLongAtomic::new(1);
    assert!(!strong_release(&refs));
    assert_eq!(refs.load(SeqCst), 0);
    assert!(strong_release(&refs));
    assert_eq!(refs.load(SeqCst), -1);
}

#[test]
fn test_weak_refs_outlive_service_and_balance_addref() {
    // test_service_lifecycle.c::test_weak_refs: implicit + external weak refs.
    let strong = CLongAtomic::new(1);
    let weak = CLongAtomic::new(0);
    weak_addref(&weak);
    assert!(!strong_release(&strong)); // temporary strong reference
    weak_addref(&weak);
    assert!(!weak_release(&weak));
    assert!(strong_release(&strong)); // destroys service
    assert!(!weak_release(&weak)); // implicit weak owner
    assert_eq!(weak.load(SeqCst), 0); // external control remains alive
    assert!(weak_release(&weak));
    assert_eq!(weak.load(SeqCst), -1);
}

#[test]
fn release_and_addref_boundaries_preserve_c_long_width() {
    for start in [CLong::MIN, CLong::MIN + 1, -2, -1, 0, 1, CLong::MAX] {
        let refs = CLongAtomic::new(start);
        assert_eq!(strong_release(&refs), start == 0);
        assert_eq!(refs.load(SeqCst), start.wrapping_sub(1));
        let weak = CLongAtomic::new(start);
        assert_eq!(weak_release(&weak), start == 0);
        assert_eq!(weak.load(SeqCst), start.wrapping_sub(1));
    }
    for start in [CLong::MIN, -2, -1, 0, 1, CLong::MAX - 1, CLong::MAX] {
        let weak = CLongAtomic::new(start);
        weak_addref(&weak);
        assert_eq!(weak.load(SeqCst), start.wrapping_add(1));
    }
}
