//! Tier 3: the verbatim C bodies and real obs-internal.h layout.
//! NULL module handles and out-of-bounds indexes are undefined in C and excluded.
//! No intended differences. Lists and ids remain owned by the fixture.
use core::{
    ffi::{c_char, c_int, c_void},
    mem::{align_of, offset_of, size_of},
    ptr,
};
use obs_c_oracle::service_getters as fixture;
use obs_output::ffi::service_module_lookup::{self as rust, ModuleIds, ObsModule};
use proptest::prelude::*;
unsafe extern "C" {
    fn oracle_module_lookup_layout(index: usize) -> usize;
    fn oracle_module_lookup_set_roots(enabled: *mut c_void, disabled: *mut c_void);
    fn oracle_obs_service_module_first(disabled: bool) -> *mut ObsModule;
    fn oracle_obs_service_module_next(module: *mut ObsModule) -> *mut ObsModule;
    fn oracle_obs_service_module_service_count(module: *mut ObsModule) -> usize;
    fn oracle_obs_service_module_service_id(module: *mut ObsModule, i: usize) -> *const c_char;
    fn oracle_obs_service_module_load_state(module: *mut ObsModule) -> c_int;
}
#[test]
fn real_header_layout() {
    let values = [
        size_of::<ObsModule>(),
        align_of::<ObsModule>(),
        offset_of!(ObsModule, mod_name),
        offset_of!(ObsModule, file),
        offset_of!(ObsModule, bin_path),
        offset_of!(ObsModule, data_path),
        offset_of!(ObsModule, module),
        offset_of!(ObsModule, loaded),
        offset_of!(ObsModule, load_state),
        offset_of!(ObsModule, load),
        offset_of!(ObsModule, unload),
        offset_of!(ObsModule, post_load),
        offset_of!(ObsModule, set_locale),
        offset_of!(ObsModule, get_string),
        offset_of!(ObsModule, free_locale),
        offset_of!(ObsModule, ver),
        offset_of!(ObsModule, set_pointer),
        offset_of!(ObsModule, name),
        offset_of!(ObsModule, description),
        offset_of!(ObsModule, author),
        offset_of!(ObsModule, metadata),
        offset_of!(ObsModule, next),
        offset_of!(ObsModule, sources),
        offset_of!(ObsModule, outputs),
        offset_of!(ObsModule, encoders),
        offset_of!(ObsModule, services),
        offset_of!(ObsModule, module_type),
        size_of::<ModuleIds>(),
        align_of::<ModuleIds>(),
        offset_of!(ModuleIds, array),
        offset_of!(ModuleIds, num),
        offset_of!(ModuleIds, capacity),
    ];
    for (i, value) in values.into_iter().enumerate() {
        // SAFETY: i is in the C layout array, which has exactly these entries.
        assert_eq!(
            value,
            unsafe { oracle_module_lookup_layout(i) },
            "layout entry {i}"
        );
    }
}
fn compare(count: usize, state: i32, enabled_present: bool, disabled_present: bool) {
    let _lock = super::SERVICE_GLOBALS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let ids: Vec<_> = (0..count)
        .map(|i| {
            if i % 2 == 0 {
                c"same".as_ptr()
            } else {
                c"other".as_ptr()
            }
        })
        .collect();
    // SAFETY: fixtures own valid modules, ids live through every read, all indexes bounded.
    unsafe {
        let tail = fixture::oracle_module_new(ptr::null(), 0, 0, ptr::null_mut());
        let head = fixture::oracle_module_new(ids.as_ptr(), ids.len(), state, tail);
        let enabled = if enabled_present {
            head
        } else {
            ptr::null_mut()
        };
        let disabled = if disabled_present {
            tail
        } else {
            ptr::null_mut()
        };
        oracle_module_lookup_set_roots(enabled, disabled);
        for disabled in [false, true] {
            assert_eq!(
                rust::obs_service_module_first(disabled),
                oracle_obs_service_module_first(disabled)
            );
        }
        for module in [head.cast(), tail.cast()] {
            assert_eq!(
                rust::obs_service_module_next(module),
                oracle_obs_service_module_next(module)
            );
            let n = oracle_obs_service_module_service_count(module);
            assert_eq!(rust::obs_service_module_service_count(module), n);
            assert_eq!(
                rust::obs_service_module_load_state(module),
                oracle_obs_service_module_load_state(module)
            );
            for i in 0..n {
                assert_eq!(
                    rust::obs_service_module_service_id(module, i),
                    oracle_obs_service_module_service_id(module, i)
                );
            }
        }
        oracle_module_lookup_set_roots(ptr::null_mut(), ptr::null_mut());
        fixture::oracle_module_free(head);
        fixture::oracle_module_free(tail);
    }
}
#[test]
fn fixed_boundaries_match_c() {
    for n in [0, 1, 2, 32] {
        for state in 0..=6 {
            for a in [false, true] {
                for b in [false, true] {
                    compare(n, state, a, b);
                }
            }
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(128)))]
    #[test]
    fn generated_modules_match_c(count in 0usize..65, state in 0i32..7, enabled in any::<bool>(), disabled in any::<bool>()) {
        compare(count,state,enabled,disabled);
    }
}

#[test]
fn metadata_reads_do_not_touch_service_storage() {
    // SAFETY: these getters only read metadata, never index the NULL array.
    unsafe {
        let module = fixture::oracle_module_new(ptr::null(), usize::MAX, 6, ptr::null_mut());
        assert_eq!(
            rust::obs_service_module_service_count(module.cast()),
            usize::MAX
        );
        assert_eq!(
            rust::obs_service_module_service_count(module.cast()),
            oracle_obs_service_module_service_count(module.cast())
        );
        assert!(rust::obs_service_module_next(module.cast()).is_null());
        assert_eq!(rust::obs_service_module_load_state(module.cast()), 6);
        fixture::oracle_module_free(module);
    }
}
