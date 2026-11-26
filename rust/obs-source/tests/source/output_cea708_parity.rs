//! Tier 3: the original C `obs_source_output_cea708()` against the Rust
//! export, each on its own real source caption fixture (the C allocator,
//! list and mutex of `source_caption_callbacks.c`, the production field
//! adapters). Both dispatches record every call: which callback, its
//! param, the source and packet it got and whether the list mutex was held.
//! No intentional differences or exclusions.
use core::ffi::c_void;
use obs_source::ffi::source_caption_callbacks::{Callback, obs_source_add_caption_callback as add};
use obs_source::ffi::source_output_cea708::obs_source_output_cea708 as rust_output;
use proptest::prelude::*;
use std::cell::RefCell;
use std::sync::mpsc;

unsafe extern "C" {
    fn oracle_caption_new() -> *mut c_void;
    fn oracle_caption_free(source: *mut c_void);
    fn oracle_caption_count(source: *mut c_void) -> usize;
    fn oracle_caption_unlocked(source: *mut c_void) -> bool;
    fn oracle_cea708_set_destroying(source: *mut c_void, destroying: core::ffi::c_long);
    fn oracle_cea708_push(source: *mut c_void, callback: Callback, param: *mut c_void);
    fn oracle_obs_source_output_cea708(source: *mut c_void, captions: *const c_void);
    fn obs_source_caption_callback_lock(source: *mut c_void);
    fn obs_source_caption_callback_unlock(source: *mut c_void);
}

type Output = unsafe extern "C" fn(*mut c_void, *const c_void);

struct Fixture(*mut c_void);
impl Fixture {
    fn new() -> Self {
        // SAFETY: C allocates and initializes a real source caption fixture.
        let fixture = Self(unsafe { oracle_caption_new() });
        assert!(!fixture.0.is_null());
        fixture
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: owns the source, list and initialized mutex.
        unsafe { oracle_caption_free(self.0) };
    }
}

/// One recorded call: callback id, param token (or usize::MAX for NULL),
/// whether the source and packet were the dispatched ones, and whether the
/// list mutex was held.
type Call = (u8, usize, bool, bool, bool);

struct Run {
    source: *mut c_void,
    captions: *const c_void,
    calls: Vec<Call>,
    /// Callback 3 appends this many entries of callback 2.
    grow: usize,
}

thread_local! {
    static RUN: RefCell<Option<Run>> = const { RefCell::new(None) };
}

static TOKENS: [u8; 4] = [0; 4];

fn token(param: *mut c_void) -> usize {
    if param.is_null() {
        return usize::MAX;
    }
    TOKENS
        .iter()
        .position(|t| core::ptr::eq(t, param.cast::<u8>()))
        .expect("a token")
}

fn record(id: u8, param: *mut c_void, source: *mut c_void, captions: *const c_void) {
    // SAFETY: the dispatched source is a live fixture.
    let locked = !unsafe { oracle_caption_unlocked(source) };
    RUN.with(|run| {
        let mut run = run.borrow_mut();
        let run = run.as_mut().expect("a run");
        run.calls.push((
            id,
            token(param),
            source == run.source,
            captions == run.captions,
            locked,
        ));
        if id == 3 {
            for _ in 0..run.grow {
                // SAFETY: the dispatch holds the live fixture's mutex.
                unsafe { oracle_cea708_push(source, Some(callback_b), param) };
            }
        }
    });
}

unsafe extern "C" fn callback_a(param: *mut c_void, source: *mut c_void, captions: *const c_void) {
    record(1, param, source, captions);
}
unsafe extern "C" fn callback_b(param: *mut c_void, source: *mut c_void, captions: *const c_void) {
    record(2, param, source, captions);
}
unsafe extern "C" fn callback_grow(
    param: *mut c_void,
    source: *mut c_void,
    captions: *const c_void,
) {
    record(3, param, source, captions);
}

fn callback(id: u8) -> Callback {
    match id % 3 {
        0 => Some(callback_a),
        1 => Some(callback_b),
        _ => Some(callback_grow),
    }
}

fn param(index: u8) -> *mut c_void {
    match TOKENS.get(usize::from(index % 5)) {
        Some(token) => core::ptr::from_ref(token).cast_mut().cast(),
        None => core::ptr::null_mut(),
    }
}

/// Holds the fixture's list mutex on another thread while `body` runs, so
/// a dispatch that took it would block.
fn while_held(source: *mut c_void, body: impl FnOnce()) {
    let (held_tx, held_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let address = source as usize;
    let holder = std::thread::spawn(move || {
        let source = address as *mut c_void;
        // SAFETY: the fixture outlives this thread (joined below).
        unsafe { obs_source_caption_callback_lock(source) };
        held_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        // SAFETY: this thread took the mutex above.
        unsafe { obs_source_caption_callback_unlock(source) };
    });
    held_rx.recv().unwrap();
    body();
    release_tx.send(()).unwrap();
    holder.join().unwrap();
}

struct Case<'a> {
    entries: &'a [(u8, u8)],
    destroying: bool,
    null_captions: bool,
    grow: usize,
}

fn dispatch(output: Output, case: &Case<'_>) -> (Vec<Call>, usize) {
    let fixture = Fixture::new();
    let packet = [0xfcu8, 0x94, 0x2c];
    let captions: *const c_void = if case.null_captions {
        core::ptr::null()
    } else {
        packet.as_ptr().cast()
    };
    for &(id, token) in case.entries {
        // SAFETY: a live fixture; registration through the Rust export.
        unsafe { add(fixture.0, callback(id), param(token)) };
    }
    if case.destroying {
        // SAFETY: a live fixture.
        unsafe { oracle_cea708_set_destroying(fixture.0, 1) };
    }
    RUN.with(|run| {
        *run.borrow_mut() = Some(Run {
            source: fixture.0,
            captions,
            calls: Vec::new(),
            grow: case.grow,
        });
    });
    let early = case.destroying || case.null_captions;
    // SAFETY: a live fixture; every registered callback is a recorder.
    let call = || unsafe { output(fixture.0, captions) };
    if early {
        while_held(fixture.0, call);
    } else {
        call();
    }
    // SAFETY: a live fixture.
    assert!(unsafe { oracle_caption_unlocked(fixture.0) });
    // SAFETY: a live fixture.
    let count = unsafe { oracle_caption_count(fixture.0) };
    let calls = RUN.with(|run| run.borrow_mut().take().expect("a run").calls);
    (calls, count)
}

fn check(case: &Case<'_>) -> Vec<Call> {
    let c = dispatch(oracle_obs_source_output_cea708, case);
    let r = dispatch(rust_output, case);
    assert_eq!(r, c);
    c.0
}

#[test]
fn characterized_edges() {
    let list = [(0, 0), (1, 0), (0, 1), (0, 0), (1, 4)];
    for destroying in [false, true] {
        for null_captions in [false, true] {
            let calls = check(&Case {
                entries: &list,
                destroying,
                null_captions,
                grow: 0,
            });
            assert_eq!(calls.is_empty(), destroying || null_captions);
        }
    }
    let calls = check(&Case {
        entries: &list,
        destroying: false,
        null_captions: false,
        grow: 0,
    });
    assert_eq!(
        calls.iter().map(|c| (c.0, c.1)).collect::<Vec<_>>(),
        [(2, usize::MAX), (1, 0), (1, 1), (2, 0), (1, 0)]
    );
    assert!(calls.iter().all(|c| c.2 && c.3 && c.4));
    assert!(
        check(&Case {
            entries: &[],
            destroying: false,
            null_captions: false,
            grow: 0,
        })
        .is_empty()
    );
}

#[test]
fn growth_during_dispatch() {
    for grow in [1, 2, 7, 64, 300] {
        let calls = check(&Case {
            entries: &[(0, 0), (0, 1), (2, 2)],
            destroying: false,
            null_captions: false,
            grow,
        });
        assert_eq!(
            calls.iter().map(|c| (c.0, c.1)).collect::<Vec<_>>(),
            [(3, 2), (1, 1), (1, 0)]
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn lists(
        entries in prop::collection::vec((0u8..3, 0u8..5), 0..48),
        destroying in any::<bool>(),
        null_captions in any::<bool>(),
        grow in 0usize..40,
    ) {
        check(&Case { entries: &entries, destroying, null_captions, grow });
    }
}
