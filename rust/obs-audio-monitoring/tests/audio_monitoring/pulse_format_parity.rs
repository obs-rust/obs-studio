//! Tier 2 layout and Tier 3 parity: the shims against the original
//! `pulseaudio-monitor-format.c.h` (oracle/pulse_format.c).
//!
//! The unchanged C tests `test_pulse_monitor_format` and
//! `test_pulse_default_id` compile the C into themselves, so they never see
//! the Rust swap; this file is what compares the two. The real-stream
//! tests (`test_pulse_monitor_device`, `test_pulse_two_sources`, ...) run
//! the swapped functions inside libobs.
//!
//! Every output byte is compared: the whole sample buffer with its guard
//! samples, the channel count and the eight positions of the channel map,
//! both halves of the drop split.
//!
//! Intentional differences, excluded here and nowhere else:
//! - `pulseaudio_channel_map`: `map[8..]` is indeterminate in C (never
//!   written); Rust fills it with `PA_CHANNEL_POSITION_INVALID`. Only
//!   `map[0..8]` is compared.
//! - A null `resample_data` (or `resample_data[0]`) with samples to process
//!   does nothing in Rust; C dereferences it.
//! - `pulseaudio_monitor_drop_split` with `bytes_per_frame == 0` drops
//!   nothing in Rust; C divides by zero.
//! - `pulseaudio_monitor_devices_match` with an id `"default"` and a null
//!   `default_sink` is no match in Rust; C dereferences it.
//!
//! Not compared because the C is undefined there (documented, not
//! differences): a NaN volume on the integer formats (C converts NaN to an
//! integer; x86-64 gives 0, 0 and INT32_MIN, Rust gives 0), and a signed
//! overflow of the `int64_t` index arithmetic in
//! `pulseaudio_monitor_queued_bytes` (Rust wraps).
//!
//! `<pulse/sample.h>` and `<pulse/channelmap.h>` are libpulse's own where
//! installed (the values are then libpulse's), else the oracle's stand-ins.

use std::ffi::{CString, c_char};
use std::mem::{align_of, offset_of, size_of};

use obs_audio_monitoring::ffi::pulse_format::{
    pa_channel_map, pulseaudio_channel_map, pulseaudio_channels_to_obs_speakers,
    pulseaudio_monitor_devices_match, pulseaudio_monitor_drop, pulseaudio_monitor_drop_split,
    pulseaudio_monitor_process_volume, pulseaudio_monitor_queued_bytes,
    pulseaudio_monitor_sink_channels, pulseaudio_monitor_sink_format,
    pulseaudio_to_obs_audio_format, uint_fast8_t, uint_fast32_t,
};
use obs_audio_monitoring::pulse_format::{
    AUDIO_FORMAT_16BIT, AUDIO_FORMAT_32BIT, AUDIO_FORMAT_FLOAT, AUDIO_FORMAT_U8BIT,
    AUDIO_FORMAT_UNKNOWN, CHANNELS_MAX, POSITION_FRONT_CENTER, POSITION_FRONT_LEFT,
    POSITION_FRONT_RIGHT, POSITION_INVALID, POSITION_LFE, POSITION_MAX, POSITION_MONO,
    POSITION_REAR_CENTER, POSITION_REAR_LEFT, POSITION_REAR_RIGHT, POSITION_SIDE_LEFT,
    POSITION_SIDE_RIGHT, SAMPLE_ALAW, SAMPLE_FLOAT32BE, SAMPLE_FLOAT32LE, SAMPLE_INVALID,
    SAMPLE_MAX, SAMPLE_S16BE, SAMPLE_S16LE, SAMPLE_S24_32BE, SAMPLE_S24_32LE, SAMPLE_S24BE,
    SAMPLE_S24LE, SAMPLE_S32BE, SAMPLE_S32LE, SAMPLE_U8, SAMPLE_ULAW, SPEAKERS_2POINT1,
    SPEAKERS_4POINT0, SPEAKERS_4POINT1, SPEAKERS_5POINT1, SPEAKERS_7POINT1, SPEAKERS_MONO,
    SPEAKERS_STEREO, SPEAKERS_UNKNOWN,
};
use obs_c_oracle::pulse_format::{
    OracleChannelMap, OracleDrop, VALUE_NAMES, oracle_pulse_channels_to_speakers,
    oracle_pulse_format_headers_real, oracle_pulse_format_layout, oracle_pulse_format_value,
    oracle_pulse_process_volume, oracle_pulseaudio_channel_map,
    oracle_pulseaudio_monitor_devices_match, oracle_pulseaudio_monitor_drop_split,
    oracle_pulseaudio_monitor_queued_bytes, oracle_pulseaudio_monitor_sink_channels,
    oracle_pulseaudio_monitor_sink_format, oracle_pulseaudio_to_obs_audio_format,
};
use proptest::prelude::*;

#[test]
fn enum_values_match_oracle() {
    let rust: [i64; 41] = [
        SAMPLE_U8.into(),
        SAMPLE_ALAW.into(),
        SAMPLE_ULAW.into(),
        SAMPLE_S16LE.into(),
        SAMPLE_S16BE.into(),
        SAMPLE_FLOAT32LE.into(),
        SAMPLE_FLOAT32BE.into(),
        SAMPLE_S32LE.into(),
        SAMPLE_S32BE.into(),
        SAMPLE_S24LE.into(),
        SAMPLE_S24BE.into(),
        SAMPLE_S24_32LE.into(),
        SAMPLE_S24_32BE.into(),
        SAMPLE_MAX.into(),
        SAMPLE_INVALID.into(),
        CHANNELS_MAX as i64,
        POSITION_INVALID.into(),
        POSITION_MONO.into(),
        POSITION_FRONT_LEFT.into(),
        POSITION_FRONT_RIGHT.into(),
        POSITION_FRONT_CENTER.into(),
        POSITION_REAR_CENTER.into(),
        POSITION_REAR_LEFT.into(),
        POSITION_REAR_RIGHT.into(),
        POSITION_LFE.into(),
        POSITION_SIDE_LEFT.into(),
        POSITION_SIDE_RIGHT.into(),
        POSITION_MAX.into(),
        AUDIO_FORMAT_UNKNOWN.into(),
        AUDIO_FORMAT_U8BIT.into(),
        AUDIO_FORMAT_16BIT.into(),
        AUDIO_FORMAT_32BIT.into(),
        AUDIO_FORMAT_FLOAT.into(),
        SPEAKERS_UNKNOWN.into(),
        SPEAKERS_MONO.into(),
        SPEAKERS_STEREO.into(),
        SPEAKERS_2POINT1.into(),
        SPEAKERS_4POINT0.into(),
        SPEAKERS_4POINT1.into(),
        SPEAKERS_5POINT1.into(),
        SPEAKERS_7POINT1.into(),
    ];
    // SAFETY: plain oracle getter.
    let real = unsafe { oracle_pulse_format_headers_real() } != 0;
    for (i, (name, value)) in VALUE_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: `i` is below the table length; out of range returns
        // INT64_MIN.
        let c = unsafe { oracle_pulse_format_value(i) };
        assert_eq!(
            value, c,
            "{name}: Rust {value}, C {c} (libpulse headers: {real})"
        );
    }
}

#[test]
fn layouts_match_c() {
    // SAFETY: plain oracle getter.
    let c = |which| unsafe { oracle_pulse_format_layout(which) };
    assert_eq!(size_of::<pa_channel_map>(), c(0));
    assert_eq!(align_of::<pa_channel_map>(), c(1));
    assert_eq!(offset_of!(pa_channel_map, channels), c(2));
    assert_eq!(offset_of!(pa_channel_map, map), c(3));
    assert_eq!(size_of::<OracleChannelMap>(), c(0));
    assert_eq!(size_of::<pulseaudio_monitor_drop>(), c(4));
    assert_eq!(align_of::<pulseaudio_monitor_drop>(), c(5));
    assert_eq!(offset_of!(pulseaudio_monitor_drop, unwritten), c(6));
    assert_eq!(offset_of!(pulseaudio_monitor_drop, rewind), c(7));
    assert_eq!(size_of::<OracleDrop>(), c(4));
    assert_eq!(size_of::<uint_fast32_t>(), c(8), "uint_fast32_t");
    assert_eq!(size_of::<uint_fast8_t>(), c(9), "uint_fast8_t");
}

fn check_format(format: i32) {
    // SAFETY: plain values.
    let (c_obs, c_sink) = unsafe {
        (
            oracle_pulseaudio_to_obs_audio_format(format),
            oracle_pulseaudio_monitor_sink_format(format),
        )
    };
    assert_eq!(
        pulseaudio_to_obs_audio_format(format),
        c_obs,
        "format {format}"
    );
    assert_eq!(
        pulseaudio_monitor_sink_format(format),
        c_sink,
        "format {format}"
    );
}

fn check_channels(channels: u64) {
    // A C caller's conversion to uint_fast32_t, on both sides.
    let rust = pulseaudio_channels_to_obs_speakers(channels as uint_fast32_t);
    // SAFETY: plain values.
    let c = unsafe { oracle_pulse_channels_to_speakers(channels) };
    assert_eq!(rust, c, "{channels} channels");
}

fn check_sink_channels(channels: u8) {
    // SAFETY: plain values.
    let c = unsafe { oracle_pulseaudio_monitor_sink_channels(channels) };
    assert_eq!(
        pulseaudio_monitor_sink_channels(channels),
        c,
        "{channels} channels"
    );
}

fn check_layout(layout: u32) {
    let rust = pulseaudio_channel_map(layout);
    // SAFETY: plain values.
    let c = unsafe { oracle_pulseaudio_channel_map(layout) };
    assert_eq!(rust.channels, c.channels, "layout {layout}");
    assert_eq!(rust.map[..8], c.map[..8], "layout {layout}");
    assert!(
        rust.map[8..].iter().all(|&p| p == POSITION_INVALID),
        "layout {layout}"
    );
}

/// Guard samples on each side of the samples, as the C test's `apply()`.
const GUARD: usize = 2;

fn sample_bytes(format: i32) -> usize {
    match format {
        SAMPLE_U8 => 1,
        SAMPLE_S16LE | SAMPLE_S16BE => 2,
        SAMPLE_S24LE | SAMPLE_S24BE => 3,
        _ => 4,
    }
}

/// Runs the volume on both sides over the same bytes (samples plus guards)
/// and compares every byte.
fn check_volume(format: i32, channels: u8, frames: u32, vol: f32, fill: &[u8]) {
    let size = sample_bytes(format);
    let count = frames as usize * usize::from(channels);
    let len = (count + 2 * GUARD) * size;
    let start: Vec<u8> = (0..len)
        .map(|i| fill.get(i % fill.len().max(1)).copied().unwrap_or(0xA5))
        .collect();

    let mut rust = start.clone();
    let mut c = start.clone();
    let rust_data: *mut u8 = rust[GUARD * size..].as_mut_ptr();
    let c_data: *mut u8 = c[GUARD * size..].as_mut_ptr();
    // SAFETY: each plane pointer points to `count` samples of `size` bytes,
    // with GUARD samples to spare on each side; the buffers are separate.
    unsafe {
        pulseaudio_monitor_process_volume(
            format,
            channels as uint_fast8_t,
            vol,
            &raw const rust_data,
            frames,
        );
        oracle_pulse_process_volume(format, u64::from(channels), vol, &raw const c_data, frames);
    }
    if rust != c {
        let first = rust.iter().zip(&c).position(|(a, b)| a != b);
        panic!(
            "format {format}, {channels} channels, {frames} frames, vol {vol} ({:#x}): \
             first difference at byte {first:?}\n rust {rust:02x?}\n    c {c:02x?}\n   in {start:02x?}",
            vol.to_bits()
        );
    }
}

fn check_queued(write_index: i64, read_index: i64, played_usec: u64, bps: u64, unwritten: usize) {
    let rust =
        pulseaudio_monitor_queued_bytes(write_index, read_index, played_usec, bps, unwritten);
    // SAFETY: plain values.
    let c = unsafe {
        oracle_pulseaudio_monitor_queued_bytes(write_index, read_index, played_usec, bps, unwritten)
    };
    assert_eq!(
        rust, c,
        "queued({write_index}, {read_index}, {played_usec}, {bps}, {unwritten})"
    );
}

fn check_drop(excess_usec: u64, bps: u64, frame: usize, unwritten: usize) {
    let rust = pulseaudio_monitor_drop_split(excess_usec, bps, frame, unwritten);
    // SAFETY: plain values; `frame` is not 0.
    let c = unsafe { oracle_pulseaudio_monitor_drop_split(excess_usec, bps, frame, unwritten) };
    assert_eq!(
        (rust.unwritten, rust.rewind),
        (c.unwritten, c.rewind),
        "drop({excess_usec}, {bps}, {frame}, {unwritten})"
    );
}

fn check_match(id1: Option<&[u8]>, id2: Option<&[u8]>, default_sink: Option<&[u8]>) {
    let owned = |id: Option<&[u8]>| id.map(|b| CString::new(b).expect("no interior NUL"));
    let (o1, o2, od) = (owned(id1), owned(id2), owned(default_sink));
    let ptr = |s: &Option<CString>| s.as_ref().map_or(std::ptr::null(), |s| s.as_ptr());
    let (p1, p2, pd): (*const c_char, *const c_char, *const c_char) =
        (ptr(&o1), ptr(&o2), ptr(&od));
    // SAFETY: null or live NUL-terminated strings; `default_sink` is not
    // null where an id is "default" (the C would dereference it).
    let (rust, c) = unsafe {
        (
            pulseaudio_monitor_devices_match(p1, p2, pd),
            oracle_pulseaudio_monitor_devices_match(p1, p2, pd),
        )
    };
    let show = |id: Option<&[u8]>| id.map(|b| String::from_utf8_lossy(b).into_owned());
    assert_eq!(
        rust,
        c,
        "devices_match({:?}, {:?}) with default {:?}",
        show(id1),
        show(id2),
        show(default_sink)
    );
}

/// The exact quotient of `num * mul / 1000000` fits in 64 bits, so
/// `util_mul_div64` is defined on every host (MSVC x64's `_udiv128` faults
/// past that).
fn mul_div_fits(num: u64, mul: u64) -> bool {
    u128::from(num) * u128::from(mul) / 1_000_000 <= u128::from(u64::MAX)
}

const ALL_FORMATS: [i32; 14] = [
    SAMPLE_U8,
    SAMPLE_ALAW,
    SAMPLE_ULAW,
    SAMPLE_S16LE,
    SAMPLE_S16BE,
    SAMPLE_FLOAT32LE,
    SAMPLE_FLOAT32BE,
    SAMPLE_S32LE,
    SAMPLE_S32BE,
    SAMPLE_S24LE,
    SAMPLE_S24BE,
    SAMPLE_S24_32LE,
    SAMPLE_S24_32BE,
    SAMPLE_INVALID,
];

const PLUS_26_DB: f32 = 19.952_623;

/// Every input of `test_pulse_monitor_format`, plus the edge cases of the
/// Tier 1 tests.
#[test]
fn cmocka_cases_match_oracle() {
    for format in ALL_FORMATS
        .into_iter()
        .chain([SAMPLE_MAX, 14, -2, i32::MIN, i32::MAX])
    {
        check_format(format);
    }
    for channels in (0..=300).chain([u64::from(u32::MAX), 1 << 32, (1 << 32) | 2, u64::MAX]) {
        check_channels(channels);
    }
    for channels in 0..=u8::MAX {
        check_sink_channels(channels);
    }
    for layout in (0..=40).chain([255, 0x8000_0000, u32::MAX]) {
        check_layout(layout);
    }

    let vols = [
        0.0,
        0.1,
        0.25,
        0.5,
        0.9999,
        0.999_999_94,
        1.0,
        1.000_000_1,
        1.5,
        1.999_999_9,
        2.0,
        4.0,
        PLUS_26_DB,
        20.0,
    ];
    let u8s: Vec<u8> = (0..=u8::MAX).collect();
    let s16s: Vec<u8> = [
        0i16, 1, -1, 3, -3, 100, -100, 1642, 1643, -1643, 12345, -12345,
    ]
    .into_iter()
    .chain([
        16383, 16384, -16384, -16385, 20000, -20000, 32766, 32767, -32768,
    ])
    .flat_map(i16::to_ne_bytes)
    .collect();
    let s32s: Vec<u8> = [
        0i32,
        1,
        -1,
        3,
        -3,
        1 << 24,
        -(1 << 24),
        16_777_217,
        123_456_789,
    ]
    .into_iter()
    .chain([
        -123_456_789,
        1 << 30,
        -(1 << 30),
        (1 << 30) + 128,
        -(1 << 30) - 128,
    ])
    .chain([
        1_000_000_000,
        -1_000_000_000,
        2_147_483_520,
        i32::MIN,
        i32::MAX,
    ])
    .flat_map(i32::to_ne_bytes)
    .collect();
    let f32s: Vec<u8> = [
        0.0f32,
        -0.0,
        1.0,
        -1.0,
        0.5,
        -0.25,
        0.75,
        1.5,
        0.3,
        f32::MAX,
    ]
    .into_iter()
    .chain([f32::MIN_POSITIVE, f32::INFINITY, f32::NEG_INFINITY])
    .flat_map(f32::to_ne_bytes)
    .collect();
    for vol in vols {
        for channels in 1..=8u8 {
            for frames in [0, 1, 2, 3, 7, 32] {
                check_volume(SAMPLE_U8, channels, frames, vol, &u8s);
                check_volume(SAMPLE_S16LE, channels, frames, vol, &s16s);
                check_volume(SAMPLE_S32LE, channels, frames, vol, &s32s);
                check_volume(SAMPLE_FLOAT32LE, channels, frames, vol, &f32s);
            }
        }
        for format in ALL_FORMATS {
            check_volume(format, 2, 6, vol, &u8s);
        }
    }

    let bps = 48_000 * 4;
    let big = 192_000i64 * 32 * 3600 * 10;
    for (w, r, played, bps, unwritten) in [
        (1000, 400, 0, bps, 0),
        (1000, 400, 0, bps, 50),
        (1000, 400, 1000, bps, 0),
        (1000, 400, 1000, bps, 7),
        (1000, 400, 1, bps, 0),
        (1000, 400, 6, bps, 0),
        (1000, 400, 5000, bps, 0),
        (1000, 400, 5000, bps, 30),
        (400, 1000, 0, bps, 30),
        (1000, 1000, 0, bps, 0),
        (1192, 1000, 1000, bps, 3),
        (big + 6_144_000, big, 1_000_000, 6_144_000, 0),
        (big + 6_144_000, big, 500_000, 6_144_000, 0),
        (1, 0, 0, 0, usize::MAX),
        (i64::MAX, 0, 0, 0, 1),
        (0, i64::MAX, 0, 0, 1),
        (1000, 0, u64::MAX, 0, 0),
        (1 << 40, 0, 1 << 40, 1 << 20, 0),
    ] {
        check_queued(w, r, played, bps, unwritten);
    }

    for (usec, bps, frame, unwritten) in [
        (1000, 192_000, 4, 1000),
        (1000, 192_000, 4, 100),
        (1000, 192_000, 4, 0),
        (1000, 192_000, 4, 192),
        (0, 192_000, 4, 500),
        (10, 1_152_000, 24, 100),
        (1_000_000, 1_152_000, 24, 100),
        (1000, 192_000, 193, 100),
        (1000, 192_000, usize::MAX, 100),
        (u64::MAX, 0, 4, 100),
        (u64::MAX, 1, 3, usize::MAX),
        (u64::MAX, 999_999, 7, 5),
    ] {
        check_drop(usec, bps, frame, unwritten);
    }
    for frame in 1..=32usize {
        for usec in 0..4 * frame as u64 {
            for unwritten in [usize::MAX, 0, frame + 1] {
                check_drop(usec, 1_000_000, frame, unwritten);
            }
        }
    }

    let s = |v: &'static str| Some(v.as_bytes());
    let long = "x".repeat(599);
    let long_monitor = format!("{long}.monitor");
    type Ids<'a> = (Option<&'a [u8]>, Option<&'a [u8]>, Option<&'a [u8]>);
    let cases: Vec<Ids> = vec![
        (s("sinkA.monitor"), s("sinkA"), None),
        (s("sinkA"), s("sinkA"), None),
        (s("sinkB.monitor"), s("sinkA"), None),
        (s("sinkA.monitor"), s("sinkA.monitor"), None),
        (s("sinkA.monitor.monitor"), s("sinkA.monitor"), None),
        (s("SinkA.monitor"), s("sinkA"), None),
        (s(""), s(""), None),
        (s(".monitor"), s(""), None),
        (s("default"), s("default"), s("sinkA")),
        (s("default"), s("sinkA"), s("sinkA")),
        (s("default"), s("sinkB"), s("sinkA")),
        (s("sinkA.monitor"), s("default"), s("sinkA")),
        (s("sinkB.monitor"), s("default"), s("sinkA")),
        (s("sinkA"), s("default"), s("sinkA")),
        (s("Default"), s("sinkA"), s("sinkA")),
        (s("sinkA.monitor"), s("Default"), s("sinkA")),
        (s("default"), s("default"), s("")),
        (s(".monitor"), s("default"), s("")),
        (s("default"), s(""), s("")),
        (s("default"), s("sinkA"), s("")),
        (s("sinkA.monitor"), s("default"), s("")),
        (s("default"), s("sinkA.monitor"), s("sinkA.monitor")),
        (s("sinkA.monitor.monitor"), s("default"), s("sinkA.monitor")),
        (None, s("sinkA"), s("sinkA")),
        (s("sinkA.monitor"), None, s("sinkA")),
        (None, None, None),
        (None, s("default"), None),
        (s("default"), None, None),
        (s("defaul"), s("sinkA"), s("sinkA")),
        (s("sinkA.monito"), s("sinkA"), None),
        (s("default"), s("default"), s("default")),
        (s("default.monitor"), s("default"), s("default")),
        (Some(b"\xff\xfe.monitor"), Some(b"\xff\xfe"), None),
        (Some(long_monitor.as_bytes()), Some(long.as_bytes()), None),
        (
            Some(long_monitor.as_bytes()),
            s("default"),
            Some(long.as_bytes()),
        ),
        (s("default"), Some(long.as_bytes()), Some(long.as_bytes())),
        (Some(long.as_bytes()), Some(long.as_bytes()), None),
    ];
    for (id1, id2, default_sink) in cases {
        check_match(id1, id2, default_sink);
    }
}

/// NaN volumes, for the record only: the integer formats convert a NaN
/// product, which is undefined in C, so only Rust's result (0) is
/// asserted. FLOAT32LE multiplies, which is defined: there both sides give
/// NaN.
#[test]
fn nan_volume_documented() {
    for (format, sample) in [
        (SAMPLE_U8, vec![200u8]),
        (SAMPLE_S16LE, 1000i16.to_ne_bytes().to_vec()),
        (SAMPLE_S32LE, 1000i32.to_ne_bytes().to_vec()),
    ] {
        let mut rust = sample.clone();
        let mut c = sample.clone();
        let (rp, cp): (*mut u8, *mut u8) = (rust.as_mut_ptr(), c.as_mut_ptr());
        // SAFETY: one sample of the format in each buffer.
        unsafe {
            pulseaudio_monitor_process_volume(format, 1, f32::NAN, &raw const rp, 1);
            oracle_pulse_process_volume(format, 1, f32::NAN, &raw const cp, 1);
        }
        assert!(rust.iter().all(|&b| b == 0), "format {format}: {rust:02x?}");
        eprintln!("NaN volume, format {format}: Rust {rust:02x?}, C {c:02x?} (undefined in C)");
    }

    let mut rust = 0.5f32.to_ne_bytes();
    let mut c = rust;
    let (rp, cp): (*mut u8, *mut u8) = (rust.as_mut_ptr(), c.as_mut_ptr());
    // SAFETY: one f32 sample in each buffer.
    unsafe {
        pulseaudio_monitor_process_volume(SAMPLE_FLOAT32LE, 1, f32::NAN, &raw const rp, 1);
        oracle_pulse_process_volume(SAMPLE_FLOAT32LE, 1, f32::NAN, &raw const cp, 1);
    }
    assert!(f32::from_ne_bytes(rust).is_nan());
    assert!(f32::from_ne_bytes(c).is_nan());
}

/// The intentional differences: Rust's side only.
#[test]
fn intentional_differences() {
    let null: *const *mut u8 = std::ptr::null();
    let null_plane: *mut u8 = std::ptr::null_mut();
    // SAFETY: Rust checks both pointers before reading them.
    unsafe {
        pulseaudio_monitor_process_volume(SAMPLE_S16LE, 2, 0.5, null, 16);
        pulseaudio_monitor_process_volume(SAMPLE_S16LE, 2, 0.5, &raw const null_plane, 16);
    }
    let drop = pulseaudio_monitor_drop_split(1000, 192_000, 0, 100);
    assert_eq!((drop.unwritten, drop.rewind), (0, 0));
    let id = CString::new("default").expect("no NUL");
    let sink = CString::new("sinkA").expect("no NUL");
    // SAFETY: NUL-terminated strings and null.
    unsafe {
        assert!(!pulseaudio_monitor_devices_match(
            id.as_ptr(),
            sink.as_ptr(),
            std::ptr::null()
        ));
        assert!(!pulseaudio_monitor_devices_match(
            sink.as_ptr(),
            id.as_ptr(),
            std::ptr::null()
        ));
    }
    for layout in [SPEAKERS_MONO, SPEAKERS_7POINT1, SPEAKERS_UNKNOWN] {
        let map = pulseaudio_channel_map(layout);
        assert!(map.map[8..].iter().all(|&p| p == POSITION_INVALID));
    }
}

fn format() -> impl Strategy<Value = i32> {
    prop_oneof![
        8 => prop::sample::select(vec![SAMPLE_U8, SAMPLE_S16LE, SAMPLE_S32LE, SAMPLE_FLOAT32LE]),
        2 => prop::sample::select(ALL_FORMATS.to_vec()),
        1 => -3..=20i32,
        1 => any::<i32>(),
    ]
}

/// Volumes 0 to 20, weighted toward the edges: 1 and just around it, the
/// powers of two, +26 dB, and values just past each.
fn volume() -> impl Strategy<Value = f32> {
    let special = prop::sample::select(vec![
        0.0f32,
        f32::from_bits(1),
        0.1,
        0.5,
        0.999_999_94,
        1.0,
        1.000_000_1,
        2.0,
        4.0,
        PLUS_26_DB,
        20.0,
    ]);
    prop_oneof![
        4 => 0.0f32..=20.0,
        2 => 0.0f32..=1.0,
        2 => special,
        // A few ulp around a special value.
        1 => (prop::sample::select(vec![0.5f32, 1.0, 2.0, 4.0, PLUS_26_DB]), -4i32..=4)
            .prop_map(|(v, d)| f32::from_bits(v.to_bits().wrapping_add_signed(d))),
    ]
}

/// Sample bytes: any bytes, or the format's loud and quiet values.
fn fill() -> impl Strategy<Value = Vec<u8>> {
    let loud_s16 = prop::sample::select(vec![
        i16::MAX,
        i16::MIN,
        16384,
        -16385,
        1642,
        1643,
        -1643,
        0,
        1,
        -1,
    ])
    .prop_map(|v| v.to_ne_bytes().to_vec());
    let loud_s32 = prop::sample::select(vec![
        i32::MAX,
        i32::MIN,
        1 << 30,
        -(1 << 30),
        (1 << 30) + 128,
        2_147_483_520,
        123_456_789,
        0,
        1,
        -1,
    ])
    .prop_map(|v| v.to_ne_bytes().to_vec());
    let floats = prop_oneof![-2.0f32..=2.0, any::<f32>()].prop_map(|v| v.to_ne_bytes().to_vec());
    prop_oneof![
        4 => prop::collection::vec(any::<u8>(), 1..=256),
        1 => prop::collection::vec(loud_s16, 1..=32).prop_map(|v| v.concat()),
        1 => prop::collection::vec(loud_s32, 1..=32).prop_map(|v| v.concat()),
        1 => prop::collection::vec(floats, 1..=32).prop_map(|v| v.concat()),
    ]
}

fn channels_count() -> impl Strategy<Value = u64> {
    prop_oneof![
        4 => 0..=300u64,
        1 => any::<u32>().prop_map(u64::from),
        1 => any::<u64>(),
    ]
}

/// A device id: a short name from a small alphabet (so ids collide), with
/// or without `.monitor` (once or twice), `"default"`, a near miss of it,
/// or any bytes.
fn device_name() -> impl Strategy<Value = Vec<u8>> {
    let base = prop::collection::vec(prop::sample::select(b"ab.".to_vec()), 0..=4);
    let suffix = prop::sample::select(vec![&b""[..], b".monitor", b".monitor.monitor", b".m"]);
    prop_oneof![
        4 => (base, suffix).prop_map(|(b, s)| [b.as_slice(), s].concat()),
        2 => prop::sample::select(vec![
            b"default".to_vec(),
            b"Default".to_vec(),
            b"defaul".to_vec(),
            b"default.monitor".to_vec(),
            b"".to_vec(),
            b".monitor".to_vec(),
        ]),
        1 => prop::collection::vec(1..=255u8, 0..=24),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn random_volumes_match_oracle(
        format in format(),
        channels in prop_oneof![8 => 1..=8u8, 1 => Just(0u8), 1 => any::<u8>()],
        frames in prop_oneof![4 => 0..=64u32, 1 => 0..=1024u32],
        vol in volume(),
        fill in fill(),
    ) {
        check_volume(format, channels, frames, vol, &fill);
    }

    #[test]
    fn random_formats_and_maps_match_oracle(
        format in format(),
        channels in channels_count(),
        sink_channels in any::<u8>(),
        layout in prop_oneof![4 => 0..=16u32, 1 => any::<u32>()],
    ) {
        check_format(format);
        check_channels(channels);
        check_sink_channels(sink_channels);
        check_layout(layout);
    }

    /// No signed overflow (undefined in C): indices and the played bytes
    /// stay within 2^62.
    #[test]
    fn random_queued_bytes_match_oracle(
        write_index in prop_oneof![3 => 0..=1i64 << 40, 1 => -(1i64 << 62)..=1 << 62],
        read_index in prop_oneof![3 => 0..=1i64 << 40, 1 => -(1i64 << 62)..=1 << 62],
        played_usec in prop_oneof![3 => 0..=10_000_000u64, 1 => any::<u64>()],
        bps in prop_oneof![
            3 => prop::sample::select(vec![8000u64, 48_000 * 2, 48_000 * 4, 192_000 * 32]),
            1 => any::<u64>(),
        ],
        unwritten in prop_oneof![3 => 0..=1usize << 20, 1 => any::<usize>()],
    ) {
        prop_assume!(mul_div_fits(played_usec, bps));
        let played = u128::from(played_usec) * u128::from(bps) / 1_000_000;
        prop_assume!(played < 1 << 62);
        check_queued(write_index, read_index, played_usec, bps, unwritten);
    }

    #[test]
    fn random_drop_splits_match_oracle(
        excess_usec in prop_oneof![3 => 0..=10_000_000u64, 1 => any::<u64>()],
        bps in prop_oneof![
            3 => prop::sample::select(vec![8000u64, 48_000 * 2, 48_000 * 4, 1_152_000, 192_000 * 32]),
            1 => any::<u64>(),
        ],
        frame in prop_oneof![4 => 1..=64usize, 1 => 1..=usize::MAX],
        unwritten in prop_oneof![3 => 0..=1usize << 20, 1 => any::<usize>()],
    ) {
        prop_assume!(mul_div_fits(excess_usec, bps));
        check_drop(excess_usec, bps, frame, unwritten);
    }

    /// `default_sink` is null only when neither id is `"default"` (the C
    /// would dereference it).
    #[test]
    fn random_device_ids_match_oracle(
        id1 in prop::option::weighted(0.95, device_name()),
        id2 in prop::option::weighted(0.95, device_name()),
        default_sink in prop::option::weighted(0.8, device_name()),
    ) {
        let is_default = |id: &Option<Vec<u8>>| id.as_deref() == Some(b"default");
        let default_sink = if default_sink.is_none() && (is_default(&id1) || is_default(&id2)) {
            Some(Vec::new())
        } else {
            default_sink
        };
        check_match(id1.as_deref(), id2.as_deref(), default_sink.as_deref());
    }
}
