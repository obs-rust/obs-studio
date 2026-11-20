//! Exact C/Rust query parity. No exclusions or intentional differences.
//!
//! The state query and the identifier lookup are the real Rust getters on the
//! shared recording fixtures of `background_state_parity` and
//! `combination_background_id_parity`, never same-named fakes: both getters
//! are defined in this crate, so a second definition would collide.
use core::ffi::{CStr, c_char, c_int, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_hotkey::background_query::NATIVE;
use obs_hotkey::ffi::{
    background_query::obs_hotkey_combination_is_background,
    background_state::obs_hotkeys_get_background_state,
    combination_background_id::obs_hotkey_combination_get_background_id,
    nix_hotkey_state::obs_key_combination,
};
use obs_util::ffi::bmem::bnum_allocs;
use obs_util::ffi::darray::bfree;
use proptest::prelude::*;
use std::cell::RefCell;

#[derive(Default)]
struct World {
    releases: usize,
}
thread_local! { static WORLD: RefCell<World> = RefCell::new(World::default()); }

unsafe extern "C" fn query_state(_: *mut c_void, flags: *mut u32) -> c_int {
    // SAFETY: the original query passes a null optional output; the getter
    // reaches only the armed per-thread state fixture.
    unsafe { obs_hotkeys_get_background_state(flags) }
}
unsafe extern "C" fn query_id(_: *mut c_void, combo: obs_key_combination) -> *mut c_char {
    // SAFETY: the generated combination uses a valid real-header key value.
    unsafe { obs_hotkey_combination_get_background_id(combo) }
}
unsafe extern "C" fn release(_: *mut c_void, id: *mut c_void) {
    WORLD.with_borrow_mut(|world| world.releases += 1);
    // SAFETY: the original query returns exactly the owned pointer, or null.
    unsafe { bfree(id) };
}
type Getter = unsafe extern "C" fn(obs_key_combination) -> *mut c_char;
unsafe extern "C" {
    fn oracle_hotkey_combination_background_id_arm(
        bits: u32,
        modifiers: u32,
        key: c_int,
        getter: Getter,
    );
    fn oracle_hotkey_combination_background_id_events(out: *mut c_char, size: usize);
    fn oracle_hotkey_background_state_arm(bits: u32, value: c_int, flags: u32);
    fn oracle_hotkey_background_state_events(out: *mut c_char, size: usize);
    fn oracle_hotkey_background_query_layout(out: *mut usize);
    fn oracle_hotkey_background_query(
        combo: obs_key_combination,
        context: *mut c_void,
        state: unsafe extern "C" fn(*mut c_void, *mut u32) -> c_int,
        id: unsafe extern "C" fn(*mut c_void, obs_key_combination) -> *mut c_char,
        release: unsafe extern "C" fn(*mut c_void, *mut c_void),
    ) -> bool;
}

fn reset(state: i32, present: bool, combo: obs_key_combination) {
    WORLD.set(World::default());
    // The state lock succeeds and holds `state`; no flags are asked for.
    // SAFETY: arms this thread's state fixture.
    unsafe { oracle_hotkey_background_state_arm(1, state, u32::MAX) };
    // Lock succeeds and a callback is registered; it fills the identifier
    // ("background-id") or succeeds with none.
    let bits = 3 | if present { 2 << 2 } else { 0 };
    // SAFETY: arms this thread's fixture; the getter only reenters on request.
    unsafe {
        oracle_hotkey_combination_background_id_arm(
            bits,
            combo.modifiers,
            combo.key,
            obs_hotkey_combination_get_background_id,
        );
    }
}

fn events(read: unsafe extern "C" fn(*mut c_char, usize)) -> String {
    let mut out = [0 as c_char; 64];
    // SAFETY: the oracle writes a terminated trace of at most 63 bytes.
    unsafe {
        read(out.as_mut_ptr(), out.len());
        CStr::from_ptr(out.as_ptr()).to_str().unwrap().to_owned()
    }
}

fn lookups() -> String {
    events(oracle_hotkey_combination_background_id_events)
}

fn queries() -> String {
    events(oracle_hotkey_background_state_events)
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 6];
    // SAFETY: the oracle writes six size_t measurements from the real header.
    unsafe { oracle_hotkey_background_query_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<obs_key_combination>(),
            align_of::<obs_key_combination>(),
            offset_of!(obs_key_combination, modifiers),
            offset_of!(obs_key_combination, key),
            NATIVE as usize,
            obs_hotkey::nix_hotkey_state::OBS_KEY_LAST_VALUE,
        ]
    );
}

fn compare(state: i32, modifiers: u32, key: i32, present: bool) {
    let combo = obs_key_combination { modifiers, key };
    // SAFETY: accounting is read only inside the isolated test process.
    let before = unsafe { bnum_allocs() };
    reset(state, present, combo);
    // SAFETY: combo contains a valid key; our hooks provide all C dependencies.
    let rust = unsafe { obs_hotkey_combination_is_background(combo) };
    let rust_queries = queries();
    let rust_lookups = lookups();
    // SAFETY: the returned identifier must have been released by the shim.
    assert_eq!(
        unsafe { bnum_allocs() },
        before,
        "Rust leaked the identifier"
    );
    reset(state, present, combo);
    // SAFETY: all callbacks are supplied here and use only the per-thread world.
    let c = unsafe {
        oracle_hotkey_background_query(combo, core::ptr::null_mut(), query_state, query_id, release)
    };
    assert_eq!(rust, c);
    assert_eq!(lookups(), rust_lookups);
    // The fixture marks a combination that differs from this one with 'K'.
    assert_eq!(rust_lookups, if state == NATIVE { "" } else { "LRUC" });
    assert_eq!(queries(), rust_queries);
    WORLD.with_borrow(|world| assert_eq!(world.releases, usize::from(state != NATIVE)));
    // SAFETY: the oracle callback returns the identifier to the same allocator.
    assert_eq!(unsafe { bnum_allocs() }, before, "C leaked the identifier");
    // One locked state read, with no flags output.
    assert_eq!(rust_queries, "LRU");
}

#[test]
fn differential() {
    // bnum_allocs is process-wide. Re-exec exactly this test so unrelated
    // category tests cannot change the baseline between lookup and free.
    let full = concat!(module_path!(), "::differential");
    let test = full.split_once("::").unwrap().1;
    const MARKER: &str = "OBS_HOTKEY_BACKGROUND_ISOLATED";
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
    for state in [i32::MIN, -1, NATIVE, 1, 2, 3, 4, 5, i32::MAX] {
        for modifiers in [0, 1, 2, 4, 8, u32::MAX] {
            for present in [false, true] {
                compare(state, modifiers, 0, present);
                compare(
                    state,
                    modifiers,
                    (obs_hotkey::nix_hotkey_state::OBS_KEY_LAST_VALUE - 1) as i32,
                    present,
                );
            }
        }
    }
    let config = proptest::test_runner::Config {
        cases: obs_c_oracle::parity::cases(512),
        failure_persistence: None,
        ..Default::default()
    };
    proptest::test_runner::TestRunner::new(config)
        .run(
            &(
                any::<i32>(),
                any::<u32>(),
                0..obs_hotkey::nix_hotkey_state::OBS_KEY_LAST_VALUE as i32,
                any::<bool>(),
            ),
            |(state, modifiers, key, present)| {
                compare(state, modifiers, key, present);
                Ok(())
            },
        )
        .unwrap();
}
