//! Exact traces against the unchanged extracted C body, with no exclusions.
use core::ffi::c_void;
use obs_scene::atomic_update::{Scene, update};
use proptest::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

unsafe extern "C" {
    fn oracle_scene_atomic_trace(bits: u32, refs: u32, data: u64, out: *mut u64);
    fn oracle_scene_atomic_trace_with(
        bits: u32,
        refs: u32,
        data: u64,
        out: *mut u64,
        update: unsafe extern "C" fn(
            *mut c_void,
            Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
            *mut c_void,
        ),
    );
}
struct State {
    refs: Cell<u64>,
    trace: RefCell<Vec<u64>>,
    removed: bool,
}
impl State {
    fn record(&self, event: u64) {
        self.trace.borrow_mut().extend([event, self.refs.get()]);
    }
}
struct Handle(Rc<State>, bool);
impl Scene for Handle {
    fn acquire(&self) -> Option<Self> {
        self.0.record(1);
        if self.0.refs.get() == 0 {
            return None;
        }
        self.0.refs.set(self.0.refs.get() + 1);
        Some(Self(self.0.clone(), true))
    }
    fn lock(&self) {
        self.0.record(2);
    }
    fn unlock(&self) {
        self.0.record(4);
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        if self.1 {
            self.0.refs.set(self.0.refs.get() - 1);
            self.0.record(5);
        }
    }
}
fn check(bits: u32, refs: u32, data: u64) {
    let state = Rc::new(State {
        refs: Cell::new(u64::from(refs)),
        trace: RefCell::new(Vec::new()),
        removed: bits & 4 != 0,
    });
    let scene = Handle(state.clone(), false);
    update((bits & 1 == 0).then_some(&scene), |received| {
        assert!(Rc::ptr_eq(&received.0, &state));
        assert_eq!(received.0.removed, bits & 4 != 0);
        state.record(3);
        state.trace.borrow_mut().push(data);
        if bits & 2 != 0 {
            state.refs.set(state.refs.get() - 1);
        }
        state.record(6);
    });
    let mut oracle = [0_u64; 16];
    // SAFETY: C owns its fixture and writes at most 14 values including length.
    unsafe { oracle_scene_atomic_trace(bits, refs, data, oracle.as_mut_ptr()) };
    let mut shim = [0_u64; 16];
    // SAFETY: the oracle owns the instrumented scene and callback for the whole
    // call; both implementations use its matching reference/lock boundary.
    unsafe {
        oracle_scene_atomic_trace_with(
            bits,
            refs,
            data,
            shim.as_mut_ptr(),
            obs_scene::ffi::atomic_update::obs_scene_atomic_update,
        );
    }
    assert_eq!(shim, oracle, "ABI shim: bits={bits} refs={refs}");
    let count = usize::try_from(oracle[0]).unwrap();
    assert_eq!(
        &*state.trace.borrow(),
        &oracle[1..=count],
        "bits={bits} refs={refs}"
    );
}
#[test]
fn characterized_edges() {
    for bits in 0..8 {
        for refs in 0..=3 {
            for data in [0, 1, u64::MAX] {
                check(bits, refs, data);
            }
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128), failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn trace_matches_original_c(bits in 0_u32..8, refs in 0_u32..1000, data in any::<u64>()) {
        check(bits, refs, data);
    }
}
