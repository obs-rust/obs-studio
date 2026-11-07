//! Original-C callback parity. No intentional behavior differences.
use obs_core::ffi::module_locale_getters as rs;
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use std::cell::{Cell, RefCell};
use std::ffi::{c_char, c_void};

type Callback = unsafe extern "C" fn(*const c_char, *mut *const c_char) -> bool;
unsafe extern "C" {
    fn oracle_module_locale_getters_new(callback: Option<Callback>) -> *mut c_void;
    fn oracle_module_locale_getters_free(module: *mut c_void);
    fn oracle_module_locale_getters_field_address(module: *const c_void) -> bool;
    fn oracle_module_locale_getters_callback_layout(size: *mut usize, align: *mut usize);
    fn oracle_obs_module_get_locale_string(
        module: *const c_void,
        key: *const c_char,
        out: *mut *const c_char,
    ) -> bool;
    fn oracle_obs_module_get_locale_text(
        module: *const c_void,
        key: *const c_char,
    ) -> *const c_char;
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct State {
    mode: u8,
    value: *const c_char,
    calls: u32,
    key: *const c_char,
    null_out: bool,
    forwarded_out: bool,
}
thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    static EXPECTED_OUT: Cell<*mut *const c_char> = const { Cell::new(std::ptr::null_mut()) };
}

unsafe extern "C" fn callback(key: *const c_char, out: *mut *const c_char) -> bool {
    STATE.with_borrow_mut(|state| {
        state.calls += 1;
        state.key = key;
        state.null_out = out.is_null();
        state.forwarded_out = out == EXPECTED_OUT.get();
        let value = match state.mode {
            1 | 5 => Some(state.value),
            2 => Some(std::ptr::null()),
            3 => Some(key),
            _ => None,
        };
        if let Some(value) = value.filter(|_| !out.is_null()) {
            // SAFETY: each caller provides a writable output slot or null.
            unsafe { *out = value };
        }
        state.mode >= 2
    })
}

fn compare(flags: u8, mode: u8, bytes: &[u8]) {
    let mut key_bytes = bytes.to_vec();
    key_bytes.push(0);
    let mut value_bytes = bytes.iter().rev().copied().collect::<Vec<_>>();
    value_bytes.push(0);
    let key = if flags & 2 == 0 {
        key_bytes.as_ptr().cast()
    } else {
        std::ptr::null()
    };
    let initial = State {
        mode,
        value: value_bytes.as_ptr().cast(),
        ..State::default()
    };
    // SAFETY: C owns the opaque fixture until free; callback handles all test inputs.
    unsafe {
        let module =
            oracle_module_locale_getters_new(if flags & 1 == 0 { None } else { Some(callback) });
        assert!(!module.is_null());
        assert!(oracle_module_locale_getters_field_address(module));
        let mut c_out = c"sentinel".as_ptr();
        let mut rs_out = c_out;
        STATE.set(initial);
        let c_slot = if flags & 4 == 0 {
            &mut c_out
        } else {
            std::ptr::null_mut()
        };
        EXPECTED_OUT.set(c_slot);
        let c_result = oracle_obs_module_get_locale_string(module, key, c_slot);
        let c_state = STATE.with_borrow(|state| *state);
        STATE.set(initial);
        let rs_slot = if flags & 4 == 0 {
            &mut rs_out
        } else {
            std::ptr::null_mut()
        };
        EXPECTED_OUT.set(rs_slot);
        let rs_result = rs::obs_module_get_locale_string(module, key, rs_slot);
        assert_eq!(rs_result, c_result, "flags={flags} mode={mode}");
        assert_eq!(rs_out, c_out);
        assert_eq!(STATE.with_borrow(|state| *state), c_state);
        assert!(flags & 1 == 0 || c_state.forwarded_out);
        EXPECTED_OUT.set(std::ptr::null_mut());
        STATE.set(initial);
        let c_text = oracle_obs_module_get_locale_text(module, key);
        let c_state = STATE.with_borrow(|state| *state);
        STATE.set(initial);
        assert_eq!(
            rs::obs_module_get_locale_text(module, key),
            c_text,
            "text flags={flags} mode={mode}"
        );
        assert_eq!(STATE.with_borrow(|state| *state), c_state);
        oracle_module_locale_getters_free(module);
    }
    assert_eq!(&key_bytes[..bytes.len()], bytes);
}

#[test]
fn original_c_parity_and_real_header_layout() {
    let (mut size, mut align) = (0, 0);
    // SAFETY: both output slots are writable; C measures the real callback type.
    unsafe { oracle_module_locale_getters_callback_layout(&mut size, &mut align) };
    assert_eq!(size, std::mem::size_of::<Option<Callback>>());
    assert_eq!(align, std::mem::align_of::<Option<Callback>>());
    for flags in 0..8 {
        for mode in 0..6 {
            compare(flags, mode, &[0xff, 0, b'x']);
        }
    }
    let mut runner = TestRunner::new(Config {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..Config::default()
    });
    runner
        .run(
            &(
                0u8..8,
                0u8..6,
                proptest::collection::vec(any::<u8>(), 0..64),
            ),
            |(flags, mode, bytes)| {
                compare(flags, mode, &bytes);
                Ok(())
            },
        )
        .expect("original C locale getter parity");
}

#[test]
fn null_module_debug_assertion() {
    if !cfg!(debug_assertions) {
        return;
    }
    const PROBE: &str = "OBS_MODULE_LOCALE_NULL_PROBE";
    if let Ok(mode) = std::env::var(PROBE) {
        // SAFETY: deliberately probe the documented debug assertion in a child process.
        unsafe {
            if mode == "c" {
                oracle_obs_module_get_locale_string(
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null_mut(),
                );
            } else {
                rs::obs_module_get_locale_string(
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null_mut(),
                );
            }
        }
        std::process::exit(0);
    }
    for mode in ["c", "rust"] {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg(format!(
                "{}::null_module_debug_assertion",
                module_path!().split_once("::").unwrap().1
            ))
            .env(PROBE, mode)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(!status.success(), "{mode} accepted NULL module");
    }
}
