//! Tier 3 executes the extracted C against the same real source and callbacks.
use core::ffi::{c_int, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_source::ffi::source_interaction::{self as ffi, Fields, KeyEvent, MouseEvent};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

#[repr(C)]
struct Dispatchers {
    click: unsafe extern "C" fn(*mut c_void, *const MouseEvent, i32, bool, u32),
    movement: unsafe extern "C" fn(*mut c_void, *const MouseEvent, bool),
    wheel: unsafe extern "C" fn(*mut c_void, *const MouseEvent, c_int, c_int),
    focus: unsafe extern "C" fn(*mut c_void, bool),
    key: unsafe extern "C" fn(*mut c_void, *const KeyEvent, bool),
}
const RUST: Dispatchers = Dispatchers {
    click: ffi::obs_source_send_mouse_click,
    movement: ffi::obs_source_send_mouse_move,
    wheel: ffi::obs_source_send_mouse_wheel,
    focus: ffi::obs_source_send_focus,
    key: ffi::obs_source_send_key_click,
};
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_source_interaction_check(
        functions: *const Dispatchers,
        flags: u32,
        mask: u8,
        null_source: bool,
        null_data: bool,
        null_event: bool,
        kind: i32,
        toggle: bool,
        count: u32,
        x: c_int,
        y: c_int,
    ) -> bool;
    fn oracle_source_interaction_layout(out: *mut usize);
    fn oracle_source_interaction_logs(
        functions: *const Dispatchers,
        get: GetLog,
        set: SetLog,
    ) -> bool;
}

#[test]
fn public_event_and_private_adapter_layouts() {
    let rust = [
        size_of::<MouseEvent>(),
        align_of::<MouseEvent>(),
        offset_of!(MouseEvent, modifiers),
        offset_of!(MouseEvent, x),
        offset_of!(MouseEvent, y),
        size_of::<KeyEvent>(),
        align_of::<KeyEvent>(),
        offset_of!(KeyEvent, modifiers),
        offset_of!(KeyEvent, text),
        offset_of!(KeyEvent, native_modifiers),
        offset_of!(KeyEvent, native_scancode),
        offset_of!(KeyEvent, native_vkey),
        size_of::<Fields>(),
        align_of::<Fields>(),
        offset_of!(Fields, flags),
        offset_of!(Fields, data),
        offset_of!(Fields, mouse_click),
        offset_of!(Fields, mouse_move),
        offset_of!(Fields, mouse_wheel),
        offset_of!(Fields, focus),
        offset_of!(Fields, key_click),
        size_of::<Dispatchers>(),
        align_of::<Dispatchers>(),
        offset_of!(Dispatchers, click),
        offset_of!(Dispatchers, movement),
        offset_of!(Dispatchers, wheel),
        offset_of!(Dispatchers, focus),
        offset_of!(Dispatchers, key),
        obs_source::interaction::INTERACTION as usize,
    ];
    let mut c = [0; 29];
    // SAFETY: the oracle writes exactly the 29 measured values into this array.
    unsafe { oracle_source_interaction_layout(c.as_mut_ptr()) };
    assert_eq!(rust, c);
}

#[test]
fn characterized_edges_and_every_callback_mask() {
    for flags in [0, 32, !32, u32::MAX] {
        for mask in 0..32 {
            for toggle in [false, true] {
                for null_event in [false, true] {
                    // SAFETY: the oracle owns the live fixture and all borrowed payloads.
                    assert!(unsafe {
                        oracle_source_interaction_check(
                            &RUST,
                            flags,
                            mask,
                            false,
                            toggle,
                            null_event,
                            if toggle { i32::MIN } else { 2 },
                            toggle,
                            if toggle { u32::MAX } else { 0 },
                            i32::MIN,
                            i32::MAX,
                        )
                    });
                }
            }
        }
    }
}

#[test]
fn null_diagnostics_match_c() {
    if !crate::output::isolated(concat!(module_path!(), "::null_diagnostics_match_c")) {
        return;
    }
    // SAFETY: this isolated process owns the logger; the oracle restores it.
    unsafe {
        assert!(oracle_source_interaction_logs(
            &RUST,
            base_get_log_handler,
            base_set_log_handler
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn arbitrary_payloads_match_c(flags in any::<u32>(), mask in 0_u8..32,
        null_data in any::<bool>(), null_event in any::<bool>(), kind in any::<i32>(),
        toggle in any::<bool>(), count in any::<u32>(), x in any::<i32>(), y in any::<i32>()) {
        // SAFETY: the oracle supplies live source storage and exact callback signatures.
        let equal = unsafe { oracle_source_interaction_check(&RUST, flags, mask, false,
            null_data, null_event, kind, toggle, count, x, y) };
        prop_assert!(equal);
    }
}
