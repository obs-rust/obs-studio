//! Tier 3: original C and Rust over the real registry adapter and real
//! obs_data objects: which callback runs, with which type data, on which
//! object, and what the caller gets back. No excluded inputs except a NULL
//! ID with types registered, which the C lookup passes to strcmp.
use core::ffi::{c_char, c_void};
use obs_source::ffi::source_defaults::{obs_get_source_defaults, obs_source_settings};

type Proc = unsafe extern "C" fn(*const c_char) -> *mut c_void;

unsafe extern "C" {
    fn oracle_source_defaults_check(rust_defaults: Proc, rust_settings: Proc, flags: u32) -> bool;
}

#[test]
fn callback_selection_and_ownership_match_c() {
    for flags in 0..16 {
        // SAFETY: the oracle installs a per-thread registry of live C infos,
        // calls both sides with static IDs and releases every object it gets.
        let equal = unsafe {
            oracle_source_defaults_check(obs_get_source_defaults, obs_source_settings, flags)
        };
        assert!(equal, "flags={flags}");
    }
}
