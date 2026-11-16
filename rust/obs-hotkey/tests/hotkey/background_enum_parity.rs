//! Original-C parity for #4798. No exclusions or intentional differences.
//!
//! Both sides walk the same thread-local real-header core: the oracle's
//! renamed original C, and the Rust export through the real field adapter.
use core::ffi::{c_char, c_void};
use obs_hotkey::ffi::background_enum::{
    obs_hotkey_background_shortcut, obs_hotkeys_enum_background_shortcuts,
};
use obs_hotkey::ffi::nix_hotkey_state::obs_key_combination;
use obs_util::ffi::darray::darray;
use proptest::prelude::*;
use std::ffi::CString;

unsafe extern "C" {
    fn oracle_obs_hotkeys_enum_background_shortcuts(
        func: Option<
            unsafe extern "C" fn(*mut c_void, obs_key_combination, *const c_char, bool) -> bool,
        >,
        data: *mut c_void,
    );
    fn oracle_hotkey_background_enum_set(
        available: bool,
        array: *mut obs_hotkey_background_shortcut,
        num: usize,
    );
    fn oracle_hotkey_background_enum_replace(
        array: *mut obs_hotkey_background_shortcut,
        num: usize,
    );
    fn oracle_hotkey_background_enum_held() -> bool;
    fn oracle_hotkey_background_enum_counts(out: *mut usize);
    fn oracle_hotkey_background_enum_list() -> *const darray;
    fn oracle_hotkey_background_enum_layout(out: *mut usize);
    fn obs_hotkey_background_enum_lock() -> bool;
    fn obs_hotkey_background_enum_fields() -> *const darray;
    fn obs_hotkey_background_enum_unlock();
}

#[derive(Debug, PartialEq, Eq)]
struct Call {
    modifiers: u32,
    key: i32,
    trigger: usize,
    bound: bool,
    held: bool,
    data: usize,
}

struct Recorder {
    calls: Vec<Call>,
    stop_after: usize,
    /// Replace the live list on this call (0-based) with `replacement`.
    replace_at: Option<usize>,
    replacement: Vec<obs_hotkey_background_shortcut>,
}

unsafe extern "C" fn record(
    data: *mut c_void,
    combo: obs_key_combination,
    trigger: *const c_char,
    bound: bool,
) -> bool {
    // SAFETY: every enumeration below passes a live Recorder as data.
    let recorder = unsafe { &mut *data.cast::<Recorder>() };
    let index = recorder.calls.len();
    recorder.calls.push(Call {
        modifiers: combo.modifiers,
        key: combo.key,
        trigger: trigger as usize,
        bound,
        // SAFETY: reads this thread's fixture flag.
        held: unsafe { oracle_hotkey_background_enum_held() },
        data: data as usize,
    });
    if recorder.replace_at == Some(index) {
        // SAFETY: the mutex is held during callbacks; the replacement lives
        // in the recorder for the whole enumeration.
        unsafe {
            oracle_hotkey_background_enum_replace(
                recorder.replacement.as_mut_ptr(),
                recorder.replacement.len(),
            );
        }
    }
    recorder.calls.len() < recorder.stop_after
}

fn counts() -> [usize; 2] {
    let mut counts = [0; 2];
    // SAFETY: C writes two counters from this thread's fixture.
    unsafe { oracle_hotkey_background_enum_counts(counts.as_mut_ptr()) };
    counts
}

type Outcome = (Vec<Call>, [usize; 2]);

fn run(
    original: bool,
    available: bool,
    callback: bool,
    list: &mut [obs_hotkey_background_shortcut],
    stop_after: usize,
    replace_at: Option<usize>,
    replacement: &[obs_hotkey_background_shortcut],
) -> Outcome {
    let mut recorder = Recorder {
        calls: vec![],
        stop_after,
        replace_at,
        replacement: replacement.to_vec(),
    };
    // SAFETY: the list outlives the enumeration; the fixture is unlocked.
    unsafe { oracle_hotkey_background_enum_set(available, list.as_mut_ptr(), list.len()) };
    let func = callback.then_some(
        record
            as unsafe extern "C" fn(*mut c_void, obs_key_combination, *const c_char, bool) -> bool,
    );
    let data = (&raw mut recorder).cast::<c_void>();
    // SAFETY: record accepts this Recorder; both read this thread's fixture.
    unsafe {
        if original {
            oracle_obs_hotkeys_enum_background_shortcuts(func, data);
        } else {
            obs_hotkeys_enum_background_shortcuts(func, data);
        }
    }
    // SAFETY: reads this thread's fixture flag.
    assert!(!unsafe { oracle_hotkey_background_enum_held() });
    (recorder.calls, counts())
}

fn compare(
    available: bool,
    callback: bool,
    mut list: Vec<obs_hotkey_background_shortcut>,
    stop_after: usize,
    replace_at: Option<usize>,
    replacement: Vec<obs_hotkey_background_shortcut>,
) -> Outcome {
    let c = run(
        true,
        available,
        callback,
        &mut list,
        stop_after,
        replace_at,
        &replacement,
    );
    let rust = run(
        false,
        available,
        callback,
        &mut list,
        stop_after,
        replace_at,
        &replacement,
    );
    assert_eq!(rust, c);
    // A NULL callback never locks; otherwise one lock, and one unlock if held.
    let locked = usize::from(callback);
    assert_eq!(c.1, [locked, usize::from(callback && available)]);
    assert!(c.0.iter().all(|call| call.held));
    // SAFETY: detach the borrowed list before it is dropped.
    unsafe { oracle_hotkey_background_enum_set(false, core::ptr::null_mut(), 0) };
    c
}

/// A fixed pool of borrowed triggers; an index past the end stands for NULL.
static TRIGGERS: std::sync::LazyLock<[CString; 3]> = std::sync::LazyLock::new(|| {
    [
        CString::new("").unwrap(),
        CString::new(vec![b'x', 0xff]).unwrap(),
        CString::new("<Control>a").unwrap(),
    ]
});

fn trigger(index: usize) -> *mut c_char {
    TRIGGERS
        .get(index)
        .map_or(core::ptr::null_mut(), |s| s.as_ptr().cast_mut())
}

fn shortcut(
    modifiers: u32,
    key: i32,
    trigger: *mut c_char,
    bound: bool,
) -> obs_hotkey_background_shortcut {
    obs_hotkey_background_shortcut {
        combo: obs_key_combination { modifiers, key },
        trigger,
        bound,
    }
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 8];
    // SAFETY: C writes eight measurements of the real headers.
    unsafe { oracle_hotkey_background_enum_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<obs_hotkey_background_shortcut>(),
            align_of::<obs_hotkey_background_shortcut>(),
            core::mem::offset_of!(obs_hotkey_background_shortcut, combo),
            core::mem::offset_of!(obs_hotkey_background_shortcut, trigger),
            core::mem::offset_of!(obs_hotkey_background_shortcut, bound),
            size_of::<obs_key_combination>(),
            0,
            0,
        ]
    );
    assert_eq!(core::mem::offset_of!(darray, array), 0);
    assert_eq!(core::mem::offset_of!(darray, num), size_of::<*mut c_void>());
}

#[test]
fn adapter_borrows_the_live_list() {
    let mut list = vec![shortcut(1, 2, trigger(1), true)];
    // SAFETY: the list outlives the checks; the fixture is unlocked.
    unsafe { oracle_hotkey_background_enum_set(true, list.as_mut_ptr(), list.len()) };
    // SAFETY: the real adapter is read only between its own lock and unlock.
    unsafe {
        assert!(obs_hotkey_background_enum_lock());
        let fields = obs_hotkey_background_enum_fields();
        assert_eq!(fields, oracle_hotkey_background_enum_list());
        assert_eq!((*fields).array, list.as_mut_ptr().cast::<c_void>());
        assert_eq!((*fields).num, 1);
        obs_hotkey_background_enum_unlock();
    }
    assert_eq!(counts(), [1, 1]);
    // SAFETY: an unavailable core fails the lock and reads nothing.
    unsafe {
        oracle_hotkey_background_enum_set(false, core::ptr::null_mut(), 0);
        assert!(!obs_hotkey_background_enum_lock());
    }
    assert_eq!(counts(), [1, 0]);
}

/// The cases of test_hotkey_background_enum.c, against the original C.
#[test]
fn characterized_edges() {
    let ordered = vec![
        shortcut(u32::MAX, 0, core::ptr::null_mut(), false),
        shortcut(1 << 2, 65, trigger(1), true),
        shortcut(1 << 2, 65, trigger(0), false),
        shortcut(0, 300, trigger(1), true),
    ];
    for available in [false, true] {
        for callback in [false, true] {
            compare(available, callback, vec![], 99, None, vec![]);
        }
    }
    for stop in 0..=5 {
        let (calls, _) = compare(true, true, ordered.clone(), stop, None, vec![]);
        assert_eq!(calls.len(), stop.clamp(1, 4));
    }
    let replacement = vec![
        shortcut(0, 65, trigger(2), false),
        shortcut(1 << 3, 66, trigger(1), true),
        shortcut(1 << 1, 67, core::ptr::null_mut(), false),
    ];
    let (calls, _) = compare(
        true,
        true,
        ordered[..1].to_vec(),
        99,
        Some(0),
        replacement.clone(),
    );
    assert_eq!(calls.len(), 3);
    let (calls, _) = compare(true, true, ordered, 99, Some(1), replacement[..1].to_vec());
    assert_eq!(calls.len(), 2);
}

fn shortcuts() -> impl Strategy<Value = Vec<obs_hotkey_background_shortcut>> {
    prop::collection::vec(
        (any::<u32>(), any::<i32>(), 0usize..=3, any::<bool>()),
        0..8,
    )
    .prop_map(|entries| {
        entries
            .into_iter()
            .map(|(modifiers, key, index, bound)| shortcut(modifiers, key, trigger(index), bound))
            .collect()
    })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn enumeration_matches_original_c(
        available in any::<bool>(),
        callback in any::<bool>(),
        list in shortcuts(),
        stop_after in 0usize..10,
        replace_at in prop::option::of(0usize..8),
        replacement in shortcuts(),
    ) {
        compare(available, callback, list, stop_after, replace_at, replacement);
    }
}
