//! Tier 1: assertions from test/cmocka/test_object_ref.c over safe fixtures.
use obs_core::{
    canvas_ref,
    object_ref::{self, Host},
};
use obs_util::platform::CLongAtomic;
use std::sync::atomic::Ordering::SeqCst;

struct Fixture {
    initialized: bool,
    object: bool,
    kind: u32,
    refs: CLongAtomic,
    weak: CLongAtomic,
    stored: Option<usize>,
    warnings: usize,
    destroys: usize,
    at_destroy: Option<(usize, i128, i128)>,
    freed: bool,
}

impl Fixture {
    fn new(kind: u32, refs: i32, weak: i32) -> Self {
        Self {
            initialized: true,
            object: true,
            kind,
            refs: CLongAtomic::new(refs.into()),
            weak: CLongAtomic::new(weak.into()),
            stored: Some(42),
            warnings: 0,
            destroys: 0,
            at_destroy: None,
            freed: false,
        }
    }
    fn acquire(&self, object: bool, control: bool) -> Option<usize> {
        object_ref::get_ref((object && control).then_some((&self.refs, self.stored))).flatten()
    }
}

impl Host for Fixture {
    type Control = ();
    fn initialized(&self) -> bool {
        self.initialized
    }
    fn warn_shutdown(&mut self) {
        self.warnings += 1;
    }
    fn control(&self) -> Option<()> {
        self.object.then_some(())
    }
    fn strong(&self, (): ()) -> &CLongAtomic {
        &self.refs
    }
    fn destroy(&mut self) {
        self.destroys += 1;
        self.at_destroy = Some((
            17,
            i128::from(self.refs.load(SeqCst)),
            i128::from(self.weak.load(SeqCst)),
        ));
    }
    fn release_weak(&mut self, (): ()) {
        assert_eq!(self.destroys, 1);
        self.freed = canvas_ref::release(Some(&self.weak));
    }
}

#[test]
fn without_core() {
    let mut f = Fixture::new(1, 0, 0);
    f.initialized = false;
    assert_eq!(f.acquire(false, false), None);
    assert_eq!(f.acquire(true, true), Some(42));
    assert_eq!(f.refs.load(SeqCst), 1);
    f.object = false;
    object_ref::release(&mut f);
    f.object = true;
    object_ref::release(&mut f);
    assert_eq!(f.warnings, 2);
    assert_eq!(f.refs.load(SeqCst), 1);
    assert_eq!(f.weak.load(SeqCst), 0);
    assert_eq!(f.destroys, 0);
}

#[test]
fn null_and_expired() {
    for kind in 0..6 {
        let mut f = Fixture::new(kind, 0, 3);
        assert_eq!(f.acquire(false, false), None);
        f.object = false;
        object_ref::release(&mut f);
        assert_eq!(f.warnings, 0);
        f.object = true;
        assert_eq!(f.acquire(true, false), None);
        f.stored = None;
        assert_eq!(f.acquire(true, true), None);
        assert_eq!(f.refs.load(SeqCst), 1);
        object_ref::release(&mut f);
        assert_eq!(f.refs.load(SeqCst), 0);
        f.stored = Some(42);
        for refs in [-1, -2, -3] {
            f.refs.store(refs, SeqCst);
            assert_eq!(f.acquire(true, true), None);
            assert_eq!(f.refs.load(SeqCst), refs);
            assert_eq!(f.weak.load(SeqCst), 3);
            assert_eq!(f.destroys, 0);
        }
    }
}

#[test]
fn strong_counts_and_stored_identity() {
    for kind in 0..6 {
        for count in [0, 1, 7, 1023] {
            let mut f = Fixture::new(kind, count, 4);
            assert_eq!(f.acquire(true, true), Some(42));
            assert_eq!(f.acquire(true, true), Some(42));
            assert_eq!(f.refs.load(SeqCst), (count + 2) as core::ffi::c_long);
            object_ref::release(&mut f);
            assert_eq!(f.refs.load(SeqCst), (count + 1) as core::ffi::c_long);
            object_ref::release(&mut f);
            assert_eq!(f.refs.load(SeqCst), count as core::ffi::c_long);
            assert_eq!(f.weak.load(SeqCst), 4);
            assert_eq!(f.destroys, 0);
            assert_eq!(f.stored, Some(42));
            assert_eq!(f.kind, kind);
        }
        let mut f = Fixture::new(kind, 0, 0);
        assert_eq!(f.acquire(true, true), Some(42)); // argument identity is 17
        assert_eq!(f.refs.load(SeqCst), 1);
        object_ref::release(&mut f);
        assert_eq!(f.refs.load(SeqCst), 0);
        assert_eq!(f.destroys, 0);
    }
}

#[test]
fn last_release_and_weak_ownership() {
    for kind in 0..6 {
        for weak in [1, 0] {
            let mut f = Fixture::new(kind, 0, weak);
            object_ref::release(&mut f);
            assert_eq!(f.destroys, 1);
            assert_eq!(f.at_destroy, Some((17, -1, weak.into())));
            assert_eq!(f.refs.load(SeqCst), -1);
            assert_eq!(f.weak.load(SeqCst), (weak - 1) as core::ffi::c_long);
            assert_eq!(f.freed, weak == 0);
            assert_eq!(f.stored, Some(42));
            if weak == 1 {
                assert_eq!(f.acquire(true, true), None);
                assert_eq!(object_ref::get_ref(Some((&f.refs, 42))), None);
                assert_eq!(f.refs.load(SeqCst), -1);
                assert_eq!(f.destroys, 1);
                assert!(canvas_ref::release(Some(&f.weak)));
            }
        }
    }
}
