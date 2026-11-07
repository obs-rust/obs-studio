use obs_output::service_weak_acquire::{target, try_acquire};
use obs_util::platform::{CLong, CLongAtomic};
use std::sync::atomic::Ordering::SeqCst;

#[test]
fn test_strong_refs_acquire_increments_without_replacing_target() {
    let refs = CLongAtomic::new(0);
    assert!(try_acquire(&refs));
    assert_eq!(refs.load(SeqCst), 1);
    assert!(try_acquire(&refs));
    assert_eq!(refs.load(SeqCst), 2);
    assert_eq!(target(Some(42usize)), Some(42));
    assert_eq!(target(None::<usize>), None);
}

#[test]
fn test_weak_refs_expired_never_resurrects() {
    for count in [-1, -2, CLong::MIN] {
        let refs = CLongAtomic::new(count);
        assert!(!try_acquire(&refs));
        assert_eq!(refs.load(SeqCst), count);
    }
}

#[test]
fn largest_defined_increment() {
    let refs = CLongAtomic::new(CLong::MAX - 1);
    assert!(try_acquire(&refs));
    assert_eq!(refs.load(SeqCst), CLong::MAX);
}
