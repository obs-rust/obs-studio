//! Tier 3: the `obs_source_ptm_gate_*` shims and the safe core against the
//! original C (`obs-c-oracle`, `oracle/source_ptm_gate.c`: the unmodified
//! `libobs/obs-source-ptm-gate.c.h` with its functions renamed).
//!
//! The four functions take and return `uint64_t` and `bool` by value, so a
//! run compares the stop time as an integer and each `bool` as the byte the
//! C returns (it must be 0 or 1, and the same on both sides). Each input is
//! given to the oracle, to the shim (the `extern "C"` function the hybrid
//! links) and to the core.
//!
//! Runs:
//! - `OBS_SOURCE_PTM_GATE_NS_PER_MS` against the C header;
//! - the two gates over all four inputs each;
//! - the stop time and the hold over every pair of 46 special clocks, delays
//!   and stop times (0, 1 ms, 1 s, the slack below `u64::MAX` the largest
//!   unwrapped product leaves, that product's delay and value, `i64::MAX`,
//!   `u64::MAX / 2`, the end of the range, each +-2): 2,116 pairs each; and
//!   proptests over full-range `u64` clocks and delays, and near the edges;
//! - `source_muted()` of `obs-source.c`, copied onto a stand-in source and
//!   run once over the oracle's functions and once over the shims: over
//!   every combination of its six flags (64), with stop times before, at and
//!   after the clock and saturated, three pairs of delays and three clocks
//!   (14,400 runs), and a proptest over full-range delays, stop times and
//!   clocks with any flags. The result and both stored stop times must
//!   match.
//!
//! There is no intentional difference: the shims touch no memory.

use obs_c_oracle::source_ptm_gate as oracle;
use obs_source::ffi::source_ptm_gate as shim;
use obs_source::ptm_gate::{self as core, NS_PER_MS};
use proptest::prelude::*;

const MS: u64 = NS_PER_MS;
const SEC: u64 = 1000 * MS;
const MAX_DELAY: u64 = u64::MAX / MS;
const MAX_DELAY_SLACK: u64 = u64::MAX - MAX_DELAY * MS;
const T: u64 = 1000 * SEC;

#[track_caller]
fn c_bool(byte: u8, what: &str) -> bool {
    assert!(byte <= 1, "{what}: the C returned the byte {byte}");
    byte == 1
}

fn oracle_stop(os_time: u64, delay: u64) -> u64 {
    // SAFETY: by-value integers in and out; the C touches no memory.
    unsafe { oracle::oracle_obs_source_ptm_gate_stop_time(os_time, delay) }
}

#[track_caller]
fn oracle_held(os_time: u64, stop: u64) -> bool {
    // SAFETY: as above.
    c_bool(
        unsafe { oracle::oracle_obs_source_ptm_gate_held(os_time, stop) },
        "held",
    )
}

#[track_caller]
fn oracle_mutes(enabled: bool, active: bool) -> bool {
    // SAFETY: as above.
    c_bool(
        unsafe { oracle::oracle_obs_source_ptm_gate_mutes(enabled, active) },
        "mutes",
    )
}

#[track_caller]
fn oracle_talk_mutes(enabled: bool, active: bool) -> bool {
    // SAFETY: as above.
    c_bool(
        unsafe { oracle::oracle_obs_source_ptm_gate_talk_mutes(enabled, active) },
        "talk_mutes",
    )
}

#[track_caller]
fn check_stop(os_time: u64, delay: u64) {
    let c = oracle_stop(os_time, delay);
    assert_eq!(
        shim::obs_source_ptm_gate_stop_time(os_time, delay),
        c,
        "shim stop_time({os_time}, {delay})"
    );
    assert_eq!(
        core::stop_time(os_time, delay),
        c,
        "core stop_time({os_time}, {delay})"
    );
}

#[track_caller]
fn check_held(os_time: u64, stop: u64) {
    let c = oracle_held(os_time, stop);
    assert_eq!(
        shim::obs_source_ptm_gate_held(os_time, stop),
        c,
        "shim held({os_time}, {stop})"
    );
    assert_eq!(core::held(os_time, stop), c, "core held({os_time}, {stop})");
}

/// The 46 special values: 0, 1 ms, 1 s, a clock, the slack, the largest
/// unwrapped delay and its product, `u64::MAX / 2`, `i64::MAX`, the end of
/// the range less 1 ms and less the slack, and `u64::MAX`, each +-2
/// (wrapping), without repeats.
fn special_values() -> Vec<u64> {
    let centres = [
        0,
        MS,
        SEC,
        T,
        MAX_DELAY_SLACK,
        MAX_DELAY,
        MAX_DELAY * MS,
        u64::MAX / 2,
        i64::MAX.cast_unsigned(),
        u64::MAX - MS,
        u64::MAX - MAX_DELAY_SLACK,
        u64::MAX,
    ];
    let mut values = Vec::new();
    for c in centres {
        for d in 0..=4_u64 {
            values.push(c.wrapping_add(d).wrapping_sub(2));
        }
    }
    values.sort_unstable();
    values.dedup();
    values
}

#[test]
fn ns_per_ms_matches_the_c_header() {
    // SAFETY: returns a constant.
    assert_eq!(
        unsafe { oracle::oracle_source_ptm_gate_ns_per_ms() },
        NS_PER_MS
    );
    assert_eq!(special_values().len(), 46);
}

#[test]
fn gates_every_input() {
    for enabled in [false, true] {
        for active in [false, true] {
            let c = oracle_mutes(enabled, active);
            assert_eq!(shim::obs_source_ptm_gate_mutes(enabled, active), c);
            assert_eq!(core::mutes(enabled, active), c);
            let c = oracle_talk_mutes(enabled, active);
            assert_eq!(shim::obs_source_ptm_gate_talk_mutes(enabled, active), c);
            assert_eq!(core::talk_mutes(enabled, active), c);
        }
    }
}

#[test]
fn stop_time_every_special_pair() {
    let values = special_values();
    for &os_time in &values {
        for &delay in &values {
            check_stop(os_time, delay);
        }
    }
}

#[test]
fn held_every_special_pair() {
    let values = special_values();
    for &os_time in &values {
        for &stop in &values {
            check_held(os_time, stop);
        }
    }
}

/// The fields of `struct obs_source` `source_muted()` reads and writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct GateSource {
    enabled: bool,
    user_muted: bool,
    push_to_mute_enabled: bool,
    user_push_to_mute_pressed: bool,
    push_to_talk_enabled: bool,
    user_push_to_talk_pressed: bool,
    push_to_mute_delay: u64,
    push_to_mute_stop_time: u64,
    push_to_talk_delay: u64,
    push_to_talk_stop_time: u64,
}

/// The four functions, from one side.
struct Gate {
    stop_time: fn(u64, u64) -> u64,
    held: fn(u64, u64) -> bool,
    mutes: fn(bool, bool) -> bool,
    talk_mutes: fn(bool, bool) -> bool,
}

const ORACLE: Gate = Gate {
    stop_time: oracle_stop,
    held: oracle_held,
    mutes: oracle_mutes,
    talk_mutes: oracle_talk_mutes,
};

const SHIM: Gate = Gate {
    stop_time: |os_time, delay| shim::obs_source_ptm_gate_stop_time(os_time, delay),
    held: |os_time, stop| shim::obs_source_ptm_gate_held(os_time, stop),
    mutes: |enabled, active| shim::obs_source_ptm_gate_mutes(enabled, active),
    talk_mutes: |enabled, active| shim::obs_source_ptm_gate_talk_mutes(enabled, active),
};

/// `source_muted()` of `obs-source.c`, on the stand-in, over one side's
/// functions.
fn source_muted(gate: &Gate, source: &mut GateSource, os_time: u64) -> bool {
    if source.push_to_mute_enabled && source.user_push_to_mute_pressed {
        source.push_to_mute_stop_time = (gate.stop_time)(os_time, source.push_to_mute_delay);
    }

    if source.push_to_talk_enabled && source.user_push_to_talk_pressed {
        source.push_to_talk_stop_time = (gate.stop_time)(os_time, source.push_to_talk_delay);
    }

    let push_to_mute_active =
        source.user_push_to_mute_pressed || (gate.held)(os_time, source.push_to_mute_stop_time);
    let push_to_talk_active =
        source.user_push_to_talk_pressed || (gate.held)(os_time, source.push_to_talk_stop_time);

    !source.enabled
        || source.user_muted
        || (gate.mutes)(source.push_to_mute_enabled, push_to_mute_active)
        || (gate.talk_mutes)(source.push_to_talk_enabled, push_to_talk_active)
}

fn source_of(bits: u8, delays: (u64, u64), stops: (u64, u64)) -> GateSource {
    GateSource {
        enabled: bits & 1 != 0,
        user_muted: bits & 2 != 0,
        push_to_mute_enabled: bits & 4 != 0,
        user_push_to_mute_pressed: bits & 8 != 0,
        push_to_talk_enabled: bits & 16 != 0,
        user_push_to_talk_pressed: bits & 32 != 0,
        push_to_mute_delay: delays.0,
        push_to_talk_delay: delays.1,
        push_to_mute_stop_time: stops.0,
        push_to_talk_stop_time: stops.1,
    }
}

#[track_caller]
fn check_source_muted(source: GateSource, os_time: u64) {
    let mut c = source;
    let mut rust = source;
    let c_muted = source_muted(&ORACLE, &mut c, os_time);
    let rust_muted = source_muted(&SHIM, &mut rust, os_time);
    assert_eq!(
        (rust_muted, rust),
        (c_muted, c),
        "source_muted({source:?}, {os_time})"
    );
}

#[test]
fn source_muted_every_flag() {
    let mut runs = 0_u32;
    for os_time in [T, MAX_DELAY_SLACK + 1, u64::MAX - 1] {
        let stops = [
            0,
            os_time.wrapping_sub(1),
            os_time,
            os_time.wrapping_add(1),
            u64::MAX,
        ];
        for bits in 0_u8..64 {
            for &ptm in &stops {
                for &ptt in &stops {
                    for delays in [(0, 0), (300, 500), (MAX_DELAY + 1, MAX_DELAY)] {
                        check_source_muted(source_of(bits, delays, (ptm, ptt)), os_time);
                        runs += 1;
                    }
                }
            }
        }
    }
    assert_eq!(runs, 14_400);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn stop_time_full_range(os_time in any::<u64>(), delay in any::<u64>()) {
        check_stop(os_time, delay);
    }

    #[test]
    fn stop_time_near_the_edges(
        os_time in prop_oneof![0..=2 * MAX_DELAY_SLACK, u64::MAX - 1000 * SEC..=u64::MAX, any::<u64>()],
        delay in prop_oneof![0..=1000_u64, MAX_DELAY - 1000..=MAX_DELAY + 1000, any::<u64>()],
    ) {
        check_stop(os_time, delay);
    }

    #[test]
    fn held_full_range(os_time in any::<u64>(), stop in any::<u64>()) {
        check_held(os_time, stop);
    }

    #[test]
    fn source_muted_full_range(
        bits in 0_u8..64,
        delays in (any::<u64>(), any::<u64>()),
        stops in (any::<u64>(), any::<u64>()),
        os_time in any::<u64>(),
    ) {
        check_source_muted(source_of(bits, delays, stops), os_time);
    }
}
