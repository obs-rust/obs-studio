//! Mirrors test_output_weak_lifetime.c without FFI or unsafe code.
use obs_output::output_weak_lifetime::{self as api, Host};
use std::cell::Cell;

#[derive(Default)]
struct Fixture {
    refs: Cell<i64>,
    weak: Cell<i64>,
    freed: Cell<bool>,
    target: Cell<Option<u8>>,
}

impl Fixture {
    fn live() -> Self {
        Self { target: Cell::new(Some(1)), ..Self::default() }
    }

    // Strong release is outside this port. Model its destruction and implicit weak release.
    fn strong_release(&self) {
        self.refs.set(self.refs.get() - 1);
        if self.refs.get() == -1 {
            api::release(self, Some(0));
        }
    }
}

impl Host for Fixture {
    type Weak = u8;
    type Output = u8;
    fn control(&self, output: u8) -> Option<u8> { assert_eq!(output, 1); Some(0) }
    fn addref(&self, weak: u8) { assert_eq!(weak, 0); assert!(!self.freed.get()); self.weak.set(self.weak.get() + 1); }
    fn release(&self, weak: u8) -> bool { assert_eq!(weak, 0); assert!(!self.freed.get()); self.weak.set(self.weak.get() - 1); self.weak.get() == -1 }
    fn free(&self, weak: u8) { assert_eq!(weak, 0); assert!(!self.freed.replace(true)); }
    fn promote(&self, weak: u8) -> bool {
        assert_eq!(weak, 0);
        assert!(!self.freed.get());
        if self.refs.get() < 0 { return false; }
        self.refs.set(self.refs.get() + 1);
        true
    }
    fn target(&self, weak: u8) -> Option<u8> { assert_eq!(weak, 0); assert!(!self.freed.get()); self.target.get() }
}

#[test]
fn test_output_weak_lifetime_null_inputs() {
    let f = Fixture::live();
    api::addref(&f, None);
    api::release(&f, None);
    assert_eq!(api::get_weak(&f, None), None);
    assert_eq!(api::get_output(&f, None), None);
    assert!(!api::references(&f, None, None));
    assert_eq!((f.refs.get(), f.weak.get(), f.freed.get()), (0, 0, false));
}

#[test]
fn test_output_weak_lifetime_weak_ownership() {
    let f = Fixture::live();
    assert_eq!(api::get_weak(&f, Some(1)), Some(0));
    assert_eq!(f.weak.get(), 1);
    assert_eq!(api::get_weak(&f, Some(1)), Some(0));
    assert_eq!(f.weak.get(), 2);
    api::addref(&f, Some(0));
    assert_eq!(f.weak.get(), 3);
    api::release(&f, Some(0));
    assert_eq!(f.weak.get(), 2);
    api::release(&f, Some(0));
    api::release(&f, Some(0));
    assert_eq!((f.weak.get(), f.refs.get(), f.target.get()), (0, 0, Some(1)));
    assert!(!f.freed.get());
    f.refs.set(-1);
    api::release(&f, Some(0));
    assert!(f.freed.get());
}

#[test]
fn test_output_weak_lifetime_promotion_and_identity() {
    let f = Fixture::live();
    f.weak.set(3);
    for count in [0, 1, 7, 1023, -1, -2, -3] {
        f.refs.set(count);
        let result = api::get_output(&f, Some(0));
        assert_eq!(result, (count >= 0).then_some(1));
        assert_eq!(f.refs.get(), count + i64::from(count >= 0));
        assert_eq!(f.weak.get(), 3);
        if count >= 0 { f.strong_release(); }
        assert_eq!(f.refs.get(), count);
        assert!(api::references(&f, Some(0), Some(1)));
        assert!(!api::references(&f, Some(0), Some(2)));
        assert!(!api::references(&f, Some(0), None));
        assert!(!api::references(&f, None, Some(1)));
    }
}

#[test]
fn test_output_weak_lifetime_real_output_lifetime_policy() {
    let f = Fixture::live();
    let weak = api::get_weak(&f, Some(1));
    let second = api::get_weak(&f, Some(1));
    assert_eq!(weak, second);
    assert_eq!(f.weak.get(), 2);
    let promoted = api::get_output(&f, weak);
    assert_eq!(promoted, Some(1));
    assert_eq!(f.refs.get(), 1);
    f.strong_release();
    assert_eq!(f.refs.get(), 0);
    assert!(api::references(&f, weak, promoted));
    assert_eq!(api::get_output(&f, weak), promoted);
    f.strong_release();
    f.strong_release();
    assert_eq!((f.refs.get(), f.weak.get()), (-1, 1));
    assert_eq!(api::get_output(&f, weak), None);
    api::release(&f, second);
    assert_eq!(f.weak.get(), 0);
    assert_eq!(api::get_output(&f, weak), None);
    assert!(!f.freed.get());
    api::release(&f, weak);
    assert!(f.freed.get());
}

#[test]
fn null_target_still_acquires_strong_owner() {
    let f = Fixture::default();
    assert_eq!(api::get_output(&f, Some(0)), None);
    assert_eq!(f.refs.get(), 1);
    assert!(!api::references(&f, Some(0), None));
}
