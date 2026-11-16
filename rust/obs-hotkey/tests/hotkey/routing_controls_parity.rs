//! Stored pointer identity and recursive-lock parity against original C.
use core::ffi::c_void;
use obs_hotkey::ffi::routing_controls::{self as rs, Callback};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_obs_hotkey_set_callback_routing_func(callback: Callback, data: *mut c_void);
    fn oracle_obs_hotkey_enable_callback_rerouting(enabled: bool);
    fn oracle_hotkey_routing_reset(
        present: bool,
        callback: Callback,
        data: *mut c_void,
        enabled: bool,
    );
    fn oracle_hotkey_routing_snapshot(
        callback: *mut Callback,
        data: *mut *mut c_void,
        enabled: *mut bool,
        counts: *mut usize,
    );
    fn obs_hotkey_routing_lock() -> bool;
    fn obs_hotkey_routing_unlock();
}

unsafe extern "C" fn first(_: *mut c_void, _: usize, _: bool) {
    panic!("setters must not call the router");
}
unsafe extern "C" fn second(_: *mut c_void, _: usize, _: bool) {
    panic!("setters must not call the router");
}
fn callback(which: u8) -> Callback {
    match which {
        0 => None,
        1 => Some(first),
        _ => Some(second),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    callback: Option<usize>,
    data: usize,
    enabled: bool,
    counts: [usize; 3],
}
fn snapshot() -> Snapshot {
    let mut callback: Callback = None;
    let mut data = core::ptr::null_mut();
    let mut enabled = false;
    let mut counts = [0; 3];
    // SAFETY: the thread-local oracle writes into these live correctly typed outputs.
    unsafe {
        oracle_hotkey_routing_snapshot(
            &raw mut callback,
            &raw mut data,
            &raw mut enabled,
            counts.as_mut_ptr(),
        )
    };
    Snapshot {
        callback: callback.map(|f| f as usize),
        data: data as usize,
        enabled,
        counts,
    }
}

fn run(
    rust: bool,
    present: bool,
    initial: bool,
    nested: bool,
    ops: &[(bool, u8, bool, u8)],
) -> Vec<Snapshot> {
    let mut contexts = [11u8, 22, 33];
    let pointers = [
        core::ptr::null_mut(),
        (&raw mut contexts[0]).cast(),
        (&raw mut contexts[1]).cast(),
        (&raw mut contexts[2]).cast(),
    ];
    // SAFETY: all oracle state is thread-local. Pointers remain live until return;
    // setters only store them. The optional outer acquisition is released below.
    unsafe {
        oracle_hotkey_routing_reset(present, Some(first), pointers[1], initial);
        let outer = nested && obs_hotkey_routing_lock();
        let mut result = Vec::new();
        for &(router, which, enabled, context) in ops {
            if router {
                if rust {
                    rs::obs_hotkey_set_callback_routing_func(
                        callback(which),
                        pointers[usize::from(context)],
                    );
                } else {
                    oracle_obs_hotkey_set_callback_routing_func(
                        callback(which),
                        pointers[usize::from(context)],
                    );
                }
            } else if rust {
                rs::obs_hotkey_enable_callback_rerouting(enabled);
            } else {
                oracle_obs_hotkey_enable_callback_rerouting(enabled);
            }
            let mut state = snapshot();
            // Encode pointer identity by index: each run owns separate backing bytes.
            state.data = pointers
                .iter()
                .position(|p| *p as usize == state.data)
                .expect("stored context identity");
            result.push(state);
        }
        if outer {
            obs_hotkey_routing_unlock();
        }
        assert_eq!(snapshot().counts[2], 0);
        assert_eq!(contexts, [11, 22, 33]);
        result
    }
}

fn compare(present: bool, initial: bool, nested: bool, ops: &[(bool, u8, bool, u8)]) {
    let rust = run(true, present, initial, nested, ops);
    assert_eq!(rust, run(false, present, initial, nested, ops));
    let mut expected_callback = Some(first as *const () as usize);
    let mut expected_data = 1;
    let mut expected_enabled = initial;
    for (i, (&(router, which, enabled, data), state)) in ops.iter().zip(&rust).enumerate() {
        if present {
            if router {
                expected_callback = callback(which).map(|f| f as usize);
                expected_data = usize::from(data);
            } else {
                expected_enabled = enabled;
            }
        }
        assert_eq!(
            (state.callback, state.data, state.enabled),
            (expected_callback, expected_data, expected_enabled)
        );
        assert_eq!(
            state.counts,
            [
                i + 1 + usize::from(nested),
                if present { i + 1 } else { 0 },
                usize::from(nested && present)
            ]
        );
    }
}

#[test]
fn characterized_edges_and_recursive_acquisition() {
    let ops = [
        (false, 0, true, 0),
        (true, 0, false, 0),
        (true, 1, false, 2),
        (false, 0, false, 0),
        (true, 2, false, 3),
        (false, 0, true, 0),
    ];
    for present in [false, true] {
        for initial in [false, true] {
            for nested in [false, true] {
                compare(present, initial, nested, &ops);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn differential(present in any::<bool>(), initial in any::<bool>(), nested in any::<bool>(), ops in prop::collection::vec((any::<bool>(), 0u8..3, any::<bool>(), 0u8..4), 0..64)) {
        compare(present, initial, nested, &ops);
    }
}
