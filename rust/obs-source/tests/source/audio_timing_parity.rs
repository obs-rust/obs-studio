//! Tier 3: the `obs_source_audio_timing_*` shims against the original C
//! (`obs-c-oracle`, `oracle/source_audio_timing.c`: the unmodified
//! `libobs/obs-source-audio-timing.c.h` with its functions renamed).
//!
//! The structs cross the C ABI as bytes: each run fills one buffer with a
//! random byte pattern (padding and output fields included), writes the
//! inputs, copies it, runs the C step on one copy and the shim on the other,
//! and compares every byte. So the outputs must be equal, and neither side
//! may write anything the other does not (an input, an unwritten output,
//! the padding).
//!
//! Runs:
//! - the layout of both structs and the enum, the action values and the
//!   thresholds against the C header;
//! - each step over the special values of the full `u64` and `i64` ranges
//!   (the thresholds +-1, both ends, 2^63) with every flag combination;
//! - proptests of each step over full-range values, with the expected
//!   timestamp, the system time and `next_audio_sys_ts_min` drawn near the
//!   thresholds as often as anywhere;
//! - proptests of sequences of 2-50 packets through the model of
//!   `source_output_audio_data()` (the stores, `handle_ts_jump()`, the
//!   buffer start of `source_output_audio_place()`), one copy driven by the
//!   C steps and one by the shims, from a random source state, with the
//!   sync offset changed, the flags toggled, the timing moved by the video
//!   thread and `timing_set` cleared by the audio mix between packets.
//!   The step structs, `buffer_start` and the written-back state must be
//!   equal after every packet.
//!
//! Excluded, the intentional difference: a null pointer, which the C
//! dereferences and the shims ignore (checked on the Rust side only).

use std::ffi::c_void;
use std::mem::{align_of, offset_of, size_of};
use std::ptr::{addr_of, addr_of_mut};

use obs_c_oracle::source_audio_timing as oracle;
use obs_source::audio_timing::{MAX_TS_VAR, TS_SMOOTHING_THRESHOLD};
use obs_source::ffi::source_audio_timing::{
    self as shim, obs_source_audio_timing_pre as Pre, obs_source_audio_timing_under as Under,
};
use proptest::prelude::*;

const SMOOTH: u64 = TS_SMOOTHING_THRESHOLD;
const MAX_VAR: u64 = MAX_TS_VAR;
const PRE_SIZE: usize = size_of::<Pre>();
const UNDER_SIZE: usize = size_of::<Under>();

#[test]
fn layout_matches_the_c_header() {
    let rust: [usize; 39] = [
        size_of::<Pre>(),
        align_of::<Pre>(),
        offset_of!(Pre, timestamp),
        offset_of!(Pre, os_time),
        offset_of!(Pre, duration),
        offset_of!(Pre, next_audio_ts_min),
        offset_of!(Pre, timing_adjust),
        offset_of!(Pre, diff),
        offset_of!(Pre, drift_error),
        offset_of!(Pre, action),
        offset_of!(Pre, timing_set),
        offset_of!(Pre, async_unbuffered),
        offset_of!(Pre, async_decoupled),
        offset_of!(Pre, direct),
        offset_of!(Pre, reanchor),
        offset_of!(Pre, continuous),
        size_of::<Under>(),
        align_of::<Under>(),
        offset_of!(Under, timestamp),
        offset_of!(Under, packet_timing_adjust),
        offset_of!(Under, data_timestamp),
        offset_of!(Under, os_time),
        offset_of!(Under, next_audio_ts_min),
        offset_of!(Under, resample_offset),
        offset_of!(Under, sync_offset),
        offset_of!(Under, timing_adjust),
        offset_of!(Under, next_audio_sys_ts_min),
        offset_of!(Under, last_sync_offset),
        offset_of!(Under, continuous),
        offset_of!(Under, push_back),
        offset_of!(Under, reset_timing),
        size_of::<shim::obs_source_audio_ts_action>(),
        shim::OBS_SOURCE_AUDIO_TS_KEEP as usize,
        shim::OBS_SOURCE_AUDIO_TS_RESET as usize,
        shim::OBS_SOURCE_AUDIO_TS_JUMP as usize,
        shim::OBS_SOURCE_AUDIO_TS_SMOOTH as usize,
        shim::OBS_SOURCE_AUDIO_TS_EXCEEDED as usize,
        TS_SMOOTHING_THRESHOLD as usize,
        MAX_TS_VAR as usize,
    ];
    for (which, (name, value)) in oracle::LAYOUT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a pure lookup.
        let c = unsafe { oracle::oracle_source_audio_timing_layout(which) };
        assert_eq!(value, c, "{name}");
    }
}

#[test]
fn null_pointers_do_nothing_in_the_shims() {
    // SAFETY: the shims check for null (the intentional difference).
    unsafe {
        shim::obs_source_audio_timing_pre_lock(std::ptr::null_mut());
        shim::obs_source_audio_timing_under_lock(std::ptr::null_mut());
    }
}

// --------------------------------------------------------------------------
// The structs as bytes

/// A struct's bytes, aligned for it.
#[repr(C, align(8))]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Bytes<const N: usize>([u8; N]);

type PreBytes = Bytes<PRE_SIZE>;
type UnderBytes = Bytes<UNDER_SIZE>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct PreIn {
    timestamp: u64,
    os_time: u64,
    duration: u64,
    next_audio_ts_min: u64,
    timing_set: bool,
    async_unbuffered: bool,
    async_decoupled: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct UnderIn {
    timestamp: u64,
    packet_timing_adjust: u64,
    data_timestamp: u64,
    os_time: u64,
    next_audio_ts_min: u64,
    resample_offset: u64,
    sync_offset: i64,
    timing_adjust: u64,
    next_audio_sys_ts_min: u64,
    last_sync_offset: i64,
    continuous: bool,
}

fn pre_bytes(input: PreIn, fill: &[u8]) -> PreBytes {
    let mut b = Bytes([0; PRE_SIZE]);
    for (i, byte) in b.0.iter_mut().enumerate() {
        *byte = fill[i % fill.len()];
    }
    let p = b.0.as_mut_ptr().cast::<Pre>();
    // SAFETY: b is a PRE_SIZE buffer aligned for Pre; each write stores one
    // field, and no reference to the (partly garbage) struct is made.
    unsafe {
        addr_of_mut!((*p).timestamp).write(input.timestamp);
        addr_of_mut!((*p).os_time).write(input.os_time);
        addr_of_mut!((*p).duration).write(input.duration);
        addr_of_mut!((*p).next_audio_ts_min).write(input.next_audio_ts_min);
        addr_of_mut!((*p).timing_set).write(input.timing_set);
        addr_of_mut!((*p).async_unbuffered).write(input.async_unbuffered);
        addr_of_mut!((*p).async_decoupled).write(input.async_decoupled);
    }
    b
}

fn under_bytes(input: UnderIn, fill: &[u8]) -> UnderBytes {
    let mut b = Bytes([0; UNDER_SIZE]);
    for (i, byte) in b.0.iter_mut().enumerate() {
        *byte = fill[i % fill.len()];
    }
    let p = b.0.as_mut_ptr().cast::<Under>();
    // SAFETY: as in pre_bytes.
    unsafe {
        addr_of_mut!((*p).timestamp).write(input.timestamp);
        addr_of_mut!((*p).packet_timing_adjust).write(input.packet_timing_adjust);
        addr_of_mut!((*p).data_timestamp).write(input.data_timestamp);
        addr_of_mut!((*p).os_time).write(input.os_time);
        addr_of_mut!((*p).next_audio_ts_min).write(input.next_audio_ts_min);
        addr_of_mut!((*p).resample_offset).write(input.resample_offset);
        addr_of_mut!((*p).sync_offset).write(input.sync_offset);
        addr_of_mut!((*p).timing_adjust).write(input.timing_adjust);
        addr_of_mut!((*p).next_audio_sys_ts_min).write(input.next_audio_sys_ts_min);
        addr_of_mut!((*p).last_sync_offset).write(input.last_sync_offset);
        addr_of_mut!((*p).continuous).write(input.continuous);
    }
    b
}

/// The C and the shim on copies of the same bytes; panics with both when
/// they differ.
#[track_caller]
fn pre_both(input: PreIn, fill: &[u8]) -> Pre {
    let mut c = pre_bytes(input, fill);
    let mut rust = c;
    // SAFETY: valid, aligned struct bytes with initialized inputs.
    unsafe {
        oracle::oracle_obs_source_audio_timing_pre_lock(c.0.as_mut_ptr().cast::<c_void>());
        shim::obs_source_audio_timing_pre_lock(rust.0.as_mut_ptr().cast::<Pre>());
    }
    assert_eq!(rust, c, "{input:?} fill {fill:02x?}");
    // SAFETY: every field is now initialized and valid (the C wrote the
    // outputs, the bytes are equal).
    unsafe { addr_of!(rust.0).cast::<Pre>().read() }
}

#[track_caller]
fn under_both(input: UnderIn, fill: &[u8]) -> Under {
    let mut c = under_bytes(input, fill);
    let mut rust = c;
    // SAFETY: as in pre_both.
    unsafe {
        oracle::oracle_obs_source_audio_timing_under_lock(c.0.as_mut_ptr().cast::<c_void>());
        shim::obs_source_audio_timing_under_lock(rust.0.as_mut_ptr().cast::<Under>());
    }
    assert_eq!(rust, c, "{input:?} fill {fill:02x?}");
    // SAFETY: as in pre_both.
    unsafe { addr_of!(rust.0).cast::<Under>().read() }
}

#[track_caller]
fn start_both(os_time: u64, sync_offset: i64, resample_offset: u64) -> u64 {
    // SAFETY: pure functions.
    let c = unsafe {
        oracle::oracle_obs_source_audio_timing_buffer_start(os_time, sync_offset, resample_offset)
    };
    let rust = shim::obs_source_audio_timing_buffer_start(os_time, sync_offset, resample_offset);
    assert_eq!(
        rust, c,
        "buffer_start({os_time}, {sync_offset}, {resample_offset})"
    );
    rust
}

// --------------------------------------------------------------------------
// Special values

const SPECIAL: [u64; 16] = [
    0,
    1,
    SMOOTH - 1,
    SMOOTH,
    SMOOTH + 1,
    MAX_VAR - 1,
    MAX_VAR,
    MAX_VAR + 1,
    5_000_000_000,
    1_000_000_000_000_000,
    1 << 63,
    (1 << 63) - 1,
    u64::MAX - MAX_VAR,
    u64::MAX - SMOOTH,
    u64::MAX - 1,
    u64::MAX,
];

const DELTAS: [u64; 9] = [
    0,
    1,
    SMOOTH - 1,
    SMOOTH,
    SMOOTH + 1,
    MAX_VAR - 1,
    MAX_VAR,
    MAX_VAR + 1,
    1 << 63,
];

const SPECIAL_I64: [i64; 9] = [
    0,
    1,
    -1,
    300_000_000,
    -300_000_000,
    i64::MAX,
    i64::MIN,
    i64::MAX - 1,
    i64::MIN + 1,
];

const FILLS: [&[u8]; 3] = [&[0xa5], &[0x00], &[0xff, 0x5a, 0x00, 0x13]];

#[test]
fn pre_lock_special_values() {
    let mut runs = 0u64;
    for &timestamp in &SPECIAL {
        for &os_delta in &DELTAS {
            for os_time in [
                timestamp.wrapping_add(os_delta),
                timestamp.wrapping_sub(os_delta),
            ] {
                for &next_delta in &DELTAS {
                    for next in [
                        timestamp.wrapping_add(next_delta),
                        timestamp.wrapping_sub(next_delta),
                        0,
                    ] {
                        for duration in [0, 21_333_333, u64::MAX] {
                            for flags in 0..8u8 {
                                let fill = FILLS[usize::from(flags) % FILLS.len()];
                                pre_both(
                                    PreIn {
                                        timestamp,
                                        os_time,
                                        duration,
                                        next_audio_ts_min: next,
                                        timing_set: flags & 1 != 0,
                                        async_unbuffered: flags & 2 != 0,
                                        async_decoupled: flags & 4 != 0,
                                    },
                                    fill,
                                );
                                runs += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(runs, 16 * 9 * 2 * 9 * 3 * 3 * 8);
}

#[test]
fn under_lock_special_values() {
    let mut runs = 0u64;
    for &timestamp in &SPECIAL {
        for &adjust in &[0, 1, 995_000_000_000_000, u64::MAX, 1 << 63] {
            let sys = timestamp.wrapping_add(adjust);
            for &delta in &DELTAS {
                for next_sys in [sys.wrapping_add(delta), sys.wrapping_sub(delta), 0] {
                    for &sync in &SPECIAL_I64 {
                        for last in [sync, 0, sync.wrapping_add(1)] {
                            for continuous in [false, true] {
                                for &other in &[0, 1_000_000_000_000_000, u64::MAX] {
                                    let fill = FILLS[usize::try_from(runs % 3).unwrap()];
                                    under_both(
                                        UnderIn {
                                            timestamp,
                                            packet_timing_adjust: adjust,
                                            data_timestamp: timestamp ^ other,
                                            os_time: other,
                                            next_audio_ts_min: timestamp.wrapping_add(21_333_333),
                                            resample_offset: other >> 40,
                                            sync_offset: sync,
                                            timing_adjust: adjust.wrapping_add(other),
                                            next_audio_sys_ts_min: next_sys,
                                            last_sync_offset: last,
                                            continuous,
                                        },
                                        fill,
                                    );
                                    runs += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(runs, 16 * 5 * 9 * 3 * 9 * 3 * 2 * 3);
}

#[test]
fn buffer_start_special_values() {
    for &os_time in &SPECIAL {
        for &sync in &SPECIAL_I64 {
            for &resample in &SPECIAL {
                start_both(os_time, sync, resample);
            }
        }
    }
}

// --------------------------------------------------------------------------
// Random single steps

/// A value near `base`: within the thresholds +-2 as often as anywhere.
fn near(base: u64) -> impl Strategy<Value = u64> {
    prop_oneof![
        any::<u64>(),
        Just(base),
        (-3_000_000_000i64..3_000_000_000).prop_map(move |d| base.wrapping_add_signed(d)),
        (
            prop::sample::select(DELTAS.to_vec()),
            -2i64..=2,
            any::<bool>()
        )
            .prop_map(move |(d, e, up)| {
                let d = d.wrapping_add_signed(e);
                if up {
                    base.wrapping_add(d)
                } else {
                    base.wrapping_sub(d)
                }
            }),
    ]
}

fn timestamp() -> impl Strategy<Value = u64> {
    prop_oneof![
        any::<u64>(),
        prop::sample::select(SPECIAL.to_vec()),
        0u64..10_000_000_000_000,
    ]
}

fn offset() -> impl Strategy<Value = i64> {
    prop_oneof![
        any::<i64>(),
        prop::sample::select(SPECIAL_I64.to_vec()),
        -10_000_000_000i64..10_000_000_000,
    ]
}

fn fill() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 1..=UNDER_SIZE)
}

fn pre_input() -> impl Strategy<Value = PreIn> {
    timestamp()
        .prop_flat_map(|ts| {
            (
                Just(ts),
                near(ts),
                prop_oneof![Just(0u64), near(ts)],
                prop_oneof![any::<u64>(), 0u64..100_000_000],
                0u8..8,
            )
        })
        .prop_map(|(timestamp, os_time, next, duration, flags)| PreIn {
            timestamp,
            os_time,
            duration,
            next_audio_ts_min: next,
            timing_set: flags & 1 != 0,
            async_unbuffered: flags & 2 != 0,
            async_decoupled: flags & 4 != 0,
        })
}

fn under_input() -> impl Strategy<Value = UnderIn> {
    (timestamp(), any::<u64>(), offset(), any::<bool>())
        .prop_flat_map(|(ts, adjust, sync, continuous)| {
            let sys = ts.wrapping_add(adjust);
            (
                Just((ts, adjust, sync, continuous)),
                prop_oneof![Just(0u64), near(sys)],
                prop_oneof![Just(ts), any::<u64>(), near(ts)],
                near(sys),
                prop_oneof![Just(adjust), any::<u64>(), near(adjust)],
                prop_oneof![Just(sync), offset()],
                prop_oneof![Just(0u64), any::<u64>(), 0u64..10_000_000],
                any::<u64>(),
            )
        })
        .prop_map(
            |(
                (ts, adjust, sync, continuous),
                next_sys,
                data_ts,
                os_time,
                under_adjust,
                last,
                resample,
                next,
            )| UnderIn {
                timestamp: ts,
                packet_timing_adjust: adjust,
                data_timestamp: data_ts,
                os_time,
                next_audio_ts_min: next,
                resample_offset: resample,
                sync_offset: sync,
                timing_adjust: under_adjust,
                next_audio_sys_ts_min: next_sys,
                last_sync_offset: last,
                continuous,
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn pre_lock_any_input(input in pre_input(), fill in fill()) {
        pre_both(input, &fill);
    }

    #[test]
    fn under_lock_any_input(input in under_input(), fill in fill()) {
        under_both(input, &fill);
    }

    #[test]
    fn buffer_start_any_input(os_time in timestamp(), sync in offset(), resample in timestamp()) {
        start_both(os_time, sync, resample);
    }
}

// --------------------------------------------------------------------------
// Packet sequences

/// The fields of `struct obs_source` the steps read and write, and the
/// buffer the model keeps (as `test_source_audio_timing_steps`'s fake
/// source).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct State {
    timing_set: bool,
    timing_adjust: u64,
    next_audio_ts_min: u64,
    next_audio_sys_ts_min: u64,
    resample_offset: u64,
    sync_offset: i64,
    last_sync_offset: i64,
    async_unbuffered: bool,
    async_decoupled: bool,
    audio_ts: u64,
    buf_end: u64,
    pushed: bool,
    placed: u64,
}

/// What happens before and during one packet.
#[derive(Debug, Clone, Copy)]
struct Packet {
    timestamp: u64,
    os_time: u64,
    duration: u64,
    /// The sync offset set before the packet, if any.
    sync_offset: Option<i64>,
    /// The resampler offset of this packet.
    resample_offset: u64,
    /// The async flags, toggled before the packet.
    flags: Option<u8>,
    /// The audio mix cleared timing_set before the packet.
    clear_timing: bool,
    /// The video thread moved timing_adjust before the packet.
    video_adjust: Option<u64>,
    /// ... and again between the two reads (before and under the lock).
    video_adjust_under: Option<u64>,
}

type PreFn = unsafe fn(*mut u8);
type UnderFn = unsafe fn(*mut u8);
type StartFn = fn(u64, i64, u64) -> u64;

unsafe fn c_pre(p: *mut u8) {
    // SAFETY: forwarded caller contract.
    unsafe { oracle::oracle_obs_source_audio_timing_pre_lock(p.cast::<c_void>()) }
}
unsafe fn rust_pre(p: *mut u8) {
    // SAFETY: forwarded caller contract.
    unsafe { shim::obs_source_audio_timing_pre_lock(p.cast::<Pre>()) }
}
unsafe fn c_under(p: *mut u8) {
    // SAFETY: forwarded caller contract.
    unsafe { oracle::oracle_obs_source_audio_timing_under_lock(p.cast::<c_void>()) }
}
unsafe fn rust_under(p: *mut u8) {
    // SAFETY: forwarded caller contract.
    unsafe { shim::obs_source_audio_timing_under_lock(p.cast::<Under>()) }
}
fn c_start(os_time: u64, sync_offset: i64, resample_offset: u64) -> u64 {
    // SAFETY: a pure function.
    unsafe {
        oracle::oracle_obs_source_audio_timing_buffer_start(os_time, sync_offset, resample_offset)
    }
}
fn rust_start(os_time: u64, sync_offset: i64, resample_offset: u64) -> u64 {
    shim::obs_source_audio_timing_buffer_start(os_time, sync_offset, resample_offset)
}

/// One packet through the model of `source_output_audio_data()`, with the
/// given steps. Returns the step structs as bytes.
fn packet(
    st: &mut State,
    pkt: &Packet,
    fill: &[u8],
    (pre_fn, under_fn, start_fn): (PreFn, UnderFn, StartFn),
) -> (PreBytes, UnderBytes) {
    if let Some(sync) = pkt.sync_offset {
        st.sync_offset = sync;
    }
    if let Some(flags) = pkt.flags {
        st.async_unbuffered = flags & 1 != 0;
        st.async_decoupled = flags & 2 != 0;
    }
    if pkt.clear_timing {
        st.timing_set = false;
    }
    if let Some(adjust) = pkt.video_adjust {
        st.timing_adjust = adjust;
        st.timing_set = true;
    }
    st.resample_offset = pkt.resample_offset;

    let mut pre_b = pre_bytes(
        PreIn {
            timestamp: pkt.timestamp,
            os_time: pkt.os_time,
            duration: pkt.duration,
            next_audio_ts_min: st.next_audio_ts_min,
            timing_set: st.timing_set,
            async_unbuffered: st.async_unbuffered,
            async_decoupled: st.async_decoupled,
        },
        fill,
    );
    // SAFETY: valid, aligned struct bytes with initialized inputs.
    unsafe { pre_fn(pre_b.0.as_mut_ptr()) };
    // SAFETY: the step wrote every output (the parity of the bytes is
    // checked by the caller).
    let pre = unsafe { addr_of!(pre_b.0).cast::<Pre>().read() };

    if pre.direct {
        st.timing_adjust = 0;
        st.timing_set = true;
    }
    match pre.action {
        shim::OBS_SOURCE_AUDIO_TS_RESET => {
            st.timing_set = true;
            st.timing_adjust = pre.timing_adjust;
        }
        shim::OBS_SOURCE_AUDIO_TS_JUMP => {
            // handle_ts_jump(): reset_audio_timing() (C arithmetic), then
            // reset_audio_data_at_sys_time()
            st.timing_set = true;
            st.timing_adjust = pkt.os_time.wrapping_sub(pkt.timestamp);
            st.audio_ts = pkt.os_time;
            st.buf_end = pkt.os_time;
            st.next_audio_sys_ts_min = pkt.os_time;
            st.audio_ts = start_fn(pkt.os_time, st.sync_offset, st.resample_offset);
            st.buf_end = st.audio_ts;
        }
        shim::OBS_SOURCE_AUDIO_TS_SMOOTH if pre.reanchor => {
            st.timing_adjust = pre.timing_adjust;
        }
        _ => {}
    }
    st.next_audio_ts_min = pre.next_audio_ts_min;
    let packet_timing_adjust = st.timing_adjust;
    if let Some(adjust) = pkt.video_adjust_under {
        st.timing_adjust = adjust;
    }

    let mut under_b = under_bytes(
        UnderIn {
            timestamp: pre.timestamp,
            packet_timing_adjust,
            data_timestamp: pkt.timestamp,
            os_time: pkt.os_time,
            next_audio_ts_min: st.next_audio_ts_min,
            resample_offset: st.resample_offset,
            sync_offset: st.sync_offset,
            timing_adjust: st.timing_adjust,
            next_audio_sys_ts_min: st.next_audio_sys_ts_min,
            last_sync_offset: st.last_sync_offset,
            continuous: pre.continuous,
        },
        fill,
    );
    // SAFETY: as above.
    unsafe { under_fn(under_b.0.as_mut_ptr()) };
    // SAFETY: as above.
    let under = unsafe { addr_of!(under_b.0).cast::<Under>().read() };

    if under.reset_timing {
        st.timing_set = true;
        st.timing_adjust = under.timing_adjust;
    }
    st.next_audio_sys_ts_min = under.next_audio_sys_ts_min;
    st.last_sync_offset = under.last_sync_offset;

    st.pushed = under.push_back && st.audio_ts != 0;
    if st.pushed {
        st.placed = st.buf_end;
    } else {
        if st.audio_ts == 0 || under.timestamp < st.audio_ts {
            st.audio_ts = under.timestamp;
            st.buf_end = under.timestamp;
            st.next_audio_sys_ts_min = under.timestamp;
        }
        st.placed = under.timestamp;
    }
    st.buf_end = st.placed.wrapping_add(pkt.duration);
    (pre_b, under_b)
}

/// How a packet's timestamp and system time follow the previous one.
#[derive(Debug, Clone, Copy)]
enum Step {
    /// Where the previous packet ended, with jitter, on a clock advancing
    /// by about the duration.
    Continue { jitter: i64, clock_jitter: i64 },
    /// A jump of the media timestamp by any amount.
    Jump { by: u64 },
    /// On the system clock (direct).
    Direct { lag: i64 },
    /// Anything.
    Random { timestamp: u64, os_time: u64 },
}

fn step() -> impl Strategy<Value = Step> {
    let jitter = prop_oneof![
        Just(0i64),
        -100_000_000i64..100_000_000,
        prop::sample::select(vec![
            69_999_999i64,
            70_000_000,
            70_000_001,
            -69_999_999,
            -70_000_000,
            -70_000_001,
            1_999_999_999,
            2_000_000_000,
            2_000_000_001,
            -2_000_000_001,
        ]),
    ];
    prop_oneof![
        4 => (jitter.clone(), -3_000_000_000i64..3_000_000_000)
            .prop_map(|(jitter, clock_jitter)| Step::Continue { jitter, clock_jitter: clock_jitter / 1000 }),
        1 => (jitter.clone(), jitter.clone()).prop_map(|(jitter, clock_jitter)| Step::Continue { jitter, clock_jitter }),
        1 => any::<u64>().prop_map(|by| Step::Jump { by }),
        1 => (-2_500_000_000i64..2_500_000_000).prop_map(|lag| Step::Direct { lag }),
        1 => (any::<u64>(), any::<u64>()).prop_map(|(timestamp, os_time)| Step::Random { timestamp, os_time }),
    ]
}

#[derive(Debug, Clone)]
struct Event {
    step: Step,
    duration: u64,
    sync_offset: Option<i64>,
    resample_offset: u64,
    flags: Option<u8>,
    clear_timing: bool,
    video_adjust: Option<i64>,
    video_adjust_under: Option<i64>,
}

fn event() -> impl Strategy<Value = Event> {
    (
        step(),
        prop_oneof![8 => Just(21_333_333u64), 1 => 0u64..200_000_000, 1 => any::<u64>()],
        prop_oneof![6 => Just(None), 1 => offset().prop_map(Some)],
        prop_oneof![4 => Just(0u64), 1 => 0u64..1_000_000, 1 => any::<u64>()],
        prop_oneof![6 => Just(None), 1 => (0u8..4).prop_map(Some)],
        prop::bool::weighted(0.05),
        prop_oneof![
            6 => Just(None),
            1 => (-5_000_000_000i64..5_000_000_000).prop_map(Some),
            1 => any::<i64>().prop_map(Some),
        ],
        prop_oneof![8 => Just(None), 1 => (-5_000_000_000i64..5_000_000_000).prop_map(Some)],
    )
        .prop_map(
            |(
                step,
                duration,
                sync_offset,
                resample_offset,
                flags,
                clear_timing,
                video_adjust,
                video_adjust_under,
            )| Event {
                step,
                duration,
                sync_offset,
                resample_offset,
                flags,
                clear_timing,
                video_adjust,
                video_adjust_under,
            },
        )
}

fn initial_state() -> impl Strategy<Value = State> {
    prop_oneof![
        Just(State::default()),
        (
            any::<bool>(),
            any::<u64>(),
            prop_oneof![Just(0u64), any::<u64>()],
            prop_oneof![Just(0u64), any::<u64>()],
            offset(),
            offset(),
            0u8..4,
            prop_oneof![Just(0u64), any::<u64>()],
        )
            .prop_map(
                |(timing_set, timing_adjust, next, next_sys, sync, last, flags, audio_ts)| State {
                    timing_set,
                    timing_adjust,
                    next_audio_ts_min: next,
                    next_audio_sys_ts_min: next_sys,
                    sync_offset: sync,
                    last_sync_offset: last,
                    async_unbuffered: flags & 1 != 0,
                    async_decoupled: flags & 2 != 0,
                    audio_ts,
                    buf_end: audio_ts,
                    ..State::default()
                }
            ),
    ]
}

/// Turns the events into packets, from where the previous one ended.
fn packets(start_ts: u64, start_os: u64, events: &[Event], state_adjust: u64) -> Vec<Packet> {
    let (mut ts, mut os) = (start_ts, start_os);
    let mut out = Vec::with_capacity(events.len());
    for e in events {
        let (timestamp, os_time) = match e.step {
            Step::Continue {
                jitter,
                clock_jitter,
            } => (
                ts.wrapping_add_signed(jitter),
                os.wrapping_add_signed(clock_jitter),
            ),
            Step::Jump { by } => (ts.wrapping_add(by), os),
            Step::Direct { lag } => (os.wrapping_add_signed(lag), os),
            Step::Random { timestamp, os_time } => (timestamp, os_time),
        };
        out.push(Packet {
            timestamp,
            os_time,
            duration: e.duration,
            sync_offset: e.sync_offset,
            resample_offset: e.resample_offset,
            flags: e.flags,
            clear_timing: e.clear_timing,
            video_adjust: e.video_adjust.map(|d| state_adjust.wrapping_add_signed(d)),
            video_adjust_under: e
                .video_adjust_under
                .map(|d| state_adjust.wrapping_add_signed(d)),
        });
        ts = timestamp.wrapping_add(e.duration);
        os = os_time.wrapping_add(e.duration);
    }
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn sequences_match_the_c(
        state in initial_state(),
        start_ts in timestamp(),
        start_os in prop_oneof![timestamp(), Just(1_000_000_000_000_000u64)],
        events in prop::collection::vec(event(), 2..=50),
        fill in fill(),
    ) {
        let adjust0 = start_os.wrapping_sub(start_ts);
        let packets = packets(start_ts, start_os, &events, adjust0);
        let (mut c, mut rust) = (state, state);
        for (i, pkt) in packets.iter().enumerate() {
            let (c_pre_b, c_under_b) = packet(&mut c, pkt, &fill, (c_pre, c_under, c_start));
            let (r_pre_b, r_under_b) = packet(&mut rust, pkt, &fill, (rust_pre, rust_under, rust_start));
            prop_assert_eq!(r_pre_b, c_pre_b, "pre-lock step of packet {} {:?}", i, pkt);
            prop_assert_eq!(r_under_b, c_under_b, "under-lock step of packet {} {:?}", i, pkt);
            prop_assert_eq!(rust, c, "state after packet {} {:?}", i, pkt);
        }
    }
}

/// The sequences reach every action and both under-lock outcomes, so the
/// parity above is not vacuous.
#[test]
fn sequences_reach_every_branch() {
    use proptest::strategy::ValueTree;
    use proptest::test_runner::TestRunner;

    let mut runner = TestRunner::deterministic();
    let strategy = (
        initial_state(),
        timestamp(),
        prop_oneof![timestamp(), Just(1_000_000_000_000_000u64)],
        prop::collection::vec(event(), 2..=50),
    );
    let mut seen_actions = [0u32; 5];
    let (mut pushed, mut placed, mut resets, mut reanchors, mut directs, mut changed) =
        (0, 0, 0, 0, 0, 0);
    for _ in 0..2000 {
        let (state, start_ts, start_os, events) = strategy.new_tree(&mut runner).unwrap().current();
        let packets = packets(start_ts, start_os, &events, start_os.wrapping_sub(start_ts));
        let mut st = state;
        for pkt in &packets {
            let before = st.last_sync_offset;
            let (pre_b, under_b) = packet(&mut st, pkt, &[0], (rust_pre, rust_under, rust_start));
            // SAFETY: written by the steps.
            let (pre, under) = unsafe {
                (
                    addr_of!(pre_b.0).cast::<Pre>().read(),
                    addr_of!(under_b.0).cast::<Under>().read(),
                )
            };
            seen_actions[usize::try_from(pre.action).unwrap()] += 1;
            reanchors += u32::from(pre.reanchor);
            directs += u32::from(pre.direct);
            resets += u32::from(under.reset_timing);
            changed += u32::from(before != under.last_sync_offset);
            if st.pushed {
                pushed += 1;
            } else {
                placed += 1;
            }
        }
    }
    let counts = [pushed, placed, resets, reanchors, directs, changed];
    assert!(seen_actions.iter().all(|&n| n > 100), "{seen_actions:?}");
    assert!(counts.iter().all(|&n| n > 100), "{counts:?}");
}
