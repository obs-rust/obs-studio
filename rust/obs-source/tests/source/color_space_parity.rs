//! Tier 3: the `obs_source_color_space_*` shims against the original C
//! (`obs-c-oracle`, `oracle/source_color_space.c`: the unmodified
//! `libobs/obs-source-color-space.c.h` with its functions renamed).
//!
//! The route takes the node by value and returns the C's enum, and the pick
//! takes a list and returns an `int`, so a run compares the two results as
//! integers: the route's value and the picked space.
//!
//! Runs:
//! - the node's layout, the route values and the type and flag constants
//!   against the C header;
//! - the route over every combination of the VIDEO and ASYNC flags, data,
//!   enabled, a filter parent and a filter target (64), with eight types
//!   (every `enum obs_source_type`, -1, 4 and the ends of `i32`: one filter,
//!   seven not, so all 128 rows of the characterization) and sixteen sets of
//!   other flags: 8,192 nodes; and a proptest over any `i32` type, any `u32`
//!   flags and any of the four bools;
//! - the pick over every list of 0 to 5 values from -1..=4 (the four
//!   `enum gs_color_space` values and one past each end) against every
//!   frame space from the same values: 9,331 lists x 6, 55,986 picks; and a proptest over
//!   lists of 0 to 8 arbitrary `i32` values (with the frame's space drawn
//!   from the list as often as anywhere), the lists passed by pointer to
//!   both sides;
//! - a count of 0 with a null pointer and with a pointer to values, on both
//!   sides.
//!
//! Intentional difference, checked on the shim only: a null list with a
//! count above 0 is read as empty (the C reads through the null pointer).

use std::ffi::c_int;
use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::source_color_space::{self as oracle, OracleColorSpaceNode};
use obs_source::color_space::{ASYNC, TYPE_FILTER, VIDEO};
use obs_source::ffi::source_color_space::{
    self as shim, OBS_SOURCE_COLOR_SPACE_ASK_PARENT, OBS_SOURCE_COLOR_SPACE_ASK_TARGET,
    OBS_SOURCE_COLOR_SPACE_ASYNC_PICK, OBS_SOURCE_COLOR_SPACE_CALLBACK, obs_source_color_space_ask,
    obs_source_color_space_node,
};
use proptest::prelude::*;

#[test]
fn layout_matches_the_c_header() {
    type N = obs_source_color_space_node;
    let rust = [
        size_of::<N>(),
        align_of::<N>(),
        offset_of!(N, source_type),
        offset_of!(N, output_flags),
        offset_of!(N, has_data),
        offset_of!(N, enabled),
        offset_of!(N, has_filter_parent),
        offset_of!(N, has_filter_target),
        size_of::<obs_source_color_space_ask>(),
        OBS_SOURCE_COLOR_SPACE_ASK_PARENT as usize,
        OBS_SOURCE_COLOR_SPACE_ASK_TARGET as usize,
        OBS_SOURCE_COLOR_SPACE_ASYNC_PICK as usize,
        OBS_SOURCE_COLOR_SPACE_CALLBACK as usize,
        usize::try_from(TYPE_FILTER).unwrap(),
        VIDEO as usize,
        ASYNC as usize,
        size_of::<c_int>(),
    ];
    for (which, (name, value)) in oracle::LAYOUT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a pure lookup.
        let c = unsafe { oracle::oracle_source_color_space_layout(which) };
        assert_eq!(value, c, "{name}");
    }
    // The oracle's mirror is the same layout.
    assert_eq!(size_of::<OracleColorSpaceNode>(), size_of::<N>());
    assert_eq!(align_of::<OracleColorSpaceNode>(), align_of::<N>());
}

// --------------------------------------------------------------------------
// One call on each side

#[derive(Debug, Clone, Copy)]
struct Fields {
    source_type: i32,
    output_flags: u32,
    has_data: bool,
    enabled: bool,
    has_filter_parent: bool,
    has_filter_target: bool,
}

fn c_route(f: Fields) -> u32 {
    let node = OracleColorSpaceNode {
        source_type: f.source_type,
        output_flags: f.output_flags,
        has_data: f.has_data,
        enabled: f.enabled,
        has_filter_parent: f.has_filter_parent,
        has_filter_target: f.has_filter_target,
    };
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_source_color_space_route(node) }
}

fn r_route(f: Fields) -> u32 {
    shim::obs_source_color_space_route(obs_source_color_space_node {
        source_type: f.source_type,
        output_flags: f.output_flags,
        has_data: f.has_data,
        enabled: f.enabled,
        has_filter_parent: f.has_filter_parent,
        has_filter_target: f.has_filter_target,
    })
}

#[track_caller]
fn check_route(f: Fields) -> u32 {
    let c = c_route(f);
    let r = r_route(f);
    assert_eq!(r, c, "route of {f:?}");
    c
}

fn c_pick(video_space: i32, preferred: &[i32]) -> i32 {
    let ptr = if preferred.is_empty() {
        std::ptr::null()
    } else {
        preferred.as_ptr()
    };
    // SAFETY: ptr holds preferred.len() ints, or is null with a count of 0.
    unsafe { oracle::oracle_obs_source_color_space_pick(video_space, preferred.len(), ptr) }
}

fn r_pick(video_space: i32, preferred: &[i32]) -> i32 {
    let ptr = if preferred.is_empty() {
        std::ptr::null()
    } else {
        preferred.as_ptr()
    };
    // SAFETY: as for c_pick.
    unsafe { shim::obs_source_color_space_pick(video_space, preferred.len(), ptr) }
}

#[track_caller]
fn check_pick(video_space: i32, preferred: &[i32]) -> i32 {
    let c = c_pick(video_space, preferred);
    let r = r_pick(video_space, preferred);
    assert_eq!(r, c, "pick of {video_space} from {preferred:?}");
    c
}

// --------------------------------------------------------------------------
// The route

const TYPES: [i32; 8] = [0, 1, 2, 3, -1, 4, i32::MIN, i32::MAX];

/// The other flags: none, each of `OBS_SOURCE_AUDIO` (bit 1), bit 3 and the
/// top bit alone and together, and every bit but VIDEO and ASYNC.
fn other_flag_sets() -> Vec<u32> {
    let bits = [1u32 << 1, 1 << 3, 1 << 10, 1 << 31];
    let mut sets: Vec<u32> = (0..16u32)
        .map(|mask| {
            bits.iter()
                .enumerate()
                .filter(|(i, _)| mask & (1 << i) != 0)
                .fold(0, |acc, (_, bit)| acc | bit)
        })
        .collect();
    // Replace the set of all four with every other bit.
    sets[15] = !(VIDEO | ASYNC);
    sets
}

/// Every combination of the VIDEO and ASYNC flags, data, enabled, a filter
/// parent and a filter target (64), with every type in `TYPES`: the filter
/// type gives the 64 filter rows, the seven others the 64 input rows seven
/// times over, so the 128 rows of the characterization are all here.
#[test]
fn route_every_row_type_and_flags() {
    let mut seen = [0usize; 4];
    let mut nodes = 0usize;
    for row in 0u32..64 {
        let bit = |n: u32| row & (1 << n) != 0;
        let mut flags = 0;
        if bit(5) {
            flags |= VIDEO;
        }
        if bit(4) {
            flags |= ASYNC;
        }
        for source_type in TYPES {
            for extra in other_flag_sets() {
                let f = Fields {
                    source_type,
                    output_flags: flags | extra,
                    has_data: bit(3),
                    enabled: bit(2),
                    has_filter_parent: bit(1),
                    has_filter_target: bit(0),
                };
                let route = check_route(f);
                seen[usize::try_from(route).unwrap()] += 1;
                nodes += 1;
            }
        }
    }
    assert_eq!(nodes, 64 * 8 * 16);
    assert!(seen.iter().all(|&n| n > 100), "{seen:?}");
}

// --------------------------------------------------------------------------
// The pick

const SMALL: [i32; 6] = [-1, 0, 1, 2, 3, 4];

#[test]
fn pick_every_short_list() {
    let mut lists: Vec<Vec<i32>> = vec![vec![]];
    let mut all = lists.clone();
    for _ in 0..5 {
        lists = lists
            .iter()
            .flat_map(|l| {
                SMALL.iter().map(move |&v| {
                    let mut next = l.clone();
                    next.push(v);
                    next
                })
            })
            .collect();
        all.extend(lists.iter().cloned());
    }
    assert_eq!(all.len(), 1 + 6 + 36 + 216 + 1296 + 7776);
    let (mut matched, mut unmatched) = (0usize, 0usize);
    for list in &all {
        for video_space in SMALL {
            check_pick(video_space, list);
            if list.contains(&video_space) {
                matched += 1;
            } else {
                unmatched += 1;
            }
        }
    }
    assert!(matched > 1000 && unmatched > 1000);
}

#[test]
fn pick_count_zero_reads_nothing() {
    let values = [2, 3];
    for video_space in [i32::MIN, -1, 0, 2, i32::MAX] {
        // SAFETY: a count of 0 reads nothing.
        let c_null =
            unsafe { oracle::oracle_obs_source_color_space_pick(video_space, 0, std::ptr::null()) };
        // SAFETY: as above.
        let r_null = unsafe { shim::obs_source_color_space_pick(video_space, 0, std::ptr::null()) };
        // SAFETY: as above.
        let c_some =
            unsafe { oracle::oracle_obs_source_color_space_pick(video_space, 0, values.as_ptr()) };
        // SAFETY: as above.
        let r_some = unsafe { shim::obs_source_color_space_pick(video_space, 0, values.as_ptr()) };
        assert_eq!([c_null, r_null, c_some, r_some], [video_space; 4]);
    }
}

/// Intentional difference: the C reads through a null list with a count
/// above 0; the shim reads it as empty.
#[test]
fn pick_null_list_is_empty_in_the_shim() {
    for count in [1, 3, usize::MAX] {
        // SAFETY: the shim checks for null before it reads.
        let got = unsafe { shim::obs_source_color_space_pick(2, count, std::ptr::null()) };
        assert_eq!(got, 2);
    }
}

// --------------------------------------------------------------------------
// Proptests

fn any_space() -> impl Strategy<Value = i32> {
    prop_oneof![any::<i32>(), -1i32..5]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn route_any_node(
        source_type in prop_oneof![any::<i32>(), -2i32..6],
        output_flags in any::<u32>(),
        has_data in any::<bool>(),
        enabled in any::<bool>(),
        has_filter_parent in any::<bool>(),
        has_filter_target in any::<bool>(),
    ) {
        check_route(Fields {
            source_type,
            output_flags,
            has_data,
            enabled,
            has_filter_parent,
            has_filter_target,
        });
    }

    /// Lists of 0 to 8 arbitrary values. Half the time the frame's space is
    /// drawn from the list, so matches come first, last and in the middle.
    #[test]
    fn pick_any_list(
        preferred in proptest::collection::vec(any_space(), 0..=8),
        free_space in any_space(),
        from_list in any::<bool>(),
        at in any::<proptest::sample::Index>(),
    ) {
        let video_space = if from_list && !preferred.is_empty() {
            preferred[at.index(preferred.len())]
        } else {
            free_space
        };
        check_pick(video_space, &preferred);
    }
}
