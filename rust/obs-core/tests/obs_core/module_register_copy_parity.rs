//! Tier 3: `obs_core::module_register_copy` against the original
//! `libobs/obs-module-register-copy.c.h`, compiled as an oracle.
//!
//! Every shim is compared with its `oracle_*` twin: every size from 0 to
//! `sizeof() + 8` for each structure's 32-bit field list (as `obs-module.c`
//! lists them, from the oracle) at both pointer sizes, and for 32-bit
//! layouts; every field the registrations read at every such size; the gate
//! at every size around each structure's; the transition rule for every
//! small type and the edges of `u32`; and proptests over any size, pointer
//! size, field list, offset and type.
//!
//! Intentional differences, excluded from the comparisons and nothing more
//! (both unreachable from libobs, which passes `offsetof()` constants and
//! `sizeof(void *)`):
//! - a field whose end would pass `usize::MAX`: C wraps the sum, here it is
//!   never within the size (`field_end_past_usize_max_is_not_within` in the
//!   Tier 1 tests);
//! - a pointer size of 0: C divides by zero, here the size is not rounded
//!   (`zero_pointer_size_does_not_panic`).

use obs_c_oracle::module_register_copy::{
    CONSTANT_NAMES, KIND_ENCODER, KIND_OUTPUT, KIND_SERVICE, KIND_SOURCE,
    oracle_module_register_copy_constant, oracle_obs_module_register_copy_gate,
    oracle_obs_module_register_copy_ignores_size, oracle_obs_module_register_copy_whole_size,
    oracle_obs_module_register_copy_within, read_fields, struct_size, u32_fields,
};
use obs_core::ffi::module_register_copy::{
    COPY, OUTSIDE_LOAD, TOO_LARGE, obs_module_register_copy_gate,
    obs_module_register_copy_ignores_size, obs_module_register_copy_whole_size,
    obs_module_register_copy_within,
};
use obs_core::module_register_copy::{SOURCE_TYPE_TRANSITION, U32_SIZE};
use proptest::prelude::*;

const KINDS: [u32; 4] = [KIND_SOURCE, KIND_OUTPUT, KIND_ENCODER, KIND_SERVICE];

fn shim_whole(size: usize, ptr: usize, fields: &[usize]) -> usize {
    // SAFETY: `fields` is a live slice of `fields.len()` entries.
    unsafe { obs_module_register_copy_whole_size(size, ptr, fields.as_ptr(), fields.len()) }
}

fn oracle_whole(size: usize, ptr: usize, fields: &[usize]) -> usize {
    // SAFETY: as above.
    unsafe { oracle_obs_module_register_copy_whole_size(size, ptr, fields.as_ptr(), fields.len()) }
}

fn oracle_within(offset: usize, field_size: usize, size: usize) -> bool {
    // SAFETY: plain values.
    unsafe { oracle_obs_module_register_copy_within(offset, field_size, size) }
}

fn oracle_gate(size: usize, supported: usize) -> u32 {
    // SAFETY: plain values.
    unsafe { oracle_obs_module_register_copy_gate(size, supported) }
}

fn oracle_ignores(source_type: u32) -> bool {
    // SAFETY: plain value.
    unsafe { oracle_obs_module_register_copy_ignores_size(source_type) }
}

/// The C's constants match the shim's and the core's.
#[test]
fn constants_match() {
    let expected = [
        COPY,
        OUTSIDE_LOAD,
        TOO_LARGE,
        SOURCE_TYPE_TRANSITION,
        SOURCE_TYPE_TRANSITION,
        4,
        u32::try_from(U32_SIZE).unwrap(),
        u32::try_from(size_of::<*const ()>()).unwrap(),
    ];
    for (i, (&want, name)) in expected.iter().zip(CONSTANT_NAMES).enumerate() {
        // SAFETY: plain value.
        let got = unsafe { oracle_module_register_copy_constant(i) };
        assert_eq!(got, want, "{name}");
    }
}

/// The oracle reports the layouts `obs-module.c` copies: a 32-bit field
/// list for the source, output and encoder, none for the service.
#[test]
fn layouts_are_present() {
    assert_eq!(u32_fields(KIND_SOURCE).len(), 4);
    assert_eq!(u32_fields(KIND_OUTPUT).len(), 1);
    assert_eq!(u32_fields(KIND_ENCODER).len(), 2);
    assert!(u32_fields(KIND_SERVICE).is_empty());
    for kind in KINDS {
        assert!(struct_size(kind) > 0);
        assert!(!read_fields(kind).is_empty());
    }
}

/// Every size up to `sizeof() + 8` of each structure, with its field list
/// at both pointer sizes, and with no list.
#[test]
fn whole_size_every_size_of_each_structure() {
    for kind in KINDS {
        let fields = u32_fields(kind);
        for ptr in [4, 8] {
            for size in 0..=struct_size(kind) + 8 {
                for list in [&fields[..], &[][..]] {
                    assert_eq!(
                        shim_whole(size, ptr, list),
                        oracle_whole(size, ptr, list),
                        "kind {kind} ptr {ptr} size {size} list {list:?}"
                    );
                }
            }
        }
    }
}

/// The same 32-bit fields on 32-bit layouts (each field four bytes, in
/// order: source type and output_flags at 4 and 8, output flags at 4,
/// encoder type at 4), and lists in reverse order and with overlaps.
#[test]
fn whole_size_32bit_and_odd_lists() {
    let lists: [&[usize]; 7] = [
        &[4, 8],
        &[4],
        &[4, 68],
        &[8, 4],
        &[16, 18],
        &[18, 16],
        &[0, 1, 2, 3, 4, 5, 6, 7],
    ];
    for list in lists {
        for ptr in [1, 2, 4, 8, 16] {
            for size in 0..=96 {
                assert_eq!(
                    shim_whole(size, ptr, list),
                    oracle_whole(size, ptr, list),
                    "ptr {ptr} size {size} list {list:?}"
                );
            }
        }
    }
}

/// Every field the registrations read, at every size up to `sizeof() + 8`.
#[test]
fn within_every_read_field() {
    for kind in KINDS {
        for (offset, field_size) in read_fields(kind) {
            for size in 0..=struct_size(kind) + 8 {
                assert_eq!(
                    obs_module_register_copy_within(offset, field_size, size),
                    oracle_within(offset, field_size, size),
                    "kind {kind} field {offset}+{field_size} size {size}"
                );
            }
        }
    }
}

/// The gate at every size around each structure's, and at the edges.
#[test]
fn gate_every_size() {
    for kind in KINDS {
        let supported = struct_size(kind);
        for size in (0..=supported + 8).chain([usize::MAX - 1, usize::MAX]) {
            assert_eq!(
                obs_module_register_copy_gate(size, supported),
                oracle_gate(size, supported),
                "kind {kind} size {size}"
            );
        }
    }
    for (size, supported) in [(0, 0), (1, 0), (0, usize::MAX), (usize::MAX, usize::MAX)] {
        assert_eq!(
            obs_module_register_copy_gate(size, supported),
            oracle_gate(size, supported)
        );
    }
}

/// The transition rule for every small type and the edges of `u32`.
#[test]
fn ignores_size_small_and_edges() {
    for source_type in (0..=64).chain([u32::MAX - 1, u32::MAX, 1 << 31]) {
        assert_eq!(
            obs_module_register_copy_ignores_size(source_type),
            oracle_ignores(source_type),
            "type {source_type}"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    /// Any size, a pointer size of 1 to 64 and a list of fields that end
    /// before `usize::MAX`.
    #[test]
    fn whole_size_any(
        size in prop_oneof![0usize..1 << 12, any::<usize>()],
        ptr in 1usize..=64,
        fields in prop::collection::vec(
            prop_oneof![0usize..1 << 12, 0usize..usize::MAX - U32_SIZE],
            0..8,
        ),
    ) {
        prop_assert_eq!(shim_whole(size, ptr, &fields), oracle_whole(size, ptr, &fields));
    }

    /// Any offset and field size whose end fits, and any size.
    #[test]
    fn within_any(
        offset in prop_oneof![0usize..1 << 12, 0usize..usize::MAX / 2],
        field_size in prop_oneof![0usize..16, 0usize..usize::MAX / 2],
        size in prop_oneof![0usize..1 << 12, any::<usize>()],
    ) {
        prop_assert_eq!(
            obs_module_register_copy_within(offset, field_size, size),
            oracle_within(offset, field_size, size)
        );
    }

    /// Any size and limit.
    #[test]
    fn gate_any(
        size in prop_oneof![0usize..1 << 10, any::<usize>()],
        supported in prop_oneof![0usize..1 << 10, any::<usize>()],
    ) {
        prop_assert_eq!(obs_module_register_copy_gate(size, supported), oracle_gate(size, supported));
    }

    /// Any type.
    #[test]
    fn ignores_size_any(source_type in any::<u32>()) {
        prop_assert_eq!(
            obs_module_register_copy_ignores_size(source_type),
            oracle_ignores(source_type)
        );
    }
}
