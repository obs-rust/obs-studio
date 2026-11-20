//! Tier 3: the original extracted C query versus the shim on one recording
//! fixture, through the same real-named lock, field and unlock adapters.
//! Compares the full trace (including when the optional output is cleared and
//! stored), the returned state and the output. No intentional differences.
//! Mutants show the probe catches each broken contract.
use core::ffi::{CStr, c_char, c_int};
use core::mem::{align_of, size_of};
use obs_hotkey::background_state::NATIVE;
use obs_hotkey::ffi::background_state::obs_hotkeys_get_background_state;
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*mut u32) -> c_int;
unsafe extern "C" {
    fn oracle_hotkey_background_state_probe(
        bits: u32,
        value: c_int,
        flags: u32,
        getter: Getter,
    ) -> u32;
    fn oracle_hotkey_background_state_trace(
        bits: u32,
        value: c_int,
        flags: u32,
        getter: Option<Getter>,
        out: *mut c_char,
        size: usize,
        output: *mut u32,
    ) -> c_int;
    fn oracle_hotkey_background_state_layout(out: *mut usize);
    fn obs_hotkeys_background_state_lock() -> bool;
    fn obs_hotkeys_background_state_read() -> c_int;
    fn obs_hotkeys_background_state_flags() -> u32;
    fn obs_hotkeys_background_state_unlock();
}

/// Bits: 1 the lock succeeds, 2 the caller passes a flags output.
const SUCCESS: u32 = 1;
const OUTPUT: u32 = 2;
/// The fixture's output before the call, and what a NULL output reports.
const UNTOUCHED: u32 = 0x5eed_5eed;

fn probe(bits: u32, value: c_int, flags: u32, getter: Getter) -> u32 {
    // SAFETY: the oracle owns the whole fixture and passes the getter NULL or
    // its own output; every adapter the getter reaches records into it.
    unsafe { oracle_hotkey_background_state_probe(bits, value, flags, getter) }
}

fn check(bits: u32, value: c_int, flags: u32) {
    assert_eq!(
        probe(bits, value, flags, obs_hotkeys_get_background_state),
        0,
        "bits={bits:#x} value={value} flags={flags:#x}"
    );
}

fn trace(bits: u32, value: c_int, flags: u32, getter: Option<Getter>) -> (String, c_int, u32) {
    let mut out = [0 as c_char; 64];
    let mut output = 0;
    // SAFETY: the oracle writes a terminated trace of at most 63 bytes and
    // one output value.
    unsafe {
        let state = oracle_hotkey_background_state_trace(
            bits,
            value,
            flags,
            getter,
            out.as_mut_ptr(),
            out.len(),
            &mut output,
        );
        let text = CStr::from_ptr(out.as_ptr()).to_str().unwrap().to_owned();
        (text, state, output)
    }
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 6];
    // SAFETY: the oracle writes six size_t measurements from the real headers.
    unsafe { oracle_hotkey_background_state_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<c_int>(),
            align_of::<c_int>(),
            size_of::<u32>(),
            NATIVE as usize,
            8,
            UNTOUCHED as usize,
        ]
    );
}

#[test]
fn original_traces_match_the_characterization() {
    // The C characterization's expected sequences, from the original itself.
    for (bits, expected, state, output) in [
        (0, "L", NATIVE, UNTOUCHED),
        (OUTPUT, "L", NATIVE, 0),
        (SUCCESS, "LRU", 12345, UNTOUCHED),
        (SUCCESS | OUTPUT, "LRRU", 12345, 0xa5a5_1234),
    ] {
        let want = (expected.to_owned(), state, output);
        assert_eq!(
            trace(bits, 12345, 0xa5a5_1234, None),
            want,
            "bits={bits:#x}"
        );
        assert_eq!(
            trace(
                bits,
                12345,
                0xa5a5_1234,
                Some(obs_hotkeys_get_background_state)
            ),
            want,
            "bits={bits:#x}"
        );
    }
}

#[test]
fn characterized_edges() {
    // Every declared state, an unknown and the extremes, with the C test's flags.
    let states = [0, 1, 2, 3, 4, 5, 6, 7, 8, 12345, -1, i32::MIN, i32::MAX];
    let flags = [0, 1, 0x8000_0000, 0xa5a5_1234, u32::MAX];
    for bits in 0..4 {
        for state in states {
            for flag in flags {
                check(bits, state, flag);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256), failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn get_background_state_matches_c(bits in 0..4u32, value in any::<c_int>(), flags in any::<u32>()) {
        check(bits, value, flags);
    }
}

/// Write the output the way the C does, through a raw pointer.
unsafe fn store(flags: *mut u32, value: u32) {
    if !flags.is_null() {
        // SAFETY: the fixture passes NULL or its own writable output.
        unsafe { *flags = value };
    }
}

unsafe extern "C" fn skips_clear(flags: *mut u32) -> c_int {
    // SAFETY: fixture adapters; the output is deliberately left uncleared.
    unsafe {
        if !obs_hotkeys_background_state_lock() {
            return NATIVE;
        }
        let state = obs_hotkeys_background_state_read();
        store(flags, obs_hotkeys_background_state_flags());
        obs_hotkeys_background_state_unlock();
        state
    }
}

unsafe extern "C" fn reads_on_failed_lock(flags: *mut u32) -> c_int {
    // SAFETY: fixture adapters; the state is deliberately read without the lock.
    unsafe {
        store(flags, 0);
        if !obs_hotkeys_background_state_lock() {
            let _ = obs_hotkeys_background_state_read();
            return NATIVE;
        }
        obs_hotkeys_background_state_unlock();
        obs_hotkeys_get_background_state(flags)
    }
}

unsafe extern "C" fn unlocks_failed_lock(flags: *mut u32) -> c_int {
    // SAFETY: fixture adapters; a failed lock is deliberately released.
    unsafe {
        store(flags, 0);
        if !obs_hotkeys_background_state_lock() {
            obs_hotkeys_background_state_unlock();
            return NATIVE;
        }
        obs_hotkeys_background_state_unlock();
        obs_hotkeys_get_background_state(flags)
    }
}

unsafe extern "C" fn reads_state_after_unlock(flags: *mut u32) -> c_int {
    // SAFETY: fixture adapters; the state is deliberately read after unlock.
    unsafe {
        store(flags, 0);
        if !obs_hotkeys_background_state_lock() {
            return NATIVE;
        }
        if !flags.is_null() {
            store(flags, obs_hotkeys_background_state_flags());
        }
        obs_hotkeys_background_state_unlock();
        obs_hotkeys_background_state_read()
    }
}

unsafe extern "C" fn stores_flags_after_unlock(flags: *mut u32) -> c_int {
    // SAFETY: fixture adapters; the flags are deliberately stored after unlock.
    unsafe {
        store(flags, 0);
        if !obs_hotkeys_background_state_lock() {
            return NATIVE;
        }
        let state = obs_hotkeys_background_state_read();
        let value = if flags.is_null() {
            0
        } else {
            obs_hotkeys_background_state_flags()
        };
        obs_hotkeys_background_state_unlock();
        store(flags, value);
        state
    }
}

unsafe extern "C" fn masks_flags(flags: *mut u32) -> c_int {
    // SAFETY: as the shim, then the stored flags deliberately lose high bits.
    unsafe {
        let state = obs_hotkeys_get_background_state(flags);
        if !flags.is_null() {
            *flags &= 0x3f;
        }
        state
    }
}

unsafe extern "C" fn clamps_unknown(flags: *mut u32) -> c_int {
    // SAFETY: as the shim, then unknown states deliberately become NATIVE.
    let state = unsafe { obs_hotkeys_get_background_state(flags) };
    if (0..=8).contains(&state) {
        state
    } else {
        NATIVE
    }
}

unsafe extern "C" fn reports_state_on_failed_lock(flags: *mut u32) -> c_int {
    // SAFETY: as the shim; a failed lock deliberately reports ACTIVE.
    unsafe {
        store(flags, 0);
        if !obs_hotkeys_background_state_lock() {
            return 5;
        }
        obs_hotkeys_background_state_unlock();
        obs_hotkeys_get_background_state(flags)
    }
}

#[test]
fn mutants_are_caught() {
    let all = SUCCESS | OUTPUT;
    assert_eq!(probe(all, 5, 0xa5a5_1234, skips_clear) & 1, 1);
    assert_eq!(probe(OUTPUT, 5, 0xa5a5_1234, skips_clear) & 4, 4);
    assert_eq!(probe(0, 5, 0, reads_on_failed_lock) & 1, 1);
    assert_eq!(probe(0, 5, 0, unlocks_failed_lock) & 1, 1);
    assert_eq!(probe(SUCCESS, 5, 0, reads_state_after_unlock) & 3, 3);
    assert_eq!(probe(all, 5, 0xa5a5_1234, stores_flags_after_unlock) & 1, 1);
    assert_eq!(probe(all, 5, u32::MAX, masks_flags) & 4, 4);
    assert_eq!(probe(SUCCESS, 12345, 0, clamps_unknown) & 2, 2);
    assert_eq!(probe(OUTPUT, 5, 0, reports_state_on_failed_lock) & 2, 2);
    for bits in 0..4 {
        check(bits, 5, 0xa5a5_1234);
    }
}
