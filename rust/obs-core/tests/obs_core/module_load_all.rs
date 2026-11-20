//! Tier 1: counterparts of the assertions in the unchanged
//! test_module_load_all.c, through the safe core. The caller-owned failure
//! info checks (bytes, pointer array, count, strings) need the C ABI and live
//! in module_load_all_parity.rs; the safe core takes none of it.
use super::alloc_count::allocs_during;
use obs_core::module_load_all::{load_all_modules, load_all_modules2};

#[test]
fn deprecated_loaders_do_nothing_repeatedly() {
    // Three rounds, as in the C characterization: no allocation, no state.
    for _ in 0..3 {
        assert_eq!(allocs_during(load_all_modules), 0);
        assert_eq!(allocs_during(load_all_modules2), 0);
        assert_eq!(allocs_during(load_all_modules2), 0);
    }
}
