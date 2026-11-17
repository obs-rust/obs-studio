//! Exact parity against the original C. Excludes signed count overflow and
//! releasing the final owner of a stack allocation; allocator-backed frees
//! have a separate probe. No intentional differences.
use core::ffi::{c_long, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::output_weak_lifetime::{
    WeakOutput, obs_output_get_weak_output, obs_weak_output_addref, obs_weak_output_get_output,
    obs_weak_output_references_output, obs_weak_output_release,
};
use obs_output::ffi::service_weak_acquire::WeakRef;
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_output_weak_lifetime_layout(layout: *mut usize);
    fn oracle_output_weak_lifetime_probe(
        operation: u32,
        refs: c_long,
        weak_refs: c_long,
        flags: u32,
        addref: unsafe extern "C" fn(*mut WeakOutput),
        release: unsafe extern "C" fn(*mut WeakOutput),
        get_weak: unsafe extern "C" fn(*mut c_void) -> *mut WeakOutput,
        promote: unsafe extern "C" fn(*mut WeakOutput) -> *mut c_void,
        references: unsafe extern "C" fn(*mut WeakOutput, *mut c_void) -> bool,
    ) -> bool;
    fn oracle_output_weak_lifetime_final_release(release: unsafe extern "C" fn(*mut WeakOutput));
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 8];
    // SAFETY: the C probe writes exactly eight measured size/alignment/offsets.
    unsafe { oracle_output_weak_lifetime_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<WeakRef>(),
            align_of::<WeakRef>(),
            offset_of!(WeakRef, refs),
            offset_of!(WeakRef, weak_refs),
            size_of::<WeakOutput>(),
            align_of::<WeakOutput>(),
            offset_of!(WeakOutput, counts),
            offset_of!(WeakOutput, output),
        ]
    );
}

fn check(operation: u32, refs: c_long, weak: c_long, flags: u32) {
    // SAFETY: C retains real output/control layouts and calls each callback
    // with valid owned handles or NULL. Counts avoid overflow and stack frees.
    assert!(
        unsafe {
            oracle_output_weak_lifetime_probe(
                operation,
                refs,
                weak,
                flags,
                obs_weak_output_addref,
                obs_weak_output_release,
                obs_output_get_weak_output,
                obs_weak_output_get_output,
                obs_weak_output_references_output,
            )
        },
        "operation={operation} refs={refs} weak={weak} flags={flags}"
    );
}

#[test]
fn characterized_edges_and_final_free() {
    for operation in 0..5 {
        for refs in [c_long::MIN, -3, -2, -1, 0, 1, 7, 1023, c_long::MAX - 1] {
            for weak in [1, 2, 3, c_long::MAX - 1] {
                for flags in 0..32 {
                    check(operation, refs, weak, flags);
                }
            }
        }
    }
    // SAFETY: C transfers two allocator-backed controls to the final releases.
    unsafe { oracle_output_weak_lifetime_final_release(obs_weak_output_release) };
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn helpers_match_c(operation in 0u32..5, refs in c_long::MIN..c_long::MAX,
                       weak in 1 as c_long..c_long::MAX, flags in 0u32..32) {
        check(operation, refs, weak, flags);
    }
}
