//! Exact pointer and synchronization parity with the original C query.
use core::ffi::c_char;
use core::mem::{align_of, size_of};
use obs_hotkey::ffi::background_app_id::obs_hotkeys_get_background_app_id;
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_hotkey_app_id_set(initialized: bool, id: *const c_char);
    fn oracle_hotkey_app_id_counts(out: *mut usize);
    fn oracle_hotkey_app_id_layout(out: *mut usize);
    fn oracle_obs_hotkeys_get_background_app_id() -> *const c_char;
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 6];
    // SAFETY: the oracle writes six size_t measurements into this array.
    unsafe { oracle_hotkey_app_id_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<*const c_char>(),
            align_of::<*const c_char>(),
            size_of::<*const *const c_char>(),
            align_of::<*const *const c_char>(),
            size_of::<bool>(),
            align_of::<bool>()
        ]
    );
}

fn compare(initialized: bool, present: bool, bytes: &[u8]) {
    let mut id = bytes.to_vec();
    id.push(0);
    let before = id.clone();
    let pointer = if present {
        id.as_ptr().cast()
    } else {
        core::ptr::null()
    };
    let mut rust_counts = [0; 3];
    let mut c_counts = [0; 3];
    // SAFETY: hooks are thread-local, and the backing bytes remain live for
    // both queries. No accessor runs when the core cannot be locked.
    unsafe {
        oracle_hotkey_app_id_set(initialized, pointer);
        let rust = obs_hotkeys_get_background_app_id();
        oracle_hotkey_app_id_counts(rust_counts.as_mut_ptr());
        oracle_hotkey_app_id_set(initialized, pointer);
        let c = oracle_obs_hotkeys_get_background_app_id();
        oracle_hotkey_app_id_counts(c_counts.as_mut_ptr());
        assert_eq!(rust, c);
        assert_eq!(
            rust,
            if initialized {
                pointer
            } else {
                core::ptr::null()
            }
        );
    }
    assert_eq!(rust_counts, c_counts);
    assert_eq!(rust_counts, if initialized { [1, 1, 1] } else { [1, 0, 0] });
    assert_eq!(id, before);
}

#[test]
fn characterized_edges() {
    for initialized in [false, true] {
        for present in [false, true] {
            for bytes in [
                &b""[..],
                &b"app-id sentinel"[..],
                &b"App-id sentinel"[..],
                &[0xff, 0, 0x80][..],
            ] {
                compare(initialized, present, bytes);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn differential(initialized in any::<bool>(), present in any::<bool>(), bytes in prop::collection::vec(any::<u8>(), 0..64)) {
        compare(initialized, present, &bytes);
    }
}
