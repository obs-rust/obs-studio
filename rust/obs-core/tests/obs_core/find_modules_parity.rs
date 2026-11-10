//! Original-C traversal over real-header TLS core/path fixtures. Populated
//! searches require a callback, as in C. No test-owned threads.
use obs_c_oracle::module_lists::*;
use obs_core::ffi::module_lists::obs_add_module_path;
use obs_core::ffi::module_paths::{
    obs_find_module_callback2_t as Callback, obs_module_add_extension_t as Hook, obs_module_info2,
};
use obs_util::ffi::dstr::dstr;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr;

type Find = unsafe extern "C" fn(
    *const c_char,
    *const c_char,
    c_int,
    *const c_char,
    Hook,
    Callback,
    *mut c_void,
);
type Enumerate = unsafe extern "C" fn(Callback, *mut c_void);
unsafe extern "C" {
    fn oracle_obs_find_modules2(callback: Callback, param: *mut c_void);
    fn oracle_find_modules_set_probe(probe: Find);
    fn oracle_find_modules_find(
        bin: *const c_char,
        data: *const c_char,
        kind: c_int,
        ext: *const c_char,
        hook: Hook,
        callback: Callback,
        param: *mut c_void,
    );
    fn oracle_find_modules_extension() -> *const c_char;
    fn oracle_find_modules_hook(path: *mut dstr);
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Outcome {
    paths: Vec<(Vec<u8>, Vec<u8>, c_int)>,
    modules: Vec<(Vec<u8>, Vec<u8>, Vec<u8>, c_int)>,
    append: bool,
    null_callback: bool,
}

unsafe fn bytes(value: *const c_char) -> Vec<u8> {
    // SAFETY: fixture paths and callback strings are live NUL-terminated data.
    unsafe { CStr::from_ptr(value).to_bytes().to_vec() }
}

unsafe extern "C" fn report(param: *mut c_void, info: *const obs_module_info2) {
    // SAFETY: the finder lends live info and the exact caller-owned Outcome.
    unsafe {
        let state = &mut *param.cast::<Outcome>();
        let info = &*info;
        state.modules.push((
            bytes(info.bin_path),
            bytes(info.data_path),
            bytes(info.name),
            info.module_type,
        ));
        if state.append && state.modules.len() == 1 {
            obs_add_module_path(c"appended".as_ptr(), c"data".as_ptr());
        }
    }
}

unsafe extern "C" fn probe(
    bin: *const c_char,
    data: *const c_char,
    kind: c_int,
    ext: *const c_char,
    hook: Hook,
    callback: Callback,
    param: *mut c_void,
) {
    // SAFETY: traversal forwards the live fixture paths and Outcome pointer.
    unsafe {
        let state = &mut *param.cast::<Outcome>();
        assert_eq!(bytes(ext), b".probe");
        assert!(std::ptr::fn_addr_eq(
            hook.unwrap(),
            oracle_find_modules_hook as unsafe extern "C" fn(*mut dstr)
        ));
        assert_eq!(callback.is_none(), state.null_callback);
        state.paths.push((bytes(bin), bytes(data), kind));
        if bytes(bin) != b"missing" {
            let info = obs_module_info2 {
                bin_path: bin,
                data_path: data,
                name: c"name".as_ptr(),
                module_type: kind,
            };
            callback.unwrap()(param, &info);
        }
    }
}

fn run(
    enumerate: Enumerate,
    present: bool,
    count: usize,
    append: bool,
    null_callback: bool,
) -> Outcome {
    // SAFETY: all operations use this thread's isolated real-header core.
    unsafe {
        let core = if present {
            oracle_module_lists_core_new(ptr::null())
        } else {
            ptr::null_mut()
        };
        oracle_module_lists_set_core(core);
        oracle_find_modules_set_probe(probe);
        for index in 0..count {
            let bin = if null_callback || index == 1 {
                c"missing"
            } else {
                c"binary"
            };
            obs_add_module_path(bin.as_ptr(), c"data".as_ptr());
        }
        let mut state = Outcome {
            append,
            null_callback,
            ..Outcome::default()
        };
        enumerate(
            if null_callback { None } else { Some(report) },
            (&raw mut state).cast(),
        );
        oracle_module_lists_set_core(ptr::null_mut());
        if !core.is_null() {
            oracle_module_lists_core_free(core);
        }
        state
    }
}

#[test]
fn original_c_characterizes_live_paths_and_callback_forwarding() {
    for present in [false, true] {
        assert!(
            run(oracle_obs_find_modules2, present, 0, false, true)
                .paths
                .is_empty()
        );
    }
    let empty = run(oracle_obs_find_modules2, true, 3, false, true);
    assert_eq!(empty.paths.len(), 3);
    assert!(empty.modules.is_empty());
    let live = run(oracle_obs_find_modules2, true, 3, true, false);
    assert_eq!(live.paths.len(), 4);
    assert_eq!(live.modules.len(), 3);
    assert_eq!(live.paths.last().unwrap().0, b"appended");
}
