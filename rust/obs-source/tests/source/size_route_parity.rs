//! Tier 3: the `obs_source_size_*` shims against the original C
//! (`obs-c-oracle`: `oracle/source_size.c`, the unmodified
//! `libobs/obs-source-size.c.h` with its functions renamed, and
//! `oracle/source_size_long32.c`, the same file built with a 32-bit `long`
//! as MSVC builds it).
//!
//! The route takes the node by value and the entry two values, and both
//! return the C's enum, so a run compares the two results as integers.
//!
//! Runs:
//! - the node's layout, the route and entry values and the type constants
//!   against the C header, and the 32-bit node's size and rotation offset;
//! - the route over every combination of data, a callback, a filter parent,
//!   enabled and async active (32), with eight types (every
//!   `enum obs_source_type`, -1, 4 and the ends of `i32`) and 46 rotations
//!   (every multiple of 45 from -720 to 720, 1, -1, 179, 181, 359 and the
//!   ends of `c_long` and of `i32` with their nearest multiples of 180):
//!   11,776 nodes, against the C's `long` and, for the rotations a 32-bit
//!   `long` holds, against the 32-bit build;
//! - the swap of an active async input for every rotation from -100,000 to
//!   100,000, on both builds;
//! - the entry for every type above and both filter flags, on both builds;
//! - proptests (20,000 cases each): any node with any `c_long` rotation
//!   against the C's `long`, and any node with any `i32` rotation against
//!   the 32-bit build.
//!
//! No intentional differences.

use std::ffi::{c_int, c_long};
use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::source_size::{self as oracle, OracleSizeNode, OracleSizeNode32};
use obs_source::ffi::source_size::{
    self as shim, OBS_SOURCE_SIZE_ASK_TARGET, OBS_SOURCE_SIZE_ASYNC, OBS_SOURCE_SIZE_ASYNC_SWAPPED,
    OBS_SOURCE_SIZE_CALLBACK, OBS_SOURCE_SIZE_ENTRY_FIRST_FILTER, OBS_SOURCE_SIZE_ENTRY_SELF,
    OBS_SOURCE_SIZE_TRANSITION, OBS_SOURCE_SIZE_ZERO, obs_source_size_ask, obs_source_size_node,
    obs_source_size_start,
};
use obs_source::size_route::{TYPE_FILTER, TYPE_TRANSITION};
use proptest::prelude::*;

#[test]
fn layout_matches_the_c_header() {
    type N = obs_source_size_node;
    let rust = [
        size_of::<N>(),
        align_of::<N>(),
        offset_of!(N, source_type),
        offset_of!(N, has_data),
        offset_of!(N, has_callback),
        offset_of!(N, has_filter_parent),
        offset_of!(N, enabled),
        offset_of!(N, async_active),
        offset_of!(N, async_rotation),
        size_of::<obs_source_size_ask>(),
        OBS_SOURCE_SIZE_TRANSITION as usize,
        OBS_SOURCE_SIZE_CALLBACK as usize,
        OBS_SOURCE_SIZE_ASK_TARGET as usize,
        OBS_SOURCE_SIZE_ASYNC as usize,
        OBS_SOURCE_SIZE_ASYNC_SWAPPED as usize,
        OBS_SOURCE_SIZE_ZERO as usize,
        size_of::<obs_source_size_start>(),
        OBS_SOURCE_SIZE_ENTRY_SELF as usize,
        OBS_SOURCE_SIZE_ENTRY_FIRST_FILTER as usize,
        usize::try_from(TYPE_FILTER).unwrap(),
        usize::try_from(TYPE_TRANSITION).unwrap(),
        size_of::<c_long>(),
        size_of::<c_int>(),
    ];
    for (which, (name, value)) in oracle::LAYOUT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a pure lookup.
        let c = unsafe { oracle::oracle_source_size_layout(which) };
        assert_eq!(value, c, "{name}");
    }
    // The oracle's mirrors are the same layouts.
    assert_eq!(size_of::<OracleSizeNode>(), size_of::<N>());
    assert_eq!(align_of::<OracleSizeNode>(), align_of::<N>());
    // SAFETY: pure lookups.
    let long32 = unsafe {
        [
            oracle::oracle32_source_size_layout(0),
            oracle::oracle32_source_size_layout(1),
            oracle::oracle32_source_size_layout(2),
        ]
    };
    assert_eq!(
        long32,
        [
            size_of::<OracleSizeNode32>(),
            offset_of!(OracleSizeNode32, async_rotation),
            4
        ]
    );
}

// --------------------------------------------------------------------------
// One call on each side

#[derive(Debug, Clone, Copy)]
struct Fields {
    source_type: i32,
    has_data: bool,
    has_callback: bool,
    has_filter_parent: bool,
    enabled: bool,
    async_active: bool,
}

fn c_route(f: Fields, rotation: c_long) -> u32 {
    let node = OracleSizeNode {
        source_type: f.source_type,
        has_data: f.has_data,
        has_callback: f.has_callback,
        has_filter_parent: f.has_filter_parent,
        enabled: f.enabled,
        async_active: f.async_active,
        async_rotation: rotation,
    };
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_source_size_route(node) }
}

fn c32_route(f: Fields, rotation: i32) -> u32 {
    let node = OracleSizeNode32 {
        source_type: f.source_type,
        has_data: f.has_data,
        has_callback: f.has_callback,
        has_filter_parent: f.has_filter_parent,
        enabled: f.enabled,
        async_active: f.async_active,
        async_rotation: rotation,
    };
    // SAFETY: plain values.
    unsafe { oracle::oracle32_obs_source_size_route(node) }
}

fn r_route(f: Fields, rotation: c_long) -> u32 {
    shim::obs_source_size_route(obs_source_size_node {
        source_type: f.source_type,
        has_data: f.has_data,
        has_callback: f.has_callback,
        has_filter_parent: f.has_filter_parent,
        enabled: f.enabled,
        async_active: f.async_active,
        async_rotation: rotation,
    })
}

// `c_long` is already `i32` on Windows, so the 32-bit check's conversion
// there is a no-op that always matches.
#[allow(clippy::useless_conversion, irrefutable_let_patterns)]
#[track_caller]
fn check_route(f: Fields, rotation: c_long) -> u32 {
    let c = c_route(f, rotation);
    let r = r_route(f, rotation);
    assert_eq!(r, c, "route of {f:?} at {rotation}");
    // A rotation a 32-bit long holds routes the same in the 32-bit build.
    if let Ok(rotation32) = i32::try_from(rotation) {
        assert_eq!(
            c32_route(f, rotation32),
            c,
            "32-bit route of {f:?} at {rotation}"
        );
    }
    c
}

#[track_caller]
fn check_route32(f: Fields, rotation: i32) -> u32 {
    let c = c32_route(f, rotation);
    let r = r_route(f, c_long::from(rotation));
    assert_eq!(
        r, c,
        "route of {f:?} at {rotation} against the 32-bit build"
    );
    c
}

#[track_caller]
fn check_entry(source_type: i32, has_filters: bool) -> u32 {
    // SAFETY: plain values.
    let c = unsafe { oracle::oracle_obs_source_size_entry(source_type, has_filters) };
    // SAFETY: plain values.
    let c32 = unsafe { oracle::oracle32_obs_source_size_entry(source_type, has_filters) };
    let r = shim::obs_source_size_entry(source_type, has_filters);
    assert_eq!(r, c, "entry of {source_type}, {has_filters}");
    assert_eq!(c32, c, "32-bit entry of {source_type}, {has_filters}");
    c
}

// --------------------------------------------------------------------------
// Exhaustive

const TYPES: [i32; 8] = [0, 1, 2, 3, -1, 4, i32::MIN, i32::MAX];

fn rotations() -> Vec<c_long> {
    let mut list: Vec<c_long> = (-16..=16).map(|k| k * 45).collect();
    list.extend([1, -1, 179, 181, 359]);
    let ends = [c_long::MIN, c_long::MAX];
    list.extend(ends);
    // The multiples of 180 nearest the ends of c_long.
    list.push(c_long::MIN - c_long::MIN % 180);
    list.push(c_long::MAX - c_long::MAX % 180);
    // The ends of i32 and their nearest multiples of 180 (the same as above
    // where long is 32 bits).
    list.extend([c_long::from(i32::MIN), c_long::from(i32::MAX)]);
    list.push(c_long::from(i32::MIN - i32::MIN % 180));
    list.push(c_long::from(i32::MAX - i32::MAX % 180));
    list
}

fn fields(source_type: i32, bits: u32) -> Fields {
    let bit = |n: u32| bits & (1 << n) != 0;
    Fields {
        source_type,
        has_data: bit(4),
        has_callback: bit(3),
        has_filter_parent: bit(2),
        enabled: bit(1),
        async_active: bit(0),
    }
}

#[test]
fn route_every_flag_type_and_rotation() {
    let rotations = rotations();
    assert_eq!(rotations.len(), 46);
    let mut seen = [0usize; 6];
    let mut nodes = 0usize;
    for bits in 0..32 {
        for source_type in TYPES {
            for &rotation in &rotations {
                let route = check_route(fields(source_type, bits), rotation);
                seen[usize::try_from(route).unwrap()] += 1;
                nodes += 1;
            }
        }
    }
    assert_eq!(nodes, 32 * 8 * 46);
    assert!(seen.iter().all(|&n| n > 100), "{seen:?}");
}

#[test]
fn swap_every_rotation_near_zero() {
    let async_input = fields(0, 0b10011);
    let mut swapped = 0usize;
    for rotation in -100_000..=100_000 {
        let route = check_route(async_input, rotation);
        if route == OBS_SOURCE_SIZE_ASYNC_SWAPPED {
            swapped += 1;
        } else {
            assert_eq!(route, OBS_SOURCE_SIZE_ASYNC, "{rotation}");
        }
    }
    // 1,111 multiples of 180 in -100,000..=100,000.
    assert_eq!(swapped, 200_001 - 1_111);
}

#[test]
fn entry_every_type() {
    for source_type in TYPES {
        let with = check_entry(source_type, true);
        let without = check_entry(source_type, false);
        assert_eq!(without, OBS_SOURCE_SIZE_ENTRY_SELF);
        let expected = if source_type == TYPE_FILTER {
            OBS_SOURCE_SIZE_ENTRY_SELF
        } else {
            OBS_SOURCE_SIZE_ENTRY_FIRST_FILTER
        };
        assert_eq!(with, expected, "{source_type}");
    }
}

// --------------------------------------------------------------------------
// Proptests

fn any_fields() -> impl Strategy<Value = Fields> {
    (
        prop_oneof![any::<i32>(), -2i32..6],
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(|(source_type, d, c, p, e, a)| Fields {
            source_type,
            has_data: d,
            has_callback: c,
            has_filter_parent: p,
            enabled: e,
            async_active: a,
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    /// Any node, any c_long rotation (a fifth of them a multiple of 180).
    #[test]
    fn route_any_node(
        f in any_fields(),
        rotation in any::<c_long>(),
        multiple in any::<bool>(),
        pick in 0u8..5,
    ) {
        let rotation = if multiple && pick == 0 {
            rotation - rotation % 180
        } else {
            rotation
        };
        check_route(f, rotation);
    }

    /// Any node, any i32 rotation, against the 32-bit build.
    #[test]
    fn route_any_node_long32(
        f in any_fields(),
        rotation in any::<i32>(),
        multiple in any::<bool>(),
        pick in 0u8..5,
    ) {
        let rotation = if multiple && pick == 0 {
            rotation - rotation % 180
        } else {
            rotation
        };
        check_route32(f, rotation);
    }
}
