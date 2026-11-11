//! Original-C traversal over real-header TLS core/path fixtures. Populated
//! searches require a callback, as in C. No test-owned threads.
use obs_c_oracle::module_lists::*;
use obs_core::ffi::find_modules::{
    Calls, Extension, LegacyCallback, find_legacy_with, find_with, obs_module_info,
};
use obs_core::ffi::module_lists::obs_add_module_path;
use obs_core::ffi::module_paths::{
    obs_find_module_callback2_t as Callback, obs_module_add_extension_t as Hook, obs_module_info2,
};
use obs_util::ffi::dstr::dstr;
use proptest::prelude::*;
use std::cell::Cell;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::mem::{align_of, offset_of, size_of};
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
    fn oracle_obs_find_modules(callback: LegacyCallback, param: *mut c_void);
    fn oracle_find_modules_add_path(bin: *const c_char, data: *const c_char, kind: c_int);
    fn oracle_find_modules_legacy_layout(layout: *mut usize);
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

type ModuleRecord = (Vec<u8>, Vec<u8>, Vec<u8>, c_int);

#[derive(Debug, Default, PartialEq, Eq)]
struct Outcome {
    paths: Vec<(Vec<u8>, Vec<u8>, c_int)>,
    modules: Vec<ModuleRecord>,
    append: bool,
    null_callback: bool,
    legacy: bool,
    info_address: usize,
}

enum Probe {}
impl Calls for Probe {
    fn find() -> obs_core::ffi::find_modules::Find {
        oracle_find_modules_find
    }
    fn extension() -> Extension {
        oracle_find_modules_extension
    }
    fn hook() -> Hook {
        Some(oracle_find_modules_hook)
    }
}

unsafe extern "C" fn rust_find(callback: Callback, param: *mut c_void) {
    // SAFETY: run owns the core, callback and parameter for the traversal.
    unsafe { find_with::<Probe>(callback, param) };
}

unsafe extern "C" fn legacy_report(param: *mut c_void, info: *const obs_module_info) {
    // SAFETY: the legacy callback borrows the same original info2 prefix.
    unsafe {
        let state = &mut *param.cast::<Outcome>();
        assert_eq!(state.info_address, info as usize);
        let info = &*info;
        state
            .modules
            .push((bytes(info.bin_path), bytes(info.data_path), Vec::new(), 0));
        if state.append && state.modules.len() == 1 {
            obs_add_module_path(c"appended".as_ptr(), c"data".as_ptr());
        }
    }
}

unsafe extern "C" fn legacy_original(_: Callback, param: *mut c_void) {
    // SAFETY: the caller owns the callback state and live fixture.
    unsafe {
        let callback = if (*param.cast::<Outcome>()).null_callback {
            None
        } else {
            Some(legacy_report as unsafe extern "C" fn(*mut c_void, *const obs_module_info))
        };
        oracle_obs_find_modules(callback, param);
    }
}

unsafe extern "C" fn legacy_rust(_: Callback, param: *mut c_void) {
    // SAFETY: the caller owns the callback state and live fixture.
    unsafe {
        let callback = if (*param.cast::<Outcome>()).null_callback {
            None
        } else {
            Some(legacy_report as unsafe extern "C" fn(*mut c_void, *const obs_module_info))
        };
        find_legacy_with::<Probe>(callback, param);
    }
}

unsafe fn bytes(value: *const c_char) -> Vec<u8> {
    // SAFETY: fixture paths and callback strings are live NUL-terminated data.
    unsafe { CStr::from_ptr(value).to_bytes().to_vec() }
}

unsafe extern "C" fn report(param: *mut c_void, info: *const obs_module_info2) {
    // SAFETY: the finder lends live info and the exact caller-owned Outcome.
    unsafe {
        let state = &mut *param.cast::<Outcome>();
        assert_eq!(state.info_address, info as usize);
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
        if !state.null_callback && !state.legacy {
            assert!(std::ptr::fn_addr_eq(
                callback.unwrap(),
                report as unsafe extern "C" fn(*mut c_void, *const obs_module_info2)
            ));
        }
        state.paths.push((bytes(bin), bytes(data), kind));
        if bytes(bin) != b"missing" {
            let info = obs_module_info2 {
                bin_path: bin,
                data_path: data,
                name: c"name".as_ptr(),
                module_type: kind,
            };
            state.info_address = (&raw const info) as usize;
            callback.unwrap()(param, &info);
            // Each path can report several modules, retaining its callback.
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
    run_case(enumerate, present, count, append, null_callback, false, &[])
}

fn run_case(
    enumerate: Enumerate,
    present: bool,
    count: usize,
    append: bool,
    null_callback: bool,
    legacy: bool,
    suffix: &[u8],
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
                CString::new("missing").unwrap()
            } else {
                let mut name = format!("binary-{index}").into_bytes();
                name.extend_from_slice(suffix);
                CString::new(name).unwrap()
            };
            let data = CString::new(format!("data-{index}")).unwrap();
            oracle_find_modules_add_path(bin.as_ptr(), data.as_ptr(), (index % 4) as c_int);
        }
        let mut state = Outcome {
            append,
            null_callback,
            legacy,
            ..Outcome::default()
        };
        enumerate(
            if null_callback { None } else { Some(report) },
            (&raw mut state).cast(),
        );
        state.info_address = 0;
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
    assert_eq!(live.modules.len(), 6);
    assert_eq!(live.paths.last().unwrap().0, b"appended");
}

fn check(present: bool, count: usize, append: bool, null_callback: bool, suffix: &[u8]) {
    for (original, rust, legacy) in [
        (
            oracle_obs_find_modules2 as Enumerate,
            rust_find as Enumerate,
            false,
        ),
        (legacy_original as Enumerate, legacy_rust as Enumerate, true),
    ] {
        assert_eq!(
            run_case(
                original,
                present,
                count,
                append,
                null_callback,
                legacy,
                suffix
            ),
            run_case(rust, present, count, append, null_callback, legacy, suffix)
        );
    }
}

#[test]
fn characterized_edges_legacy_prefix_and_live_path_appends_match_c() {
    check(false, 0, false, false, &[]);
    check(false, 0, false, true, &[]);
    for count in [0, 1, 2, 3, 16] {
        check(true, count, false, false, &[0xff]);
        check(true, count, true, false, &[]);
        check(true, count, false, true, &[]);
    }
}

#[test]
fn legacy_layout_matches_public_c_header() {
    let mut layout = [0; 4];
    // SAFETY: the oracle writes four size_t values from the real public header.
    unsafe { oracle_find_modules_legacy_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<obs_module_info>(),
            align_of::<obs_module_info>(),
            offset_of!(obs_module_info, bin_path),
            offset_of!(obs_module_info, data_path)
        ]
    );
    assert_eq!(
        offset_of!(obs_module_info, bin_path),
        offset_of!(obs_module_info2, bin_path)
    );
    assert_eq!(
        offset_of!(obs_module_info, data_path),
        offset_of!(obs_module_info2, data_path)
    );
}

thread_local! {
    static NULL_PARAM_CALLS: Cell<usize> = const { Cell::new(0) };
}

unsafe extern "C" fn null_param_report(param: *mut c_void, _: *const obs_module_info2) {
    assert!(param.is_null());
    NULL_PARAM_CALLS.set(NULL_PARAM_CALLS.get() + 1);
}

unsafe extern "C" fn null_param_probe(
    _: *const c_char,
    _: *const c_char,
    _: c_int,
    _: *const c_char,
    _: Hook,
    callback: Callback,
    param: *mut c_void,
) {
    assert!(param.is_null());
    // SAFETY: this test installs a callback that ignores its info pointer.
    unsafe { callback.unwrap()(param, ptr::null()) };
}

#[test]
fn null_parameter_is_forwarded_unchanged() {
    for enumerate in [
        oracle_obs_find_modules2 as Enumerate,
        rust_find as Enumerate,
    ] {
        // SAFETY: this thread owns the core fixture and synchronous callbacks.
        unsafe {
            let core = oracle_module_lists_core_new(ptr::null());
            oracle_module_lists_set_core(core);
            oracle_find_modules_add_path(c"bin".as_ptr(), c"data".as_ptr(), 0);
            oracle_find_modules_set_probe(null_param_probe);
            NULL_PARAM_CALLS.set(0);
            enumerate(Some(null_param_report), ptr::null_mut());
            assert_eq!(NULL_PARAM_CALLS.get(), 1);
            oracle_module_lists_set_core(ptr::null_mut());
            oracle_module_lists_core_free(core);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(64)))]
    #[test]
    fn arbitrary_registered_paths_match_c(count in 0usize..24, append in any::<bool>(), suffix in prop::collection::vec(1u8..=255, 0..32)) {
        check(true, count, append, false, &suffix);
    }
}

unsafe extern "C" fn capture_host_module(param: *mut c_void, info: *const obs_module_info2) {
    // SAFETY: the test owns the vector; discovery lends the info for this call.
    unsafe {
        let info = &*info;
        (*param.cast::<Vec<ModuleRecord>>()).push((
            bytes(info.bin_path), bytes(info.data_path), bytes(info.name), info.module_type,
        ));
    }
}

unsafe extern "C" fn capture_host_legacy(param: *mut c_void, info: *const obs_module_info) {
    // SAFETY: the test owns the vector; the legacy prefix is live for this call.
    unsafe {
        let info = &*info;
        (*param.cast::<Vec<(Vec<u8>, Vec<u8>)>>()).push((bytes(info.bin_path), bytes(info.data_path)));
    }
}

#[test]
fn exported_entrypoints_discover_files_with_host_extension() {
    let root = tempfile::tempdir().unwrap();
    let bin = root.path().join("bin");
    let data = root.path().join("data");
    std::fs::create_dir_all(bin.join("sample")).unwrap();
    std::fs::create_dir_all(data.join("sample")).unwrap();
    let binary = bin.join("sample/sample.probe");
    std::fs::write(&binary, b"module fixture").unwrap();
    let path = |path: &std::path::Path| CString::new(path.to_str().unwrap().replace('\\', "/")).unwrap();
    let bin_pattern = path(&bin.join("%module%"));
    let data_pattern = path(&data.join("%module%"));
    let mut modules: Vec<ModuleRecord> = Vec::new();
    let mut legacy: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    // SAFETY: this thread owns the core, paths and callback vectors throughout.
    unsafe {
        let core = oracle_module_lists_core_new(ptr::null());
        oracle_module_lists_set_core(core);
        oracle_find_modules_add_path(bin_pattern.as_ptr(), data_pattern.as_ptr(), 2);
        obs_core::ffi::find_modules::obs_find_modules2(Some(capture_host_module), (&raw mut modules).cast());
        obs_core::ffi::find_modules::obs_find_modules(Some(capture_host_legacy), (&raw mut legacy).cast());
        oracle_module_lists_set_core(ptr::null_mut());
        oracle_module_lists_core_free(core);
    }
    let expected_bin = path(&binary).into_bytes();
    let expected_data = path(&data.join("sample")).into_bytes();
    assert_eq!(modules, vec![(expected_bin.clone(), expected_data.clone(), b"sample".to_vec(), 2)]);
    assert_eq!(legacy, vec![(expected_bin, expected_data)]);
}
