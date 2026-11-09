//! Mirrors every case in test/cmocka/test_weak_object_refs.c through the safe API.
use obs_core::weak_object_refs::*;
use obs_util::platform::CLongAtomic;
use std::sync::atomic::Ordering::SeqCst;

#[test]
fn null_handles() {
    addref(None);
    assert!(!release(None));
    assert_eq!(get_weak::<usize>(None), None);
    assert_eq!(get_object::<usize>(None), None);
}

#[test]
fn weak_ownership() {
    let refs = CLongAtomic::new(4);
    let weak = CLongAtomic::new(0);
    let object = 17usize;
    let control = 23usize;
    addref(Some(&weak));
    assert_eq!(weak.load(SeqCst), 1);
    assert_eq!(refs.load(SeqCst), 4);
    assert_eq!(object, 17);
    assert_eq!(get_weak(Some((control, &weak))), Some(control));
    assert_eq!(weak.load(SeqCst), 2);
    assert_eq!(refs.load(SeqCst), 4);
    assert!(!release(Some(&weak)));
    assert_eq!(weak.load(SeqCst), 1);
    assert!(!release(Some(&weak)));
    assert_eq!(weak.load(SeqCst), 0);
    assert_eq!(refs.load(SeqCst), 4);
    assert_eq!(object, 17);
    refs.store(-1, SeqCst);
    assert_eq!(get_weak(Some((control, &weak))), Some(control));
    assert_eq!(refs.load(SeqCst), -1);
    assert_eq!(weak.load(SeqCst), 1);
    assert!(!release(Some(&weak)));
}

#[test]
fn promotion() {
    let refs = CLongAtomic::new(0);
    let weak = CLongAtomic::new(3);
    let object = 17usize;
    for count in [-2, -1, 0, 1, 7] {
        refs.store(count, SeqCst);
        assert_eq!(
            get_object(Some((&refs, object))),
            (count >= 0).then_some(object)
        );
        assert_eq!(refs.load(SeqCst), if count < 0 { count } else { count + 1 });
        assert_eq!(weak.load(SeqCst), 3);
        assert_eq!(object, 17);
    }
}

#[test]
fn final_release() {
    // The safe core signals ownership transfer; the FFI frees via bfree.
    let mut allocation = Some(Box::new(CLongAtomic::new(0)));
    let weak = allocation.as_deref().unwrap();
    addref(Some(weak));
    assert_eq!(weak.load(SeqCst), 1);
    assert!(!release(Some(weak)));
    assert_eq!(weak.load(SeqCst), 0);
    assert!(allocation.is_some());
    if release(allocation.as_deref()) {
        allocation.take();
    }
    assert!(allocation.is_none());
}
