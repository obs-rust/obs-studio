//! Tier 3: original C wrappers vs public Rust ABI through the same real-layout
//! adapters. No intentional differences. State is thread-local in the C oracle.
use super::register::KINDS;
use core::ffi::{c_char, c_int, c_void};
use obs_hotkey::ffi::register::{self, Callback};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_obs_hotkey_register_frontend(
        name: *const c_char,
        description: *const c_char,
        func: Callback,
        data: *mut c_void,
    ) -> usize;
    fn oracle_obs_hotkey_register_source(
        owner: *mut c_void,
        name: *const c_char,
        description: *const c_char,
        func: Callback,
        data: *mut c_void,
    ) -> usize;
    fn oracle_obs_hotkey_register_output(
        owner: *mut c_void,
        name: *const c_char,
        description: *const c_char,
        func: Callback,
        data: *mut c_void,
    ) -> usize;
    fn oracle_obs_hotkey_register_encoder(
        owner: *mut c_void,
        name: *const c_char,
        description: *const c_char,
        func: Callback,
        data: *mut c_void,
    ) -> usize;
    fn oracle_obs_hotkey_register_service(
        owner: *mut c_void,
        name: *const c_char,
        description: *const c_char,
        func: Callback,
        data: *mut c_void,
    ) -> usize;
    fn oracle_hotkey_register_reset(
        kind: c_int,
        available: bool,
        private_source: bool,
        result: usize,
        func: Callback,
    ) -> *mut c_void;
    fn oracle_hotkey_register_snapshot(trace: *mut usize, count: *mut usize, held: *mut bool);
    fn oracle_hotkey_register_values(values: *mut usize);
}

unsafe extern "C" fn callback(_: *mut c_void, _: usize, _: *mut c_void, _: bool) {}

type Owned =
    unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char, Callback, *mut c_void) -> usize;

#[derive(Clone, Debug)]
struct Input {
    kind: u8,
    present: bool,
    available: bool,
    private: bool,
    result: usize,
    null_name: bool,
    null_description: bool,
    null_callback: bool,
    null_data: bool,
}

fn run(input: &Input, rust: bool) -> (usize, [usize; 32], usize, bool) {
    let kind = KINDS[usize::from(input.kind)];
    let func: Callback = (!input.null_callback).then_some(callback);
    // SAFETY: this resets only the current thread's oracle fixture; the
    // returned owner has the real layout of the selected kind.
    let owner = unsafe {
        oracle_hotkey_register_reset(
            kind.code(),
            input.available,
            input.private,
            input.result,
            func,
        )
    };
    let owner = if input.present {
        owner
    } else {
        core::ptr::null_mut()
    };
    let name = if input.null_name {
        core::ptr::null()
    } else {
        c"registration.name".as_ptr()
    };
    let description = if input.null_description {
        core::ptr::null()
    } else {
        c"registration description".as_ptr()
    };
    let data = if input.null_data {
        core::ptr::null_mut()
    } else {
        name.cast_mut().cast::<c_void>()
    };
    let owned: [Owned; 4] = if rust {
        [
            register::obs_hotkey_register_source,
            register::obs_hotkey_register_output,
            register::obs_hotkey_register_encoder,
            register::obs_hotkey_register_service,
        ]
    } else {
        [
            oracle_obs_hotkey_register_source,
            oracle_obs_hotkey_register_output,
            oracle_obs_hotkey_register_encoder,
            oracle_obs_hotkey_register_service,
        ]
    };
    // SAFETY: owners come from the real C fixture, and the recording engine
    // borrows arguments without dereferencing nullable strings/callback/data.
    let result = unsafe {
        if input.kind == 0 {
            if rust {
                register::obs_hotkey_register_frontend(name, description, func, data)
            } else {
                oracle_obs_hotkey_register_frontend(name, description, func, data)
            }
        } else {
            owned[usize::from(input.kind - 1)](owner, name, description, func, data)
        }
    };
    let mut trace = [0; 32];
    let mut count = 0;
    let mut held = false;
    // SAFETY: all output buffers have exactly the C helper's required capacity.
    unsafe { oracle_hotkey_register_snapshot(trace.as_mut_ptr(), &mut count, &mut held) };
    (result, trace, count, held)
}

#[test]
fn real_header_id_and_kind_values() {
    let mut values = [0; 8];
    // SAFETY: the helper writes eight size_t values to this exact-size array.
    unsafe { oracle_hotkey_register_values(values.as_mut_ptr()) };
    assert_eq!(
        values,
        [
            size_of::<usize>(),
            align_of::<usize>(),
            usize::MAX,
            0,
            1,
            2,
            3,
            4
        ]
    );
    assert_eq!(KINDS.map(|kind| kind.code()), [0, 1, 2, 3, 4]);
}

#[test]
fn characterized_admission_and_full_width_results() {
    for kind in 0..5 {
        for present in [false, true] {
            for available in [false, true] {
                for private in [false, true] {
                    for result in [0, 1, usize::MAX - 1, usize::MAX] {
                        let input = Input {
                            kind,
                            present,
                            available,
                            private,
                            result,
                            null_name: false,
                            null_description: false,
                            null_callback: false,
                            null_data: false,
                        };
                        assert_eq!(run(&input, true), run(&input, false), "{input:?}");
                    }
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]
    #[test]
    fn wrappers_match_original_c(kind in 0u8..5, flags in any::<[bool; 7]>(), result in any::<usize>()) {
        let [present, available, private, null_name, null_description, null_callback, null_data] = flags;
        let input = Input { kind, present, available, private, result, null_name, null_description, null_callback, null_data };
        prop_assert_eq!(run(&input, true), run(&input, false));
    }
}
