//! Tier 3 for `obs_source_seam::ffi::source_audio_output` against the original
//! `libobs/obs-source-audio-output.c.h` (`obs_c_oracle::source_audio_output`,
//! renamed `oracle_*`).
//!
//! Both sides run on their own test source (built by the oracle) from the same
//! starting fields, on the same fake clock. After every packet the two must
//! agree on the log (each clock read, output query, compensation call, log line
//! with its values, `source_muted()` and capture callback, in order, with every
//! field it saw), on every field, on the bytes in each buffer and on the mutex
//! being free.
//!
//! There is no intentional difference from the C.

use std::ffi::{c_char, c_void};

use obs_c_oracle::source_audio_output as c;
use obs_c_oracle::source_audio_output::Init;
use obs_source_seam::ffi::source_audio_output as rs;
use proptest::prelude::*;

const RATE: u32 = 48_000;
const FRAMES: u32 = 1024;
const NOW: u64 = 100_000_000_000;
const FAR: u64 = 3_700_000_000_000;
const MSEC: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
/// `duration(RATE, FRAMES)`.
const DUR: u64 = 21_333_333;

struct Pair {
    rust: *mut c_void,
    c: *mut c_void,
    now: u64,
    rate: u32,
    channels: usize,
}

impl Pair {
    fn new(init: Init) -> Self {
        // SAFETY: plain values, and the sources are the ones just made.
        unsafe {
            let rust = c::oracle_audio_output_new();
            let other = c::oracle_audio_output_new();
            c::oracle_audio_output_set_fields(rust, &raw const init);
            c::oracle_audio_output_set_fields(other, &raw const init);
            Self {
                rust,
                c: other,
                now: NOW,
                rate: RATE,
                channels: 2,
            }
        }
    }

    fn state(source: *const c_void) -> String {
        let mut out = vec![0u8; 131_072];
        // SAFETY: a live test source and a buffer of `out.len()` bytes.
        let len = unsafe {
            c::oracle_audio_output_state(source, out.as_mut_ptr().cast::<c_char>(), out.len())
        };
        out.truncate(len);
        String::from_utf8(out).unwrap()
    }

    fn assert_same(&self, what: &str) {
        assert_eq!(Self::state(self.rust), Self::state(self.c), "after {what}");
    }

    fn send(&self, timestamp: u64, frames: u32, value: f32) {
        // SAFETY: plain values, live test sources and a live packet.
        unsafe {
            let data = c::oracle_audio_output_packet_new(timestamp, frames, value);
            c::oracle_audio_output_begin(self.rust, self.now, self.rate, self.channels);
            rs::source_output_audio_data(self.rust, data);
            c::oracle_audio_output_begin(self.c, self.now, self.rate, self.channels);
            c::oracle_source_output_audio_data(self.c, data);
            c::oracle_audio_output_packet_free(data);
        }
        self.assert_same(&format!("packet {timestamp} {frames}"));
    }

    /// The reset functions, called as `obs-source.c.h` does: under the mutex.
    fn reset(&self, run: impl Fn(*mut c_void, bool), what: &str) {
        for (source, is_rust) in [(self.rust, true), (self.c, false)] {
            // SAFETY: a live test source.
            unsafe {
                c::oracle_audio_output_begin(source, self.now, self.rate, self.channels);
                c::oracle_audio_output_lock(source);
            }
            run(source, is_rust);
            // SAFETY: as above, and this thread holds the mutex.
            unsafe { c::oracle_audio_output_unlock(source) };
        }
        self.assert_same(what);
    }
}

impl Drop for Pair {
    fn drop(&mut self) {
        // SAFETY: both are live test sources, freed once.
        unsafe {
            c::oracle_audio_output_free(self.rust);
            c::oracle_audio_output_free(self.c);
        }
    }
}

#[test]
fn packets_on_the_system_clock_match() {
    let mut pair = Pair::new(Init::default());
    pair.send(NOW, FRAMES, 1.0);
    pair.now += DUR;
    pair.send(NOW + DUR + MSEC, FRAMES, 2.0);
    pair.send(NOW + 2 * DUR, FRAMES, 3.0);
    let state = Pair::state(pair.rust);
    assert!(state.ends_with(" free"), "{state}");
    assert!(state.contains("signal unmuted"), "{state}");
    assert!(state.contains(&format!("buf {:016x}", 3 * 4096)), "{state}");
}

#[test]
fn a_timestamp_jump_matches_and_logs_its_values() {
    let mut pair = Pair::new(Init {
        sync_offset: 50,
        last_sync_offset: 50,
        ..Init::default()
    });
    pair.send(FAR, FRAMES, 1.0);
    pair.now += SEC;
    pair.send(FAR + 10 * SEC, FRAMES, 2.0);
    assert!(Pair::state(pair.rust).contains("jumped by"));
}

#[test]
fn past_the_threshold_matches_and_logs_its_values() {
    let pair = Pair::new(Init::default());
    pair.send(NOW, FRAMES, 1.0);
    pair.send(NOW + DUR + 100 * MSEC, FRAMES, 2.0);
    assert!(Pair::state(pair.rust).contains("exceeded"));
}

#[test]
fn the_compensation_call_matches() {
    let pair = Pair::new(Init {
        compensating: true,
        ..Init::default()
    });
    pair.send(NOW, FRAMES, 1.0);
    pair.send(NOW + DUR + MSEC, FRAMES, 2.0);
    assert!(Pair::state(pair.rust).contains("compensation"));
}

#[test]
fn a_monitor_only_source_matches() {
    let pair = Pair::new(Init {
        monitoring_type: 1,
        muted: true,
        ..Init::default()
    });
    pair.send(NOW, FRAMES, 1.0);
    let state = Pair::state(pair.rust);
    assert!(state.contains("signal muted"), "{state}");
    assert!(state.contains(&format!("buf {:016x}", 0)), "{state}");
}

#[test]
fn a_packet_past_the_maximum_matches() {
    let pair = Pair::new(Init {
        timing_set: true,
        audio_ts: 1,
        ..Init::default()
    });
    // a placement of more than MAX_BUF_SIZE: dropped
    pair.send(NOW, FRAMES, 1.0);
    let state = Pair::state(pair.rust);
    assert!(state.contains(&format!("last {:016x}", 7)), "{state}");
}

#[test]
fn a_silent_output_matches() {
    let mut pair = Pair::new(Init::default());
    pair.rate = 0;
    // no sample rate: no duration. The placement divides by nothing either.
    pair.send(NOW, FRAMES, 1.0);
}

#[test]
fn the_reset_functions_match() {
    let pair = Pair::new(Init {
        sync_offset: -5,
        resample_offset: 2,
        last_sync_offset: -5,
        ..Init::default()
    });
    pair.send(FAR, FRAMES, 1.0);

    pair.reset(
        |source, is_rust| {
            // SAFETY: a live test source, under its mutex.
            unsafe {
                if is_rust {
                    rs::reset_audio_timing(source, FAR, NOW);
                } else {
                    c::oracle_reset_audio_timing(source, FAR, NOW);
                }
            }
        },
        "reset_audio_timing",
    );
    pair.reset(
        |source, is_rust| {
            // SAFETY: a live test source, under its mutex.
            unsafe {
                if is_rust {
                    rs::reset_audio_data_at_sys_time(source, NOW);
                } else {
                    c::oracle_reset_audio_data_at_sys_time(source, NOW);
                }
            }
        },
        "reset_audio_data_at_sys_time",
    );
    pair.send(FAR + DUR, FRAMES, 2.0);
    pair.reset(
        |source, is_rust| {
            // SAFETY: a live test source, under its mutex.
            unsafe {
                if is_rust {
                    rs::reset_audio_data(source, 0);
                } else {
                    c::oracle_reset_audio_data(source, 0);
                }
            }
        },
        "reset_audio_data",
    );
    assert!(Pair::state(pair.rust).contains(&format!("ts {:016x}", 0)));
}

/// A stamp relative to the last one: every branch of the timing is near.
#[derive(Debug, Clone, Copy)]
enum Stamp {
    Expected,
    Near(u64),
    Past(u64),
    Jump(u64),
    Older(u64),
    SystemClock,
    Any(u64),
}

fn stamp() -> impl Strategy<Value = Stamp> {
    prop_oneof![
        4 => Just(Stamp::Expected),
        3 => (0..60 * MSEC).prop_map(Stamp::Near),
        2 => (70 * MSEC..SEC).prop_map(Stamp::Past),
        1 => (2 * SEC + 1..3 * SEC).prop_map(Stamp::Jump),
        1 => (0..SEC).prop_map(Stamp::Older),
        1 => Just(Stamp::SystemClock),
        1 => any::<u64>().prop_map(Stamp::Any),
    ]
}

fn init() -> impl Strategy<Value = Init> {
    (
        any::<bool>(),
        prop_oneof![Just(0u64), any::<u64>()],
        prop_oneof![Just(0i64), -SEC.cast_signed()..SEC.cast_signed()],
        0u64..1000,
        0i32..3,
        any::<[bool; 4]>(),
    )
        .prop_map(
            |(timing_set, adjust, sync_offset, resample_offset, monitoring_type, flags)| Init {
                timing_set,
                timing_adjust: adjust,
                sync_offset,
                resample_offset,
                monitoring_type,
                async_unbuffered: flags[0],
                async_decoupled: flags[1],
                compensating: flags[2],
                muted: flags[3],
                ..Init::default()
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(64)))]

    #[test]
    fn any_sequence_matches_the_c(
        init in init(),
        start in prop_oneof![Just(NOW), Just(FAR)],
        channels in 1usize..=2,
        steps in prop::collection::vec((stamp(), 0u32..2048, 0..100 * MSEC), 1..24),
    ) {
        let mut pair = Pair::new(init);
        pair.channels = channels;
        let mut last = start;
        for (index, (stamp, frames, tick)) in steps.into_iter().enumerate() {
            let next = last.wrapping_add(duration(frames));
            let timestamp = match stamp {
                Stamp::Expected => next,
                Stamp::Near(d) => next.wrapping_add(d),
                Stamp::Past(d) => next.wrapping_add(d),
                Stamp::Jump(d) => next.wrapping_add(d),
                Stamp::Older(d) => last.wrapping_sub(d),
                Stamp::SystemClock => pair.now,
                Stamp::Any(v) => v,
            };
            let value = f32::from(u16::try_from(index).unwrap());
            pair.send(timestamp, frames, value);
            pair.now += tick;
            last = timestamp;
        }
    }
}

fn duration(frames: u32) -> u64 {
    u64::from(frames) * SEC / u64::from(RATE)
}
