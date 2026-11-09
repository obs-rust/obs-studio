//! Original C callbacks versus the Rust ABI dispatcher, including setter gating.
use core::ffi::c_void;
use obs_source::ffi::source_mute_monitor_hotkeys::obs_source_mute_monitor_hotkey_dispatch;
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_source_mute_monitor_hotkey(
        muted: bool,
        monitored: bool,
        monitor: bool,
        target: bool,
        pressed: bool,
        out: *mut u32,
    );
}
struct Record {
    monitor: bool,
    out: [u32; 8],
}
unsafe extern "C" fn get(source: *const c_void) -> bool {
    // SAFETY: each test passes its unique live Record for this dispatch.
    let record = unsafe { &mut *source.cast_mut().cast::<Record>() };
    record.out[3] += 1;
    record.out[if record.monitor { 2 } else { 1 }] != 0
}
unsafe extern "C" fn set(source: *mut c_void, enabled: bool) {
    // SAFETY: each test passes its unique live Record for this dispatch.
    let record = unsafe { &mut *source.cast::<Record>() };
    if record.monitor {
        record.out[2] = u32::from(enabled);
        record.out[5] += 1;
    } else {
        record.out[1] = u32::from(enabled);
        record.out[4] += 1;
        record.out[6] = 1;
        record.out[7] = u32::from(enabled);
    }
}
fn compare(muted: bool, monitored: bool, monitor: bool, target: bool, pressed: bool) {
    let mut expected = [0; 8];
    let mut actual = Record {
        monitor,
        out: [0; 8],
    };
    actual.out[1] = u32::from(muted);
    actual.out[2] = u32::from(monitored);
    // SAFETY: both calls own live fixtures and output storage. The function
    // pointers have the dispatch ABI and use the same selected source control.
    unsafe {
        oracle_source_mute_monitor_hotkey(
            muted,
            monitored,
            monitor,
            target,
            pressed,
            expected.as_mut_ptr(),
        );
        actual.out[0] = u32::from(obs_source_mute_monitor_hotkey_dispatch(
            (&raw mut actual).cast(),
            monitor,
            target,
            pressed,
            get,
            set,
        ));
    }
    assert_eq!(
        actual.out, expected,
        "mute={muted} monitor={monitored} control={monitor} target={target} pressed={pressed}"
    );
}
#[test]
fn original_c_callback_matrix() {
    for muted in [false, true] {
        for monitored in [false, true] {
            for monitor in [false, true] {
                for target in [false, true] {
                    for pressed in [false, true] {
                        compare(muted, monitored, monitor, target, pressed);
                    }
                }
            }
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(64)))]
    #[test]
    fn original_c_random_callbacks(muted: bool, monitored: bool, monitor: bool, target: bool, pressed: bool) {
        compare(muted, monitored, monitor, target, pressed);
    }
}
