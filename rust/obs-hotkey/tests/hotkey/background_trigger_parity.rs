//! Tier 3: original C, real private adapter, exact ownership and lock ordering.
use core::ffi::c_char;
use core::mem::{align_of, offset_of, size_of};
use obs_hotkey::ffi::background_trigger::obs_hotkey_combination_get_background_trigger as query;
use obs_hotkey::ffi::nix_hotkey_state::obs_key_combination as Combo;
use obs_util::ffi::bmem::bnum_allocs;
use obs_util::ffi::darray::bfree;
use obs_util::ffi::dstr::dstr;
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_hotkey_trigger_set(
        available: bool,
        callback: bool,
        found: bool,
        allocate: bool,
        bytes: *const u8,
        length: usize,
        combo: Combo,
    );
    fn oracle_hotkey_trigger_snapshot(counts: *mut usize, storage: *mut *mut c_char);
    fn oracle_hotkey_trigger_layout(out: *mut usize);
    fn oracle_obs_hotkey_combination_get_background_trigger(combo: Combo) -> *mut c_char;
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 9];
    // SAFETY: the real-header oracle writes nine size_t layout measurements.
    unsafe { oracle_hotkey_trigger_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Combo>(),
            align_of::<Combo>(),
            offset_of!(Combo, modifiers),
            offset_of!(Combo, key),
            size_of::<dstr>(),
            align_of::<dstr>(),
            offset_of!(dstr, array),
            offset_of!(dstr, len),
            offset_of!(dstr, capacity),
        ]
    );
}

fn compare(
    available: bool,
    callback: bool,
    found: bool,
    allocate: bool,
    combo: Combo,
    bytes: &[u8],
) {
    let mut results = Vec::new();
    for original in [false, true] {
        let mut counts = [0; 3];
        let mut storage = core::ptr::null_mut();
        // SAFETY: setup and dispatch use thread-local state. bytes stays live
        // until both calls finish; the callback allocates length+1 bytes.
        unsafe {
            oracle_hotkey_trigger_set(
                available,
                callback,
                found,
                allocate,
                bytes.as_ptr(),
                bytes.len(),
                combo,
            );
            let before = bnum_allocs();
            let result = if original {
                oracle_obs_hotkey_combination_get_background_trigger(combo)
            } else {
                query(combo)
            };
            oracle_hotkey_trigger_snapshot(counts.as_mut_ptr(), &raw mut storage);
            let owns = available && callback && found && allocate;
            assert_eq!(result.is_null(), !owns);
            assert_eq!(storage.is_null(), !(available && callback && allocate));
            assert_eq!(bnum_allocs(), before + core::ffi::c_long::from(owns));
            let value = if result.is_null() {
                None
            } else {
                assert_eq!(result, storage, "transfer the callback's exact allocation");
                let value =
                    core::slice::from_raw_parts(result.cast::<u8>(), bytes.len() + 1).to_vec();
                assert_eq!(&value[..bytes.len()], bytes);
                assert_eq!(value[bytes.len()], 0);
                Some(value)
            };
            bfree(result.cast());
            assert_eq!(
                bnum_allocs(),
                before,
                "rejected and transferred allocations are freed"
            );
            assert_eq!(
                counts,
                [
                    1,
                    usize::from(available),
                    usize::from(available && callback)
                ]
            );
            results.push((value, counts));
        }
    }
    assert_eq!(results[0], results[1]);
}

#[test]
fn differential() {
    // bnum_allocs is process-wide: isolate only this case in the category binary.
    let full = concat!(module_path!(), "::differential");
    let test = full.split_once("::").unwrap().1;
    const MARKER: &str = "OBS_HOTKEY_TRIGGER_ISOLATED";
    if std::env::var(MARKER).as_deref() != Ok(test) {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([test, "--exact", "--test-threads=1", "--nocapture"])
            .env(MARKER, test)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            stdout.contains("1 passed"),
            "child did not run one test: {stdout}"
        );
        return;
    }
    for available in [false, true] {
        for callback in [false, true] {
            for found in [false, true] {
                for allocate in [false, true] {
                    for combo in [
                        Combo::default(),
                        Combo {
                            modifiers: u32::MAX,
                            key: 1,
                        },
                    ] {
                        for bytes in [
                            &b""[..],
                            &b"Ctrl+Alt+Shift+A (owned trigger)"[..],
                            &[0xff, 0, 0x80][..],
                        ] {
                            compare(available, callback, found, allocate, combo, bytes);
                        }
                    }
                }
            }
        }
    }
    let strategy = (
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        any::<u32>(),
        0i32..obs_hotkey::nix_hotkey_state::OBS_KEY_LAST_VALUE as i32,
        prop::collection::vec(any::<u8>(), 0..128),
    );
    proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
        cases: obs_c_oracle::parity::cases(128),
        failure_persistence: None,
        ..Default::default()
    })
    .run(
        &strategy,
        |(available, callback, found, allocate, modifiers, key, bytes)| {
            compare(
                available,
                callback,
                found,
                allocate,
                Combo { modifiers, key },
                &bytes,
            );
            Ok(())
        },
    )
    .unwrap();
}
