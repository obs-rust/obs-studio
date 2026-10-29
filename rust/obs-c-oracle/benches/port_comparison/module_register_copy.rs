//! obs-core: the registration size rules
//! (`libobs/obs-module-register-copy.c.h`), as one registration of each
//! kind makes them: the copy length of a source cut inside the pointer
//! after `icon_type` (four 32-bit fields), the in-size check of a callback,
//! the size gate, and the transition rule. Each runs a few times per
//! registered type, at startup and when a module loads. Plain values: no
//! allocation.

use criterion::Criterion;
use obs_c_oracle::module_register_copy::{
    oracle_obs_module_register_copy_gate, oracle_obs_module_register_copy_ignores_size,
    oracle_obs_module_register_copy_whole_size, oracle_obs_module_register_copy_within,
};
use obs_core::ffi::module_register_copy::{
    obs_module_register_copy_gate, obs_module_register_copy_ignores_size,
    obs_module_register_copy_whole_size, obs_module_register_copy_within,
};
use obs_core::module_register_copy::SOURCE_TYPE_TRANSITION;

/// The 32-bit fields of `struct obs_source_info` on a 64-bit build.
const SOURCE_U32: [usize; 4] = [8, 12, 288, 368];
/// A cut inside the pointer after `icon_type`.
const CUT: usize = 299;
const SOURCE_SIZE: usize = 424;

pub fn benchmarks(criterion: &mut Criterion) {
    super::pair(
        criterion,
        "module_register_copy/whole_size",
        // SAFETY: a live array of four entries.
        || unsafe { obs_module_register_copy_whole_size(CUT, 8, SOURCE_U32.as_ptr(), SOURCE_U32.len()) },
        // SAFETY: as above.
        || unsafe {
            oracle_obs_module_register_copy_whole_size(CUT, 8, SOURCE_U32.as_ptr(), SOURCE_U32.len())
        },
    );
    super::pair(
        criterion,
        "module_register_copy/within",
        || obs_module_register_copy_within(224, 8, CUT),
        // SAFETY: plain values.
        || unsafe { oracle_obs_module_register_copy_within(224, 8, CUT) },
    );
    super::pair(
        criterion,
        "module_register_copy/gate",
        || obs_module_register_copy_gate(CUT, SOURCE_SIZE),
        // SAFETY: plain values.
        || unsafe { oracle_obs_module_register_copy_gate(CUT, SOURCE_SIZE) },
    );
    super::pair(
        criterion,
        "module_register_copy/ignores_size",
        || obs_module_register_copy_ignores_size(SOURCE_TYPE_TRANSITION),
        // SAFETY: plain value.
        || unsafe { oracle_obs_module_register_copy_ignores_size(SOURCE_TYPE_TRANSITION) },
    );
}
