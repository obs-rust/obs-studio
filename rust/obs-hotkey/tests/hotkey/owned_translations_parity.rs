//! Tier 3 parity: the shims against the original
//! `libobs/obs-hotkey-owned-translations.c.h`
//! (oracle/hotkey_owned_translations.c), both over the real fields adapter
//! and a test-owned `struct obs_core`.
//!
//! - The layout of the fields and of the adapter's slot and pointer types.
//! - Every slot set from a string, the empty string and NULL, alone and
//!   together, in both groups; a proptest over sequences of up to 23 calls
//!   of either setter with random bytes (embedded NULs too) or NULL. After
//!   each call, the eight strings, and that no field keeps the caller's
//!   pointer.
//! - The live bmem allocations after each call (`bnum_allocs`, in a child
//!   process of its own, as it counts process-wide), so an old copy left
//!   unfreed or freed twice shows.
//!
//! No intentional difference: nothing is excluded.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.
use core::ffi::{c_char, c_long};
use obs_hotkey::ffi::owned_translations::{
    obs_hotkeys_set_audio_hotkeys_translations as audio,
    obs_hotkeys_set_sceneitem_hotkeys_translations as scene,
};
use obs_util::ffi::bmem::bnum_allocs;
use proptest::prelude::*;
use std::ffi::CStr;

unsafe extern "C" {
    fn obs_hotkey_owned_translation_field(slot: usize) -> *mut *mut c_char;
    fn oracle_obs_hotkeys_set_audio_hotkeys_translations(
        a: *const c_char,
        b: *const c_char,
        c: *const c_char,
        d: *const c_char,
        e: *const c_char,
        f: *const c_char,
    );
    fn oracle_obs_hotkeys_set_sceneitem_hotkeys_translations(a: *const c_char, b: *const c_char);
    fn oracle_hotkey_owned_translations_clear();
    fn oracle_hotkey_owned_translations_layout(out: *mut usize);
}

#[test]
fn real_header_pointer_and_index_layout() {
    let mut layout = [0; 6];
    // SAFETY: the C oracle writes exactly six size_t measurements.
    unsafe { oracle_hotkey_owned_translations_layout(layout.as_mut_ptr()) };
    use core::mem::{align_of, size_of};
    assert_eq!(
        layout,
        [
            size_of::<*mut c_char>(),
            align_of::<*mut c_char>(),
            size_of::<*mut *mut c_char>(),
            align_of::<*mut *mut c_char>(),
            size_of::<usize>(),
            align_of::<usize>()
        ]
    );
}

type Step = (bool, [Option<Vec<u8>>; 6]);
struct World;
impl Drop for World {
    fn drop(&mut self) {
        // SAFETY: this thread owns all eight fixture fields and their bmem storage.
        unsafe { oracle_hotkey_owned_translations_clear() };
    }
}

type Snapshot = [Option<Vec<u8>>; 8];

fn run(steps: &[Step], rust: bool) -> (Vec<Snapshot>, Vec<c_long>) {
    let _world = World;
    let mut snapshots = Vec::new();
    let mut live = Vec::new();
    // SAFETY: a counter read; it allocates nothing.
    let base = unsafe { bnum_allocs() };
    for (is_audio, values) in steps {
        let mut buffers = values.clone().map(|v| {
            v.map(|mut bytes| {
                bytes.push(0);
                bytes
            })
        });
        let pointers = buffers.each_ref().map(|v| {
            v.as_ref()
                .map_or(core::ptr::null(), |v| v.as_ptr().cast::<c_char>())
        });
        // SAFETY: all inputs are null or terminated, disjoint from the owned
        // fixture fields. The original bodies and production adapter use the
        // same real-header thread-local core. Every allocated value is freed.
        unsafe {
            let [a, b, c, d, e, f] = pointers;
            match (*is_audio, rust) {
                (true, true) => audio(a, b, c, d, e, f),
                (true, false) => {
                    oracle_obs_hotkeys_set_audio_hotkeys_translations(a, b, c, d, e, f)
                }
                (false, true) => scene(a, b),
                (false, false) => oracle_obs_hotkeys_set_sceneitem_hotkeys_translations(a, b),
            }
            // Caller buffers can change immediately after the call.
            for bytes in buffers.iter_mut().flatten() {
                bytes.fill(b'x');
            }
            snapshots.push(std::array::from_fn(|slot| {
                let ptr = *obs_hotkey_owned_translation_field(slot);
                assert!(!pointers.contains(&ptr.cast_const()) || ptr.is_null());
                (!ptr.is_null()).then(|| CStr::from_ptr(ptr).to_bytes().to_vec())
            }));
            live.push(bnum_allocs() - base);
        }
    }
    (snapshots, live)
}

fn compare(steps: &[Step]) {
    assert_eq!(run(steps, true).0, run(steps, false).0);
}

/// Strings and live allocations; only meaningful in an isolated process.
fn compare_allocations(steps: &[Step]) {
    assert_eq!(run(steps, true), run(steps, false));
}

fn steps() -> impl Strategy<Value = Vec<Step>> {
    prop::collection::vec(
        (
            any::<bool>(),
            prop::array::uniform6(prop::option::of(prop::collection::vec(any::<u8>(), 0..64))),
        ),
        1..24,
    )
}

fn fixed_steps() -> Vec<Step> {
    let filled = std::array::from_fn(|i| Some(vec![b'A' + i as u8, 0xff, b'%', b'1']));
    let empty = std::array::from_fn(|_| Some(vec![]));
    let nulls = std::array::from_fn(|_| None);
    let mut steps = vec![(true, filled), (false, empty.clone())];
    for slot in 0..6 {
        let mut mixed = empty.clone();
        mixed[slot] = None;
        steps.push((true, mixed));
    }
    steps.extend([(false, nulls.clone()), (true, nulls), (true, empty)]);
    steps
}

#[test]
fn all_slots_null_empty_replacement_and_groups_match_c() {
    compare(&fixed_steps());
}

#[test]
fn allocations_match_c() {
    // bnum_allocs is process-wide. Re-exec exactly this test so unrelated
    // category tests cannot change the count between two calls.
    let full = concat!(module_path!(), "::allocations_match_c");
    let test = full.split_once("::").unwrap().1;
    const MARKER: &str = "OBS_HOTKEY_OWNED_TRANSLATIONS_ISOLATED";
    if std::env::var(MARKER).as_deref() != Ok(test) {
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args([test, "--exact", "--test-threads=1", "--nocapture"])
            .env(MARKER, test)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success(),
            "{stdout}\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            stdout.contains("1 passed"),
            "child did not run exactly one test: {stdout}"
        );
        return;
    }
    compare_allocations(&fixed_steps());
    let config = proptest::test_runner::Config {
        cases: obs_c_oracle::parity::cases(128),
        failure_persistence: None,
        ..Default::default()
    };
    proptest::test_runner::TestRunner::new(config)
        .run(&steps(), |steps| {
            compare_allocations(&steps);
            Ok(())
        })
        .unwrap();
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn differential(steps in steps()) {
        compare(&steps);
    }
}
