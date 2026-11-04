//! Tier 3 for `obs_source_seam::ffi::source_audio_input_path` against the original
//! `libobs/obs-source-audio-input-path.c.h`
//! (`obs_c_oracle::source_audio_input_path`, renamed `oracle_*`).
//!
//! Both sides run on their own test source (built by the oracle) from the same
//! starting fields. After every packet the two must agree on the log (each output
//! query, resampler call, log line, balance read, filter and output call, in
//! order, with the planes and values it saw and which mutexes were free), on every
//! field, on the bytes in the storage and on the mutexes being free.
//!
//! One difference from the C, with no effect: the seam copies the filter's packet
//! after it takes `audio_mutex`, and the C does so just before. The log cannot see
//! it, because `filter_mutex` is held for both.

use std::ffi::{c_char, c_void};
use std::ptr;

use obs_c_oracle::source_audio_input_path as c;
use obs_c_oracle::source_audio_input_path::Init;
use obs_source::audio_input::{FORCE_MONO, SPEAKERS_STEREO};
use obs_source_seam::ffi::source_audio_input_path as rs;
use proptest::prelude::*;

const FLOAT: i32 = 4;
const FLOAT_PLANAR: i32 = 8;
const SPEAKERS_MONO: i32 = 1;
const RATE: u32 = 48_000;
const FRAMES: u32 = 1024;
const NOW: u64 = 100_000_000_000;
const MSEC: u64 = 1_000_000;

struct Pair {
    rust: *mut c_void,
    c: *mut c_void,
    speakers: i32,
}

#[derive(Debug, Clone, Copy)]
struct Packet {
    format: i32,
    speakers: i32,
    rate: u32,
    frames: u32,
    timestamp: u64,
}

impl Packet {
    fn plain() -> Self {
        Self {
            format: FLOAT_PLANAR,
            speakers: SPEAKERS_STEREO,
            rate: RATE,
            frames: FRAMES,
            timestamp: NOW,
        }
    }
}

impl Pair {
    fn new(init: Init) -> Self {
        // SAFETY: plain values.
        unsafe {
            Self {
                rust: c::oracle_audio_input_path_new(&raw const init),
                c: c::oracle_audio_input_path_new(&raw const init),
                speakers: SPEAKERS_STEREO,
            }
        }
    }

    fn begin(&self, source: *mut c_void) {
        // SAFETY: a live test source and plain values.
        unsafe { c::oracle_audio_input_path_begin(source, FLOAT_PLANAR, RATE, self.speakers) };
    }

    fn state(&self, source: *mut c_void) -> String {
        self.begin(source);
        let mut out = vec![0u8; 262_144];
        // SAFETY: a live test source and a buffer of `out.len()` bytes.
        let len = unsafe {
            c::oracle_audio_input_path_state(source, out.as_mut_ptr().cast::<c_char>(), out.len())
        };
        out.truncate(len);
        String::from_utf8(out).unwrap()
    }

    fn rust_state(&self) -> String {
        self.state(self.rust)
    }

    fn assert_same(&self, what: &str) {
        assert_eq!(self.state(self.rust), self.state(self.c), "after {what}");
    }

    fn send(&self, packet: Packet, value: f32) {
        // SAFETY: plain values, live test sources and a live packet.
        unsafe {
            let audio = c::oracle_audio_input_path_packet_new(
                packet.format,
                packet.speakers,
                packet.rate,
                packet.frames,
                packet.timestamp,
                value,
            );
            self.begin(self.rust);
            rs::obs_source_output_audio(self.rust, audio);
            self.begin(self.c);
            c::oracle_obs_source_output_audio(self.c, audio);
            c::oracle_audio_input_path_packet_free(audio);
        }
        self.assert_same(&format!("{packet:?}"));
    }

    fn set_compensation(&self, value: bool) {
        // SAFETY: live test sources.
        unsafe {
            c::oracle_audio_input_path_set_compensation(self.rust, value);
            c::oracle_audio_input_path_set_compensation(self.c, value);
        }
    }

    fn set_balance(&self, value: f32) {
        // SAFETY: live test sources.
        unsafe {
            c::oracle_audio_input_path_set_balance(self.rust, value);
            c::oracle_audio_input_path_set_balance(self.c, value);
        }
    }
}

impl Drop for Pair {
    fn drop(&mut self) {
        // SAFETY: both are live test sources, freed once.
        unsafe {
            c::oracle_audio_input_path_free(self.rust);
            c::oracle_audio_input_path_free(self.c);
        }
    }
}

#[test]
fn the_layout_of_the_packet_matches_the_c() {
    for (which, value) in rs::audio_layout().into_iter().enumerate() {
        // SAFETY: plain value.
        assert_eq!(value, unsafe { c::oracle_audio_input_path_layout(which) });
    }
}

#[test]
fn a_null_packet_logs_the_same() {
    let pair = Pair::new(Init::default());
    pair.begin(pair.rust);
    // SAFETY: a live test source, and NULL is what this checks for.
    unsafe { rs::obs_source_output_audio(pair.rust, ptr::null()) };
    pair.begin(pair.c);
    // SAFETY: as above.
    unsafe { c::oracle_obs_source_output_audio(pair.c, ptr::null()) };
    pair.assert_same("a NULL packet");
    assert!(pair.rust_state().contains("Null 'audio_in' parameter"));
}

#[test]
fn a_destroying_source_does_nothing_with_the_packet() {
    let pair = Pair::new(Init {
        destroying: true,
        ..Init::default()
    });
    pair.send(Packet::plain(), 1.0);
    pair.begin(pair.rust);
    // SAFETY: a live test source. A destroying source does not check the packet.
    unsafe { rs::obs_source_output_audio(pair.rust, ptr::null()) };
    pair.begin(pair.c);
    // SAFETY: as above.
    unsafe { c::oracle_obs_source_output_audio(pair.c, ptr::null()) };
    pair.assert_same("a NULL packet");
    assert!(!pair.rust_state().contains("Null"));
}

#[test]
fn a_packet_in_the_output_format_matches() {
    let pair = Pair::new(Init::default());
    pair.send(Packet::plain(), 1.0);
    let next = Packet {
        timestamp: NOW + 21 * MSEC,
        ..Packet::plain()
    };
    pair.send(next, 2.0);
    let state = pair.rust_state();
    assert!(state.ends_with(" free"), "{state}");
    assert!(state.contains("output frames"), "{state}");
    assert!(!state.contains("resample planes"), "{state}");
}

#[test]
fn a_format_change_matches_and_clears_the_unused_planes() {
    let pair = Pair::new(Init::default());
    pair.send(Packet::plain(), 1.0);
    let mono = Packet {
        speakers: SPEAKERS_MONO,
        rate: 44_100,
        ..Packet::plain()
    };
    pair.send(mono, 2.0);
    let state = pair.rust_state();
    // only plane 0 of a mono packet reaches the resampler
    assert!(
        state.contains(&format!("resample planes {:016x}", 1)),
        "{state}"
    );
    assert!(state.contains("destroy"), "{state}");
    pair.send(Packet::plain(), 3.0);
    assert!(pair.rust_state().contains(" noresampler"));
}

#[test]
fn a_discontinuity_and_the_compensation_flag_match() {
    let pair = Pair::new(Init::default());
    pair.set_compensation(true);
    pair.send(Packet::plain(), 1.0);
    let far = Packet {
        timestamp: NOW + 500 * MSEC,
        ..Packet::plain()
    };
    pair.send(far, 2.0);
    pair.set_compensation(false);
    let next = Packet {
        timestamp: far.timestamp + 21 * MSEC,
        ..Packet::plain()
    };
    pair.send(next, 3.0);
    assert!(pair.rust_state().contains(" noresampler"));
}

#[test]
fn a_failed_resampler_sends_the_previous_packet_on_the_same() {
    let pair = Pair::new(Init {
        create_fails: true,
        ..Init::default()
    });
    pair.send(Packet::plain(), 1.0);
    let mono = Packet {
        speakers: SPEAKERS_MONO,
        ..Packet::plain()
    };
    pair.send(mono, 2.0);
    let state = pair.rust_state();
    assert!(state.contains("creation of resampler failed"), "{state}");
    assert!(state.contains(" failed "), "{state}");
    pair.send(mono, 3.0);
}

#[test]
fn the_filters_match() {
    for filter_mode in [1, 2] {
        let pair = Pair::new(Init {
            filter_mode,
            ..Init::default()
        });
        pair.send(Packet::plain(), 1.0);
        let state = pair.rust_state();
        assert_eq!(state.contains("output frames"), filter_mode == 2, "{state}");
    }
}

#[test]
fn the_balance_and_force_mono_match() {
    let pair = Pair::new(Init {
        balance: 0.25,
        flags: FORCE_MONO,
        ..Init::default()
    });
    pair.send(Packet::plain(), 1.0);
    let centered = Packet {
        timestamp: NOW + 21 * MSEC,
        ..Packet::plain()
    };
    pair.set_balance(0.5);
    pair.send(centered, 2.0);
}

#[test]
fn a_mono_output_matches() {
    let mut pair = Pair::new(Init {
        balance: 0.25,
        flags: FORCE_MONO,
        ..Init::default()
    });
    pair.speakers = SPEAKERS_MONO;
    let packet = Packet {
        speakers: SPEAKERS_MONO,
        ..Packet::plain()
    };
    pair.send(packet, 1.0);
}

#[test]
fn a_packet_with_no_sample_rate_matches() {
    let pair = Pair::new(Init::default());
    pair.send(
        Packet {
            rate: 0,
            ..Packet::plain()
        },
        1.0,
    );
}

fn packet() -> impl Strategy<Value = Packet> {
    (
        prop_oneof![Just(FLOAT_PLANAR), Just(FLOAT)],
        prop_oneof![Just(SPEAKERS_STEREO), Just(SPEAKERS_MONO)],
        prop_oneof![4 => Just(RATE), 1 => Just(44_100u32), 1 => Just(0u32)],
        0u32..2048,
        prop_oneof![Just(NOW), (NOW..NOW + 200 * MSEC), any::<u64>(),],
    )
        .prop_map(|(format, speakers, rate, frames, timestamp)| Packet {
            format,
            speakers,
            rate,
            frames,
            timestamp,
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(64)))]

    #[test]
    fn any_sequence_matches_the_c(
        filter_mode in 0i32..3,
        flags in prop_oneof![Just(0u32), Just(FORCE_MONO)],
        create_fails in any::<bool>(),
        mono_output in any::<bool>(),
        steps in prop::collection::vec(
            (packet(), any::<bool>(), prop_oneof![Just(0.5f32), 0.0f32..=1.0]),
            1..16,
        ),
    ) {
        let mut pair = Pair::new(Init {
            filter_mode,
            flags,
            create_fails,
            ..Init::default()
        });
        if mono_output {
            pair.speakers = SPEAKERS_MONO;
        }
        for (index, (packet, compensation, balance)) in steps.into_iter().enumerate() {
            pair.set_compensation(compensation);
            pair.set_balance(balance);
            pair.send(packet, f32::from(u16::try_from(index).unwrap()));
        }
    }
}
