//! Tier 2 layout of the structs the shutdown detachment passes across the C
//! boundary: `struct obs_shutdown_detach_calls` (`obs-shutdown-detach.h`),
//! which the bridge hands the Rust shim, and the oracle's
//! `struct oracle_shutdown_detach_hooks`, which the parity test hands the C.
//! Each size, alignment and member offset is checked against the values the
//! oracle measures with the C compiler.

use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::shutdown_detach::{
    Hooks, oracle_shutdown_detach_calls_layout, oracle_shutdown_detach_hooks_layout,
};
use obs_core::ffi::shutdown_detach::Calls;

#[test]
fn calls_match_c() {
    let mut c = [0usize; 12];
    // SAFETY: the oracle writes exactly twelve values.
    unsafe { oracle_shutdown_detach_calls_layout(&mut c) };
    let rust = [
        size_of::<Calls>(),
        align_of::<Calls>(),
        offset_of!(Calls, lock),
        offset_of!(Calls, unlock),
        offset_of!(Calls, each_source),
        offset_of!(Calls, get_ref),
        offset_of!(Calls, scene_from_source),
        offset_of!(Calls, group_from_source),
        offset_of!(Calls, enum_items),
        offset_of!(Calls, item_remove),
        offset_of!(Calls, release),
        offset_of!(Calls, wait_destruction),
    ];
    assert_eq!(rust, c);
}

#[test]
fn hooks_match_c() {
    let mut c = [0usize; 12];
    // SAFETY: the oracle writes exactly twelve values.
    unsafe { oracle_shutdown_detach_hooks_layout(&mut c) };
    let rust = [
        size_of::<Hooks>(),
        align_of::<Hooks>(),
        offset_of!(Hooks, model),
        offset_of!(Hooks, lock),
        offset_of!(Hooks, unlock),
        offset_of!(Hooks, get_ref),
        offset_of!(Hooks, scene_from_source),
        offset_of!(Hooks, group_from_source),
        offset_of!(Hooks, enum_items),
        offset_of!(Hooks, item_remove),
        offset_of!(Hooks, release),
        offset_of!(Hooks, wait),
    ];
    assert_eq!(rust, c);
}
