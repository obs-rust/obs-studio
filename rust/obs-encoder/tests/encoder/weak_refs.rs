//! Tier 1 mirrors test_encoder_weak_refs.c over safe, owned state.
use obs_encoder::weak_refs::{addref, get_weak, release};
use obs_util::platform::CLongAtomic;
use std::sync::atomic::Ordering::SeqCst;

#[test]
fn null_handles() {
    addref(None);
    assert!(!release(None));
    assert_eq!(get_weak::<usize>(None), None); // NULL encoder.
    assert_eq!(get_weak::<usize>(None), None); // NULL context.control.
}

#[test]
fn exact_counts_and_identity() {
    let strong = CLongAtomic::new(7);
    let weak = CLongAtomic::new(3);
    let encoder = [42u8; 32];
    let before = encoder;
    let owner = Some(17usize);
    addref(Some(&weak));
    assert_eq!(weak.load(SeqCst), 4);
    assert_eq!(strong.load(SeqCst), 7);
    assert_eq!(owner, Some(17));
    for i in 0..3 {
        assert_eq!(get_weak(Some((19usize, &weak))), Some(19));
        assert_eq!(weak.load(SeqCst), 5 + i);
        assert_eq!(strong.load(SeqCst), 7);
        assert_eq!(owner, Some(17));
        assert_eq!(encoder, before);
    }
    for i in 0..4 {
        assert!(!release(Some(&weak)));
        assert_eq!(weak.load(SeqCst), 6 - i);
        assert_eq!(strong.load(SeqCst), 7);
        assert_eq!(owner, Some(17));
        assert_eq!(encoder, before);
    }
}

#[test]
fn acquisition_does_not_check_expiry() {
    let strong = CLongAtomic::new(-1);
    let weak = CLongAtomic::new(0);
    let owner: Option<usize> = None;
    assert_eq!(get_weak(Some((19usize, &weak))), Some(19));
    assert_eq!(weak.load(SeqCst), 1);
    assert_eq!(strong.load(SeqCst), -1);
    assert_eq!(owner, None);
    assert!(!release(Some(&weak)));
    assert_eq!(weak.load(SeqCst), 0);
}

#[test]
fn final_release_frees_control() {
    let strong = CLongAtomic::new(-1);
    let weak = CLongAtomic::new(0);
    let mut allocations = 1;
    addref(Some(&weak));
    assert_eq!(weak.load(SeqCst), 1);
    assert!(!release(Some(&weak)));
    assert_eq!(weak.load(SeqCst), 0);
    assert_eq!(allocations, 1);
    if release(Some(&weak)) {
        allocations -= 1;
    }
    assert_eq!(allocations, 0);
    assert_eq!(strong.load(SeqCst), -1);
}

#[test]
fn handles_survive_encoder_destruction() {
    // The safe ownership model represents the surrounding, unported lifecycle:
    // one implicit weak owner goes away when its last strong reference is lost.
    let strong = CLongAtomic::new(0);
    let weak = CLongAtomic::new(0);
    let a = get_weak(Some((19usize, &weak)));
    let b = get_weak(Some((19usize, &weak)));
    assert_eq!(a, b);
    assert_eq!(a, Some(19));
    assert_eq!(strong.load(SeqCst), 0);
    assert_eq!(weak.load(SeqCst), 2);
    strong.store(-1, SeqCst);
    assert!(!release(Some(&weak))); // Encoder's implicit weak owner.
    assert_eq!(strong.load(SeqCst), -1);
    assert_eq!(weak.load(SeqCst), 1);
    assert!(strong.load(SeqCst) < 0); // Upgrading a fails.
    assert!(!release(Some(&weak))); // Explicit a.
    assert_eq!(weak.load(SeqCst), 0);
    assert!(strong.load(SeqCst) < 0); // Upgrading b still fails.
    assert!(release(Some(&weak))); // Explicit b frees exactly one allocation.
}
