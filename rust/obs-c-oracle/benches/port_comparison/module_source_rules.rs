//! obs-core: the source registration rules
//! (`libobs/obs-module-source-rules.c.h`), one call each, as
//! `obs_register_source_s()` makes them for one source type: the list, the
//! size, the flags, the checks (a sync video input with every callback,
//! which runs every rule) and the id, with and without a version. Each runs
//! once per registered source type, at startup and when a module loads.

use std::ffi::{CStr, c_char};

use criterion::Criterion;
use obs_c_oracle::module_source_rules::{
    oracle_obs_module_source_rules_check, oracle_obs_module_source_rules_list_of,
    oracle_obs_module_source_rules_output_flags, oracle_obs_module_source_rules_size_supported,
    oracle_obs_module_source_rules_versioned_id,
};
use obs_core::ffi::module_source_rules::{
    obs_module_source_rules_check, obs_module_source_rules_list_of,
    obs_module_source_rules_output_flags, obs_module_source_rules_size_supported,
    obs_module_source_rules_versioned_id,
};
use obs_core::module_source_rules::{
    AUDIO, HAS_AUDIO_RENDER, HAS_GET_HEIGHT, HAS_GET_NAME, HAS_GET_WIDTH, TYPE_FILTER, TYPE_INPUT,
    VIDEO,
};
use obs_util::ffi::bmem::bfree;

/// `sizeof(struct obs_source_info)` on 64-bit targets.
const SIZE: usize = 424;
const ALL: u32 = HAS_GET_NAME | HAS_GET_WIDTH | HAS_GET_HEIGHT | HAS_AUDIO_RENDER;

/// The bytes of a `bmalloc`'d id, freed.
fn take(p: *mut c_char) -> Option<Vec<u8>> {
    if p.is_null() {
        return None;
    }
    // SAFETY: a NUL-terminated `bmalloc`'d string from a shim or the oracle.
    let out = unsafe { CStr::from_ptr(p) }.to_bytes().to_vec();
    // SAFETY: allocated by `bmalloc`.
    unsafe { bfree(p.cast()) };
    Some(out)
}

pub fn benchmarks(criterion: &mut Criterion) {
    super::pair(
        criterion,
        "module_source_rules/list_of",
        || obs_module_source_rules_list_of(TYPE_FILTER),
        // SAFETY: plain values.
        || unsafe { oracle_obs_module_source_rules_list_of(TYPE_FILTER) },
    );
    super::pair(
        criterion,
        "module_source_rules/size_supported",
        || obs_module_source_rules_size_supported(SIZE, SIZE),
        // SAFETY: plain values.
        || unsafe { oracle_obs_module_source_rules_size_supported(SIZE, SIZE) },
    );
    super::pair(
        criterion,
        "module_source_rules/output_flags",
        || obs_module_source_rules_output_flags(TYPE_FILTER, AUDIO),
        // SAFETY: plain values.
        || unsafe { oracle_obs_module_source_rules_output_flags(TYPE_FILTER, AUDIO) },
    );
    super::pair(
        criterion,
        "module_source_rules/check",
        || obs_module_source_rules_check(TYPE_INPUT, VIDEO, VIDEO, ALL),
        // SAFETY: plain values.
        || unsafe { oracle_obs_module_source_rules_check(TYPE_INPUT, VIDEO, VIDEO, ALL) },
    );
    for (case, version) in [("unversioned", 0), ("versioned", 2)] {
        let id = c"color_source";
        super::pair(
            criterion,
            &format!("module_source_rules/versioned_id/{case}"),
            // SAFETY: a NUL-terminated id; the result is freed.
            || take(unsafe { obs_module_source_rules_versioned_id(id.as_ptr(), version) }),
            // SAFETY: as above.
            || take(unsafe { oracle_obs_module_source_rules_versioned_id(id.as_ptr(), version) }),
        );
    }
}
