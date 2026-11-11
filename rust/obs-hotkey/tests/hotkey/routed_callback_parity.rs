//! Compare the ABI bridge with the unchanged extracted C under a real-header registry.
//! No intentional differences. All generated registries contain two distinct IDs.
use obs_hotkey::ffi::routed_callback::obs_hotkey_trigger_routed_callback;
use proptest::prelude::*;

type Entry = extern "C" fn(usize, bool);
unsafe extern "C" {
    fn oracle_obs_hotkey_trigger_routed_callback(id: usize, pressed: bool);
    fn oracle_routed_callback_reset(
        initialized: bool,
        enabled: bool,
        first: usize,
        second: usize,
        nested: bool,
        entry: Entry,
    );
    fn oracle_routed_callback_trace(out: *mut usize) -> usize;
}
extern "C" fn c_entry(id: usize, pressed: bool) {
    // SAFETY: reset installed the live thread-local registry for this call.
    unsafe { oracle_obs_hotkey_trigger_routed_callback(id, pressed) };
}
fn trace(
    entry: Entry,
    initialized: bool,
    enabled: bool,
    first: usize,
    id: usize,
    pressed: bool,
    nested: bool,
) -> Vec<usize> {
    let mut trace = [0; 256];
    // SAFETY: the oracle uses thread-local real C structs. The distinct IDs are
    // valid hash keys, callbacks stay within this call, and out has 256 words.
    unsafe {
        oracle_routed_callback_reset(initialized, enabled, first, first ^ 1, nested, entry);
        entry(id, pressed);
        let count = oracle_routed_callback_trace(trace.as_mut_ptr());
        trace[..count].to_vec()
    }
}
fn compare(initialized: bool, enabled: bool, first: usize, id: usize, pressed: bool, nested: bool) {
    let rust = trace(
        obs_hotkey_trigger_routed_callback,
        initialized,
        enabled,
        first,
        id,
        pressed,
        nested,
    );
    let c = trace(c_entry, initialized, enabled, first, id, pressed, nested);
    assert_eq!(rust, c);
    let callbacks = rust
        .as_chunks::<6>()
        .0
        .iter()
        .filter(|event| event[0] == 3)
        .count();
    let expected = if initialized && enabled && (id == first || id == (first ^ 1)) {
        if nested { 2 } else { 1 }
    } else {
        0
    };
    assert_eq!(callbacks, expected);
}
#[test]
fn characterized_noops_identity_press_release_and_recursive_lock() {
    for initialized in [false, true] {
        for enabled in [false, true] {
            for first in [0, 1, u32::MAX as usize, usize::MAX] {
                for id in [first, first ^ 1, first ^ 2] {
                    for pressed in [false, true] {
                        for nested in [false, true] {
                            compare(initialized, enabled, first, id, pressed, nested);
                        }
                    }
                }
            }
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn differential(initialized in any::<bool>(), enabled in any::<bool>(), first in any::<usize>(), which in 0usize..3, pressed in any::<bool>(), nested in any::<bool>()) {
        compare(initialized, enabled, first, first ^ which, pressed, nested);
    }
}
