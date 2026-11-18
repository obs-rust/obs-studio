//! Tier 3: unchanged C, real field adapters, and the Rust shims on one C world.
//! C-undefined inputs (invalid callbacks, freed cached entries or arrays) are
//! excluded. NULL contexts are a Rust no-op extension, tested separately.
use core::ffi::c_void;
use obs_hotkey::ffi::enumeration as rs;
use obs_util::ffi::darray::darray;
use proptest::prelude::*;

type Callback = Option<unsafe extern "C" fn(*mut c_void, usize, *mut c_void) -> bool>;
type ContextCallback = Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> bool>;
unsafe extern "C" {
    fn oracle_obs_enum_hotkeys(callback: Callback, data: *mut c_void);
    fn oracle_obs_enum_hotkey_bindings(callback: Callback, data: *mut c_void);
    fn oracle_obs_hotkey_seam_enum_context(
        context: *mut c_void,
        callback: ContextCallback,
        data: *mut c_void,
    );
    fn oracle_hotkey_enum_reset(
        available: bool,
        count: usize,
        live: u32,
        ids: *const usize,
        num: usize,
    );
    fn oracle_hotkey_enum_clear();
    fn oracle_hotkey_enum_active() -> bool;
    fn oracle_hotkey_enum_lock() -> bool;
    fn oracle_hotkey_enum_unlock();
    fn oracle_hotkey_enum_find(id: usize) -> *mut c_void;
    fn oracle_hotkey_enum_bindings(callback: Callback, data: *mut c_void);
    fn oracle_hotkey_enum_first() -> *mut c_void;
    fn oracle_hotkey_enum_next(key: *mut c_void) -> *mut c_void;
    fn oracle_hotkey_enum_id(key: *mut c_void) -> usize;
    fn oracle_hotkey_enum_context_fields(context: *mut c_void) -> *const darray;
    fn oracle_hotkey_enum_context() -> *mut c_void;
    fn oracle_hotkey_enum_counts(out: *mut usize);
    fn oracle_hotkey_enum_remove(id: usize);
    fn oracle_hotkey_enum_replace_context();
    fn oracle_hotkey_enum_binding_id(binding: *mut c_void) -> usize;
}

pub fn active() -> bool {
    // SAFETY: reads only this thread's test world flag.
    unsafe { oracle_hotkey_enum_active() }
}
pub fn lock() -> Option<bool> {
    active().then(|| {
        // SAFETY: the active C world owns its simulated recursive lock.
        unsafe { oracle_hotkey_enum_lock() }
    })
}
pub fn unlock() -> bool {
    if !active() {
        return false;
    }
    // SAFETY: paired with the active world's successful acquisition.
    unsafe { oracle_hotkey_enum_unlock() };
    true
}
pub fn find(id: usize) -> Option<*mut c_void> {
    active().then(|| {
        // SAFETY: a live world owns its registry nodes.
        unsafe { oracle_hotkey_enum_find(id) }
    })
}
/// # Safety
/// The callback and data must remain valid during this synchronous seam call.
pub unsafe fn bindings(callback: Callback, data: *mut c_void) -> bool {
    if !active() {
        return false;
    }
    // SAFETY: caller supplies the synchronous callback and data.
    unsafe { oracle_hotkey_enum_bindings(callback, data) };
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_enum_first() -> *mut c_void {
    // SAFETY: the test fixture owns its thread-local core.
    unsafe { oracle_hotkey_enum_first() }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn obs_hotkey_enum_next(key: *mut c_void) -> *mut c_void {
    // SAFETY: forwarded non-null live handle from the Rust walk.
    unsafe { oracle_hotkey_enum_next(key) }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn obs_hotkey_enum_id(key: *mut c_void) -> usize {
    // SAFETY: forwarded non-null live handle from the Rust walk.
    unsafe { oracle_hotkey_enum_id(key) }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn obs_hotkey_enum_context_fields(context: *mut c_void) -> *const darray {
    assert!(
        !context.is_null(),
        "Rust must guard before the field adapter"
    );
    if active() {
        // SAFETY: active fixture owns this real C context.
        unsafe { oracle_hotkey_enum_context_fields(context) }
    } else {
        // SAFETY: save/load owns a live fake context on this thread.
        unsafe { super::save_load_parity::enum_context_fields(context) }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Record {
    visits: Vec<(usize, usize, usize)>,
    limit: usize,
    kind: u8,
    rust: bool,
    action: u8,
    nested: Vec<(usize, usize, usize)>,
}
fn counts() -> [usize; 5] {
    let mut out = [0; 5];
    // SAFETY: five writable counters, as declared in the C fixture.
    unsafe { oracle_hotkey_enum_counts(out.as_mut_ptr()) };
    out
}
unsafe extern "C" fn collect(data: *mut c_void, index: usize, key: *mut c_void) -> bool {
    // SAFETY: run() lends its stack recorder to a synchronous walk.
    let record = unsafe { &mut *data.cast::<Record>() };
    // SAFETY: each callback receives a live real-header node/binding.
    let id = unsafe {
        if record.kind == 1 {
            oracle_hotkey_enum_binding_id(key)
        } else {
            oracle_hotkey_enum_id(key)
        }
    };
    if record.kind != 1 {
        assert_eq!(index, id);
    }
    record.visits.push((index, id, counts()[2]));
    if record.visits.len() == 1 {
        if record.action == 1 && record.kind < 2 {
            let mut nested = Record {
                limit: 1,
                rust: record.rust,
                kind: record.kind,
                ..Record::default()
            };
            // SAFETY: recursive mutex remains held, and nested owns its recorder.
            unsafe { call(&mut nested) };
            record.nested = nested.visits;
        }
        if record.action == 2 && record.kind == 2 {
            // SAFETY: replacement keeps the original array alive.
            unsafe { oracle_hotkey_enum_replace_context() };
        }
        if record.action == 3 && record.kind == 2 {
            // SAFETY: remove a later live node without freeing its storage.
            unsafe { oracle_hotkey_enum_remove(1) };
        }
    }
    if record.action == 1 && record.kind == 0 {
        // SAFETY: HASH_ITER permits removing the current node.
        unsafe { oracle_hotkey_enum_remove(id) };
    }
    record.visits.len() < record.limit
}
unsafe extern "C" fn collect_context(data: *mut c_void, key: *mut c_void) -> bool {
    // SAFETY: the context walk supplies a live node and stack recorder.
    unsafe { collect(data, oracle_hotkey_enum_id(key), key) }
}
unsafe fn call(record: &mut Record) {
    let data = (record as *mut Record).cast();
    // SAFETY: fixture and recorder outlive the entire synchronous call.
    unsafe {
        match (record.rust, record.kind) {
            (true, 0) => rs::obs_enum_hotkeys(Some(collect), data),
            (false, 0) => oracle_obs_enum_hotkeys(Some(collect), data),
            (true, 1) => rs::obs_enum_hotkey_bindings(Some(collect), data),
            (false, 1) => oracle_obs_enum_hotkey_bindings(Some(collect), data),
            (true, _) => rs::obs_hotkey_seam_enum_context(
                oracle_hotkey_enum_context(),
                Some(collect_context),
                data,
            ),
            (false, _) => oracle_obs_hotkey_seam_enum_context(
                oracle_hotkey_enum_context(),
                Some(collect_context),
                data,
            ),
        }
    }
}
type Visits = Vec<(usize, usize, usize)>;
type Outcome = (Visits, Visits, [usize; 5]);

struct Case<'a> {
    available: bool,
    count: usize,
    live: u32,
    ids: &'a [usize],
    limit: usize,
    kind: u8,
    action: u8,
}

fn run(rust: bool, case: &Case<'_>) -> Outcome {
    let Case {
        available,
        count,
        live,
        ids,
        limit,
        kind,
        action,
    } = *case;
    let mut record = Record {
        rust,
        limit,
        kind,
        action,
        ..Record::default()
    };
    // SAFETY: generated sizes are bounded to the fixture's 16 nodes/32 IDs;
    // context calls emulate their caller's lock, including no lock of their own.
    unsafe {
        oracle_hotkey_enum_reset(available, count, live, ids.as_ptr(), ids.len());
        if kind == 2 {
            oracle_hotkey_enum_lock();
        }
        call(&mut record);
        if kind == 2 {
            oracle_hotkey_enum_unlock();
        }
        let result = counts();
        oracle_hotkey_enum_clear();
        (record.visits, record.nested, result)
    }
}
fn compare(
    available: bool,
    count: usize,
    live: u32,
    ids: &[usize],
    limit: usize,
    kind: u8,
    action: u8,
) {
    let case = Case {
        available,
        count,
        live,
        ids,
        limit,
        kind,
        action,
    };
    let c = run(false, &case);
    let rust = run(true, &case);
    assert_eq!(rust, c);
}

#[test]
fn characterized_edges_and_callback_reentrancy() {
    for count in [0, 1, 3, 16] {
        for limit in [0, 1, 2, 32] {
            for kind in 0..3 {
                for action in 0..4 {
                    compare(
                        true,
                        count,
                        u32::MAX,
                        &[99, 0, 1, 2, usize::MAX, 15],
                        limit,
                        kind,
                        action,
                    );
                }
                if kind != 2 {
                    compare(false, count, u32::MAX, &[], limit, kind, 0);
                }
            }
        }
    }
}

#[test]
fn null_context_does_not_call_a_field_adapter() {
    // SAFETY: NULL context is handled before any private field-address access.
    unsafe { rs::obs_hotkey_seam_enum_context(core::ptr::null_mut(), None, core::ptr::null_mut()) };
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]
    #[test]
    fn random_registry_parity(count in 0usize..=16, live in any::<u16>(), ids in proptest::collection::vec(0usize..20, 0..=32), limit in 0usize..=33, kind in 0u8..3, action in 0u8..4, available in any::<bool>()) {
        compare(available || kind == 2, count, u32::from(live), &ids, limit, kind, action);
    }
}
