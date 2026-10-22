//! Tier 1 mirror of `test/cmocka/test_pulse_monitor_format.c` (all 12
//! cases, one test each under the C case's name), plus edge cases. The C
//! test compiles `pulseaudio-monitor-format.c.h` into itself, so the Rust
//! swap is invisible to it; parity is in `pulse_format_parity`.
//!
//! The C test's `pa_sample_format_to_string` messages become the format
//! numbers, and its `pa_channel_map_valid` check is written out: a map is
//! valid when it has 1 to `PA_CHANNELS_MAX` channels and each of them is a
//! position in `0..PA_CHANNEL_POSITION_MAX`.

use obs_audio_monitoring::pulse_format::{
    AUDIO_FORMAT_16BIT, AUDIO_FORMAT_32BIT, AUDIO_FORMAT_FLOAT, AUDIO_FORMAT_U8BIT,
    AUDIO_FORMAT_UNKNOWN, CHANNELS_MAX, DropSplit, POSITION_FRONT_CENTER, POSITION_FRONT_LEFT,
    POSITION_FRONT_RIGHT, POSITION_LFE, POSITION_MAX, POSITION_MONO, POSITION_REAR_CENTER,
    POSITION_REAR_LEFT, POSITION_REAR_RIGHT, POSITION_SIDE_LEFT, POSITION_SIDE_RIGHT, SAMPLE_ALAW,
    SAMPLE_FLOAT32BE, SAMPLE_FLOAT32LE, SAMPLE_INVALID, SAMPLE_MAX, SAMPLE_S16BE, SAMPLE_S16LE,
    SAMPLE_S24_32BE, SAMPLE_S24_32LE, SAMPLE_S24BE, SAMPLE_S24LE, SAMPLE_S32BE, SAMPLE_S32LE,
    SAMPLE_U8, SAMPLE_ULAW, SPEAKERS_2POINT1, SPEAKERS_4POINT0, SPEAKERS_4POINT1, SPEAKERS_5POINT1,
    SPEAKERS_7POINT1, SPEAKERS_MONO, SPEAKERS_STEREO, SPEAKERS_UNKNOWN, channel_map,
    channels_to_speakers, devices_match, drop_split, process_volume, queued_bytes, sample_size,
    scale_f32, scale_s16, scale_s32, scale_u8, sink_channels, sink_format, to_obs_audio_format,
};

/// `all_formats` in the C test.
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

/// +26 dB, the most Advanced Audio Properties sets.
const PLUS_26_DB: f32 = 19.952_623;

fn expected_obs_format(format: i32) -> u32 {
    match format {
        SAMPLE_U8 => AUDIO_FORMAT_U8BIT,
        SAMPLE_S16LE => AUDIO_FORMAT_16BIT,
        SAMPLE_S32LE => AUDIO_FORMAT_32BIT,
        SAMPLE_FLOAT32LE => AUDIO_FORMAT_FLOAT,
        _ => AUDIO_FORMAT_UNKNOWN,
    }
}

#[test]
fn sample_formats() {
    // The pa_sample_format_t numbers are libpulse ABI.
    assert_eq!(SAMPLE_U8, 0);
    assert_eq!(SAMPLE_S16LE, 3);
    assert_eq!(SAMPLE_FLOAT32LE, 5);
    assert_eq!(SAMPLE_S32LE, 7);
    assert_eq!(SAMPLE_S24_32BE, 12);
    assert_eq!(SAMPLE_MAX, 13);
    assert_eq!(SAMPLE_INVALID, -1);
    assert_eq!(ALL_FORMATS.len() as i32, SAMPLE_MAX + 1);

    for format in ALL_FORMATS {
        let obs_format = to_obs_audio_format(format);
        assert_eq!(
            obs_format,
            expected_obs_format(format),
            "format {format} maps to audio_format {obs_format}"
        );

        // Big-endian, 24-bit, a-law and mu-law sinks (and an invalid spec)
        // are played as FLOAT32LE.
        let played = sink_format(format);
        let expected = if obs_format == AUDIO_FORMAT_UNKNOWN {
            SAMPLE_FLOAT32LE
        } else {
            format
        };
        assert_eq!(
            played, expected,
            "a format {format} sink is played as {played}"
        );
    }

    assert_eq!(to_obs_audio_format(SAMPLE_MAX), AUDIO_FORMAT_UNKNOWN);
    assert_eq!(sink_format(SAMPLE_MAX), SAMPLE_FLOAT32LE);
}

#[test]
fn channel_counts() {
    let expected = [
        SPEAKERS_UNKNOWN,
        SPEAKERS_MONO,
        SPEAKERS_STEREO,
        SPEAKERS_2POINT1,
        SPEAKERS_4POINT0,
        SPEAKERS_4POINT1,
        SPEAKERS_5POINT1,
        SPEAKERS_UNKNOWN,
        SPEAKERS_7POINT1,
        SPEAKERS_UNKNOWN,
    ];
    for (channels, want) in expected.into_iter().enumerate() {
        assert_eq!(
            channels_to_speakers(channels as u64),
            want,
            "{channels} channels"
        );
    }

    let more = [
        10,
        16,
        32,
        CHANNELS_MAX as u64,
        255,
        256,
        u64::from(u32::MAX),
    ];
    for channels in more {
        assert_eq!(
            channels_to_speakers(channels),
            SPEAKERS_UNKNOWN,
            "{channels} channels"
        );
    }

    // 0, 7 and more than 8 channels are played as stereo; every other count
    // as it is.
    for channels in 0..=u8::MAX {
        let played = sink_channels(channels);
        let supported = (1..=6).contains(&channels) || channels == 8;
        assert_eq!(
            played,
            if supported { channels } else { 2 },
            "a {channels}-channel sink is played with {played} channels"
        );
    }
}

const FL: i32 = POSITION_FRONT_LEFT;
const FR: i32 = POSITION_FRONT_RIGHT;
const FC: i32 = POSITION_FRONT_CENTER;
const LFE: i32 = POSITION_LFE;
const RL: i32 = POSITION_REAR_LEFT;
const RR: i32 = POSITION_REAR_RIGHT;
const RC: i32 = POSITION_REAR_CENTER;
const SL: i32 = POSITION_SIDE_LEFT;
const SR: i32 = POSITION_SIDE_RIGHT;
const MONO: i32 = POSITION_MONO;

/// `layout_maps` in the C test: map[] past channels is filled too (the
/// default order); it is pinned so the port returns the same bytes.
const LAYOUT_MAPS: [(u32, u8, [i32; 8]); 11] = [
    (SPEAKERS_UNKNOWN, 0, [FL, FR, FC, LFE, RL, RR, SL, SR]),
    (SPEAKERS_MONO, 1, [MONO, FR, FC, LFE, RL, RR, SL, SR]),
    (SPEAKERS_STEREO, 2, [FL, FR, FC, LFE, RL, RR, SL, SR]),
    (SPEAKERS_2POINT1, 3, [FL, FR, LFE, LFE, RL, RR, SL, SR]),
    (SPEAKERS_4POINT0, 4, [FL, FR, FC, RC, RL, RR, SL, SR]),
    (SPEAKERS_4POINT1, 5, [FL, FR, FC, LFE, RC, RR, SL, SR]),
    (SPEAKERS_5POINT1, 6, [FL, FR, FC, LFE, RL, RR, SL, SR]),
    (7, 0, [FL, FR, FC, LFE, RL, RR, SL, SR]),
    (SPEAKERS_7POINT1, 8, [FL, FR, FC, LFE, RL, RR, SL, SR]),
    (9, 0, [FL, FR, FC, LFE, RL, RR, SL, SR]),
    (32, 0, [FL, FR, FC, LFE, RL, RR, SL, SR]),
];

#[test]
fn channel_maps() {
    // The pa_channel_position_t numbers are libpulse ABI.
    assert_eq!(MONO, 0);
    assert_eq!(FL, 1);
    assert_eq!(FR, 2);
    assert_eq!(FC, 3);
    assert_eq!(RC, 4);
    assert_eq!(RL, 5);
    assert_eq!(RR, 6);
    assert_eq!(LFE, 7);
    assert_eq!(SL, 10);
    assert_eq!(SR, 11);

    for (layout, channels, map) in LAYOUT_MAPS {
        let got = channel_map(layout);
        assert_eq!(
            got.channels, channels,
            "layout {layout}: {} channels",
            got.channels
        );
        for (c, (got, want)) in got.map.iter().zip(map).enumerate() {
            assert_eq!(*got, want, "layout {layout}: map[{c}] is {got}, not {want}");
        }
    }

    // Every count the monitor plays gets a valid map of that many channels,
    // and different positions.
    for channels in 1..=8u8 {
        let layout = channels_to_speakers(u64::from(channels));
        if layout == SPEAKERS_UNKNOWN {
            continue;
        }
        let map = channel_map(layout);
        assert_eq!(map.channels, channels);
        let used = &map.map[..usize::from(channels)];
        // pa_channel_map_valid().
        assert!(usize::from(map.channels) <= CHANNELS_MAX);
        assert!(
            used.iter().all(|p| (0..POSITION_MAX).contains(p)),
            "{used:?}"
        );
        for (a, pa) in used.iter().enumerate() {
            for pb in &used[a + 1..] {
                assert_ne!(pa, pb, "{channels} channels: {used:?}");
            }
        }
    }
}

/// The C test's guard samples on each side of the samples.
const GUARD: usize = 2;

/// `apply()` in the C test: runs the volume over `samples` (of
/// `sample_size` bytes each, native order) with a guard sample on each side,
/// and checks the guards: it changes exactly frames * channels samples.
fn apply(format: i32, channels: usize, vol: f32, samples: &mut [u8], sample_size: usize) {
    let count = samples.len() / sample_size;
    assert_eq!(samples.len() % sample_size, 0);
    assert_eq!(count % channels, 0);
    let mut buffer = [0xA5u8; (GUARD * 2 + 64) * 4];
    let start = GUARD * sample_size;
    let end = start + samples.len();
    assert!(end + start <= buffer.len());
    buffer[start..end].copy_from_slice(samples);

    // The slice runs to the end of the buffer: only the frames given may
    // change.
    process_volume(
        format,
        channels,
        count / channels,
        vol,
        &mut buffer[start..],
    );

    assert!(buffer[..start].iter().all(|&b| b == 0xA5), "leading guard");
    assert!(buffer[end..].iter().all(|&b| b == 0xA5), "trailing guard");
    samples.copy_from_slice(&buffer[start..end]);
}

fn apply_u8(channels: usize, vol: f32, samples: &mut [u8]) {
    apply(SAMPLE_U8, channels, vol, samples, 1);
}

fn apply_s16(channels: usize, vol: f32, samples: &mut [i16]) {
    let mut bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_ne_bytes()).collect();
    apply(SAMPLE_S16LE, channels, vol, &mut bytes, 2);
    for (s, b) in samples.iter_mut().zip(bytes.as_chunks::<2>().0) {
        *s = i16::from_ne_bytes(*b);
    }
}

fn apply_s32(channels: usize, vol: f32, samples: &mut [i32]) {
    let mut bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_ne_bytes()).collect();
    apply(SAMPLE_S32LE, channels, vol, &mut bytes, 4);
    for (s, b) in samples.iter_mut().zip(bytes.as_chunks::<4>().0) {
        *s = i32::from_ne_bytes(*b);
    }
}

fn apply_f32(channels: usize, vol: f32, samples: &mut [f32]) {
    let mut bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_ne_bytes()).collect();
    apply(SAMPLE_FLOAT32LE, channels, vol, &mut bytes, 4);
    for (s, b) in samples.iter_mut().zip(bytes.as_chunks::<4>().0) {
        *s = f32::from_ne_bytes(*b);
    }
}

#[test]
fn volume_u8() {
    // ((int)x - 128) * vol + 128, the offset added in float, then truncated.
    let input: [u8; 10] = [0, 1, 64, 127, 128, 129, 192, 254, 255, 100];
    let cases: [(f32, [u8; 10]); 5] = [
        (0.0, [128, 128, 128, 128, 128, 128, 128, 128, 128, 128]),
        (0.5, [64, 64, 96, 127, 128, 128, 160, 191, 191, 114]),
        (0.25, [96, 96, 112, 127, 128, 128, 144, 159, 159, 121]),
        (0.1, [115, 115, 121, 127, 128, 128, 134, 140, 140, 125]),
        (1.0, [0, 1, 64, 127, 128, 129, 192, 254, 255, 100]),
    ];
    for (vol, out) in cases {
        for channels in 1..=2 {
            let mut got = input;
            apply_u8(channels, vol, &mut got);
            assert_eq!(got, out, "u8 * {vol}, {channels} channels");
        }
    }

    // Above 1, samples that stay in range, and samples that leave it
    // saturate to 0 or 255 (bug 3).
    let above: [(f32, u8, u8); 18] = [
        (2.0, 127, 126),
        (2.0, 128, 128),
        (2.0, 129, 130),
        (2.0, 100, 72),
        (2.0, 64, 0),
        (2.0, 191, 254),
        (PLUS_26_DB, 130, 167),
        (PLUS_26_DB, 126, 88),
        (2.0, 192, 255),
        (2.0, 255, 255),
        (2.0, 63, 0),
        (2.0, 0, 0),
        (4.0, 200, 255),
        (4.0, 50, 0),
        (PLUS_26_DB, 255, 255),
        (PLUS_26_DB, 0, 0),
        (PLUS_26_DB, 135, 255),
        (PLUS_26_DB, 121, 0),
    ];
    for (vol, input, out) in above {
        let mut got = [input];
        apply_u8(1, vol, &mut got);
        assert_eq!(got[0], out, "u8 {input} * {vol}");
    }
}

#[test]
fn volume_s16() {
    // x * vol in float, truncated toward zero.
    let input: [i16; 12] = [
        0, 1, -1, 3, -3, 100, -100, 12345, -12345, 32767, -32768, 32766,
    ];
    let cases: [(f32, [i16; 12]); 6] = [
        (0.0, [0; 12]),
        (
            0.5,
            [0, 0, 0, 1, -1, 50, -50, 6172, -6172, 16383, -16384, 16383],
        ),
        (
            0.25,
            [0, 0, 0, 0, 0, 25, -25, 3086, -3086, 8191, -8192, 8191],
        ),
        (
            0.1,
            [0, 0, 0, 0, 0, 10, -10, 1234, -1234, 3276, -3276, 3276],
        ),
        (
            0.9999,
            [0, 0, 0, 2, -2, 99, -99, 12343, -12343, 32763, -32764, 32762],
        ),
        (
            1.0,
            [
                0, 1, -1, 3, -3, 100, -100, 12345, -12345, 32767, -32768, 32766,
            ],
        ),
    ];
    for (vol, out) in cases {
        for channels in 1..=6 {
            if !input.len().is_multiple_of(channels) {
                continue;
            }
            let mut got = input;
            apply_s16(channels, vol, &mut got);
            for ((x, g), w) in input.iter().zip(got).zip(out) {
                assert_eq!(g, w, "s16 {x} * {vol}: {g}, not {w}");
            }
        }
    }

    // Above 1, samples that stay in range, and samples that leave it
    // saturate to INT16_MAX or INT16_MIN (bug 3).
    let above: [(f32, i16, i16); 21] = [
        (2.0, 1000, 2000),
        (2.0, -1000, -2000),
        (2.0, 16383, 32766),
        (2.0, -16384, -32768),
        (1.5, 3, 4),
        (1.5, -3, -4),
        (PLUS_26_DB, 100, 1995),
        (PLUS_26_DB, -100, -1995),
        (PLUS_26_DB, 1642, 32762),
        (2.0, 20000, 32767),
        (2.0, -20000, -32768),
        (2.0, 16384, 32767),
        (2.0, -16385, -32768),
        (2.0, 32767, 32767),
        (2.0, -32768, -32768),
        (4.0, 10000, 32767),
        (4.0, -10000, -32768),
        (PLUS_26_DB, 1643, 32767),
        (PLUS_26_DB, -1643, -32768),
        (PLUS_26_DB, 32767, 32767),
        (PLUS_26_DB, -32768, -32768),
    ];
    for (vol, input, out) in above {
        let mut got = [input];
        apply_s16(1, vol, &mut got);
        assert_eq!(got[0], out, "s16 {input} * {vol}");
    }
}

#[test]
fn volume_s32() {
    // x is converted to float first, so it keeps 24 bits: 123456789 becomes
    // 123456792, and INT32_MAX 2^31, before the multiply.
    let input: [i32; 11] = [
        0,
        1,
        -1,
        3,
        -3,
        1 << 24,
        -(1 << 24),
        123_456_789,
        -123_456_789,
        i32::MIN,
        i32::MAX,
    ];
    let cases: [(f32, [i32; 11]); 3] = [
        (0.0, [0; 11]),
        (
            0.5,
            [
                0,
                0,
                0,
                1,
                -1,
                1 << 23,
                -(1 << 23),
                61_728_396,
                -61_728_396,
                -1_073_741_824,
                1_073_741_824,
            ],
        ),
        (
            0.25,
            [
                0,
                0,
                0,
                0,
                0,
                1 << 22,
                -(1 << 22),
                30_864_198,
                -30_864_198,
                -536_870_912,
                536_870_912,
            ],
        ),
    ];
    for (vol, out) in cases {
        let mut got = input;
        apply_s32(1, vol, &mut got);
        for ((x, g), w) in input.iter().zip(got).zip(out) {
            assert_eq!(g, w, "s32 {x} * {vol}: {g}, not {w}");
        }
    }

    // At 1, the float round trip changes the low bits. INT32_MAX at 1
    // becomes 2^31, out of range: the caller never applies a volume of 1
    // (within EPSILON), so that case is not pinned.
    let mut one = [0, 1, -1, 123_456_789, -123_456_789, i32::MIN, 16_777_217];
    let one_out = [0, 1, -1, 123_456_792, -123_456_792, i32::MIN, 16_777_216];
    apply_s32(1, 1.0, &mut one);
    assert_eq!(one, one_out);

    // Above 1, samples that stay in range, and samples that leave it
    // saturate to INT32_MAX or INT32_MIN (bug 3). 2^30 * 2 is 2^31, one past
    // INT32_MAX; -2^30 * 2 is INT32_MIN itself, in range.
    let above: [(f32, i32, i32); 14] = [
        (2.0, 1 << 20, 1 << 21),
        (2.0, -(1 << 29), -(1 << 30)),
        (PLUS_26_DB, 1000, 19952),
        (PLUS_26_DB, -1000, -19952),
        (2.0, 1 << 30, i32::MAX),
        (2.0, -(1 << 30), i32::MIN),
        (2.0, (1 << 30) + 128, i32::MAX),
        (2.0, -(1 << 30) - 128, i32::MIN),
        (2.0, i32::MAX, i32::MAX),
        (2.0, i32::MIN, i32::MIN),
        (4.0, 1_000_000_000, i32::MAX),
        (4.0, -1_000_000_000, i32::MIN),
        (PLUS_26_DB, 123_456_789, i32::MAX),
        (PLUS_26_DB, -123_456_789, i32::MIN),
    ];
    for (vol, input, out) in above {
        let mut got = [input];
        apply_s32(1, vol, &mut got);
        assert_eq!(got[0], out, "s32 {input} * {vol}");
    }
}

#[test]
fn volume_float() {
    // No clamp: above 1 the samples leave [-1, 1] and the server clips them.
    // That is defined, so it is pinned (bit for bit, as assert_memory_equal).
    let input: [f32; 7] = [0.0, 1.0, -1.0, 0.5, -0.25, 0.75, 1.5];
    let cases: [(f32, [f32; 7]); 4] = [
        (0.0, [0.0, 0.0, -0.0, 0.0, -0.0, 0.0, 0.0]),
        (0.5, [0.0, 0.5, -0.5, 0.25, -0.125, 0.375, 0.75]),
        (1.0, [0.0, 1.0, -1.0, 0.5, -0.25, 0.75, 1.5]),
        (2.0, [0.0, 2.0, -2.0, 1.0, -0.5, 1.5, 3.0]),
    ];
    for (vol, out) in cases {
        let mut got = input;
        apply_f32(1, vol, &mut got);
        assert_eq!(got.map(f32::to_bits), out.map(f32::to_bits), "f32 * {vol}");
    }

    let mut one = [0.3f32];
    apply_f32(1, PLUS_26_DB, &mut one);
    assert_eq!(one[0], 0.3f32 * PLUS_26_DB);
}

#[test]
fn volume_other_formats_untouched() {
    for format in ALL_FORMATS {
        if expected_obs_format(format) != AUDIO_FORMAT_UNKNOWN {
            continue;
        }
        let want: [u8; 24] = std::array::from_fn(|b| (b * 37 + 11) as u8);
        let mut got = want;
        apply(format, 2, 0.5, &mut got, 4);
        assert_eq!(got, want, "format {format}");
    }
}

#[test]
fn volume_frame_count() {
    // frames * channels samples, for every count the monitor plays; the
    // guards around them stay unchanged (apply() checks).
    for channels in 1..=8usize {
        if channels_to_speakers(channels as u64) == SPEAKERS_UNKNOWN {
            continue;
        }
        let mut frames = 0;
        while frames * channels <= 16 {
            let mut s32 = [1000i32; 16];
            let mut s16 = [1000i16; 16];
            let mut f32s = [0.5f32; 16];
            let mut u8s = [200u8; 16];
            let count = frames * channels;
            if count != 0 {
                apply_s32(channels, 0.5, &mut s32[..count]);
                apply_s16(channels, 0.5, &mut s16[..count]);
                apply_f32(channels, 0.5, &mut f32s[..count]);
                apply_u8(channels, 0.5, &mut u8s[..count]);
            }
            for s in 0..16 {
                let done = s < count;
                assert_eq!(s32[s], if done { 500 } else { 1000 });
                assert_eq!(s16[s], if done { 500 } else { 1000 });
                assert_eq!(f32s[s], if done { 0.25 } else { 0.5 });
                assert_eq!(u8s[s], if done { 164 } else { 200 });
            }
            frames += 1;
        }
    }
}

#[test]
fn queued_bytes_cases() {
    let bps: u64 = 48000 * 4; // s16 stereo

    // What the server holds plus what is not written yet.
    assert_eq!(queued_bytes(1000, 400, 0, bps, 0), 600);
    assert_eq!(queued_bytes(1000, 400, 0, bps, 50), 650);
    // Less what it played since the timing update: 1 ms is 192 bytes.
    assert_eq!(queued_bytes(1000, 400, 1000, bps, 0), 408);
    assert_eq!(queued_bytes(1000, 400, 1000, bps, 7), 415);
    // Rounded down: 1 usec is 0.192 bytes, 6 usec 1.152.
    assert_eq!(queued_bytes(1000, 400, 1, bps, 0), 600);
    assert_eq!(queued_bytes(1000, 400, 6, bps, 0), 599);
    // Played past what it holds, or a read index ahead of the write index:
    // nothing held.
    assert_eq!(queued_bytes(1000, 400, 5000, bps, 0), 0);
    assert_eq!(queued_bytes(1000, 400, 5000, bps, 30), 30);
    assert_eq!(queued_bytes(400, 1000, 0, bps, 30), 30);
    assert_eq!(queued_bytes(1000, 1000, 0, bps, 0), 0);
    // Held exactly what was played.
    assert_eq!(queued_bytes(1192, 1000, 1000, bps, 3), 3);
    // Large indices (hours of 7.1 float at 192 kHz).
    let big: i64 = 192_000 * 32 * 3600 * 10;
    assert_eq!(
        queued_bytes(big + 6_144_000, big, 1_000_000, 6_144_000, 0),
        0
    );
    assert_eq!(
        queued_bytes(big + 6_144_000, big, 500_000, 6_144_000, 0),
        3_072_000
    );
}

#[test]
fn drop_split_cases() {
    // 1 ms of s16 stereo at 48 kHz is 192 bytes.
    let split = |unwritten, rewind| DropSplit { unwritten, rewind };
    assert_eq!(drop_split(1000, 192_000, 4, 1000), split(192, 0));
    // Not enough unwritten: the rest is rewound.
    assert_eq!(drop_split(1000, 192_000, 4, 100), split(100, 92));
    assert_eq!(drop_split(1000, 192_000, 4, 0), split(0, 192));
    assert_eq!(drop_split(1000, 192_000, 4, 192), split(192, 0));
    // Nothing to drop.
    assert_eq!(drop_split(0, 192_000, 4, 500), split(0, 0));

    // Rounded down to whole frames, at every remainder: at 1 byte per usec
    // 12k + r usec of 12-byte frames drop 12k bytes.
    for frame in 1..=32usize {
        for usec in 0..4 * frame as u64 {
            let want = usec as usize - usec as usize % frame;
            assert_eq!(
                drop_split(usec, 1_000_000, frame, usize::MAX),
                split(want, 0)
            );
            assert_eq!(drop_split(usec, 1_000_000, frame, 0), split(0, want));
            let drop = drop_split(usec, 1_000_000, frame, frame + 1);
            assert_eq!(drop.unwritten + drop.rewind, want);
            assert_eq!(drop.unwritten, want.min(frame + 1));
        }
    }

    // The byte count is rounded down before the frames: 10 usec of 6
    // channels of s32 at 48 kHz (1.152 bytes per usec) is 11 bytes, less
    // than one 24-byte frame.
    assert_eq!(drop_split(10, 1_152_000, 24, 100), split(0, 0));
    assert_eq!(
        drop_split(1_000_000, 1_152_000, 24, 100),
        split(100, 1_152_000 - 100)
    );
}

#[test]
fn devices_match_rules() {
    type Case = (
        Option<&'static str>,
        Option<&'static str>,
        Option<&'static str>,
        bool,
    );
    let cases: [Case; 28] = [
        // A source (id1) matches the output (id2) when it is that output's
        // .monitor.
        (Some("sinkA.monitor"), Some("sinkA"), None, true),
        (Some("sinkA"), Some("sinkA"), None, false),
        (Some("sinkB.monitor"), Some("sinkA"), None, false),
        (Some("sinkA.monitor"), Some("sinkA.monitor"), None, false),
        (
            Some("sinkA.monitor.monitor"),
            Some("sinkA.monitor"),
            None,
            true,
        ),
        (Some("SinkA.monitor"), Some("sinkA"), None, false),
        (Some(""), Some(""), None, false),
        (Some(".monitor"), Some(""), None, true),
        // "default" stands for the default sink.
        (Some("default"), Some("default"), Some("sinkA"), true),
        (Some("default"), Some("sinkA"), Some("sinkA"), true),
        (Some("default"), Some("sinkB"), Some("sinkA"), false),
        (Some("sinkA.monitor"), Some("default"), Some("sinkA"), true),
        (Some("sinkB.monitor"), Some("default"), Some("sinkA"), false),
        (Some("sinkA"), Some("default"), Some("sinkA"), false),
        (Some("Default"), Some("sinkA"), Some("sinkA"), false),
        (Some("sinkA.monitor"), Some("Default"), Some("sinkA"), false),
        // No default sink ("").
        (Some("default"), Some("default"), Some(""), true),
        (Some(".monitor"), Some("default"), Some(""), true),
        (Some("default"), Some(""), Some(""), true),
        (Some("default"), Some("sinkA"), Some(""), false),
        (Some("sinkA.monitor"), Some("default"), Some(""), false),
        // A default sink that is itself a monitor name.
        (
            Some("default"),
            Some("sinkA.monitor"),
            Some("sinkA.monitor"),
            true,
        ),
        (
            Some("sinkA.monitor.monitor"),
            Some("default"),
            Some("sinkA.monitor"),
            true,
        ),
        // NULL never matches.
        (None, Some("sinkA"), Some("sinkA"), false),
        (Some("sinkA.monitor"), None, Some("sinkA"), false),
        (None, None, None, false),
        (None, Some("default"), None, false),
        (Some("default"), None, None, false),
    ];
    for (id1, id2, default_sink, want) in cases {
        let got = devices_match(
            id1.map(str::as_bytes),
            id2.map(str::as_bytes),
            default_sink.map(str::as_bytes),
        );
        assert_eq!(
            got, want,
            "devices_match({id1:?}, {id2:?}) with default {default_sink:?}"
        );
    }

    // A long name.
    let id = "x".repeat(599);
    let monitor = format!("{id}.monitor");
    let (id, monitor) = (id.as_bytes(), monitor.as_bytes());
    assert!(devices_match(Some(monitor), Some(id), None));
    assert!(devices_match(Some(monitor), Some(b"default"), Some(id)));
    assert!(devices_match(Some(b"default"), Some(id), Some(id)));
    assert!(!devices_match(Some(id), Some(id), None));
}

// Edge cases beyond the C test.

#[test]
fn speakers_and_layouts_far_out_of_range() {
    assert_eq!(channels_to_speakers(u64::MAX), SPEAKERS_UNKNOWN);
    assert_eq!(channels_to_speakers(1 << 32), SPEAKERS_UNKNOWN);
    assert_eq!(channels_to_speakers((1 << 32) | 2), SPEAKERS_UNKNOWN);
    let unknown = channel_map(SPEAKERS_UNKNOWN);
    for layout in [u32::MAX, 0x8000_0000, 10, 255] {
        assert_eq!(channel_map(layout), unknown, "layout {layout}");
    }
    for format in [i32::MIN, -2, SAMPLE_MAX, 14, i32::MAX] {
        assert_eq!(
            to_obs_audio_format(format),
            AUDIO_FORMAT_UNKNOWN,
            "format {format}"
        );
        assert_eq!(sink_format(format), SAMPLE_FLOAT32LE, "format {format}");
        assert_eq!(sample_size(format), None, "format {format}");
    }
}

#[test]
fn sample_sizes() {
    assert_eq!(sample_size(SAMPLE_U8), Some(1));
    assert_eq!(sample_size(SAMPLE_S16LE), Some(2));
    assert_eq!(sample_size(SAMPLE_S32LE), Some(4));
    assert_eq!(sample_size(SAMPLE_FLOAT32LE), Some(4));
    for format in [
        SAMPLE_ALAW,
        SAMPLE_ULAW,
        SAMPLE_S16BE,
        SAMPLE_FLOAT32BE,
        SAMPLE_S32BE,
    ] {
        assert_eq!(sample_size(format), None, "format {format}");
    }
}

/// The clamp bounds themselves, and the first values inside them.
#[test]
fn saturation_boundaries() {
    // u8: 255.0 exactly saturates, the float below it truncates to 254;
    // 0.0 exactly and anything below it give 0.
    assert_eq!(scale_u8(255, 1.0), 255); // 127 * 1 + 128 = 255.0
    assert_eq!(scale_u8(254, 1.0), 254);
    assert_eq!(scale_u8(0, 1.0), 0); // -128 + 128 = 0.0
    assert_eq!(scale_u8(1, 1.0), 1);
    // 127 * 1.0000001 + 128 is the f32 below 255 + 2 ulp: saturated.
    assert_eq!(scale_u8(255, 1.000_000_1), 255);
    // -128 * 0.9999999 + 128 is a tiny positive value: truncated to 0.
    assert_eq!(scale_u8(0, 0.999_999_9), 0);

    // s16: 32767.0 and -32768.0 exactly are the bounds; 32766.998 truncates.
    assert_eq!(scale_s16(i16::MAX, 1.0), i16::MAX);
    assert_eq!(scale_s16(i16::MIN, 1.0), i16::MIN);
    assert_eq!(scale_s16(i16::MAX, 0.999_999_94), 32766);
    assert_eq!(scale_s16(i16::MIN, 0.999_999_94), -32767);
    assert_eq!(scale_s16(-1, 0.5), 0); // -0.5 truncates toward zero
    assert_eq!(scale_s16(1, 0.999_999_94), 0);

    // s32: 2^31 and -2^31 are the bounds; the largest f32 below 2^31 is
    // 2147483520, in range.
    assert_eq!(scale_s32(i32::MAX, 1.0), i32::MAX); // 2^31 after the conversion
    assert_eq!(scale_s32(i32::MIN, 1.0), i32::MIN);
    assert_eq!(scale_s32(2_147_483_520, 1.0), 2_147_483_520);
    assert_eq!(scale_s32(-2_147_483_520, 1.0), -2_147_483_520);
    assert_eq!(scale_s32(1 << 30, 1.999_999_9), 2_147_483_520);
    assert_eq!(scale_s32(-(1 << 30), 1.999_999_9), -2_147_483_520);
    assert_eq!(scale_s32(-3, 0.5), -1);

    // float: no clamp, in either direction.
    assert_eq!(scale_f32(1.0, 20.0), 20.0);
    assert_eq!(scale_f32(-1.0, 20.0), -20.0);
    assert_eq!(scale_f32(f32::MAX, 20.0), f32::INFINITY);
}

/// A NaN or infinite product fails or passes the bounds as a float
/// comparison does. NaN is undefined behaviour in C (x86-64 gives 0, 0 and
/// INT32_MIN for u8, s16 and s32); Rust's conversion gives 0, which these
/// pin. The parity test only documents the C side.
#[test]
fn nan_and_infinite_volume() {
    assert_eq!(scale_u8(200, f32::NAN), 0);
    assert_eq!(scale_s16(1000, f32::NAN), 0);
    assert_eq!(scale_s32(1000, f32::NAN), 0);
    assert!(scale_f32(0.5, f32::NAN).is_nan());

    assert_eq!(scale_u8(200, f32::INFINITY), 255);
    assert_eq!(scale_u8(50, f32::INFINITY), 0);
    assert_eq!(scale_s16(1, f32::INFINITY), i16::MAX);
    assert_eq!(scale_s16(-1, f32::INFINITY), i16::MIN);
    assert_eq!(scale_s32(1, f32::INFINITY), i32::MAX);
    assert_eq!(scale_s32(-1, f32::INFINITY), i32::MIN);
    // 0 * inf is NaN.
    assert_eq!(scale_s16(0, f32::INFINITY), 0);
    // 128 is the u8 centre: 0 * inf is NaN, and NaN + 128 is NaN.
    assert_eq!(scale_u8(128, f32::INFINITY), 0);
}

/// The buffer bounds the work, not frames * channels: a short buffer is
/// processed up to its last whole sample and never past it.
#[test]
fn volume_short_or_empty_buffer() {
    // 3 bytes of s16: one whole sample, one stray byte.
    let mut data = [0x10, 0x00, 0x7F];
    data[..2].copy_from_slice(&1000i16.to_ne_bytes());
    process_volume(SAMPLE_S16LE, 2, 4, 0.5, &mut data);
    assert_eq!(i16::from_ne_bytes([data[0], data[1]]), 500);
    assert_eq!(data[2], 0x7F);

    // frames * channels overflows: the whole buffer, no panic.
    let mut data = [200u8; 5];
    process_volume(SAMPLE_U8, 8, usize::MAX, 0.5, &mut data);
    assert_eq!(data, [164; 5]);

    // 0 frames or 0 channels: nothing.
    let mut data = [200u8; 4];
    process_volume(SAMPLE_U8, 2, 0, 0.5, &mut data);
    process_volume(SAMPLE_U8, 0, 2, 0.5, &mut data);
    assert_eq!(data, [200; 4]);

    let mut empty: [u8; 0] = [];
    for format in ALL_FORMATS {
        process_volume(format, 2, 16, 0.5, &mut empty);
    }

    // A 4-byte format with a 3-byte buffer: nothing.
    let mut data = [1u8, 2, 3];
    process_volume(SAMPLE_S32LE, 1, 1, 0.5, &mut data);
    process_volume(SAMPLE_FLOAT32LE, 1, 1, 0.5, &mut data);
    assert_eq!(data, [1, 2, 3]);
}

#[test]
fn queued_bytes_wraps_as_c() {
    // The uint64_t sum wraps.
    assert_eq!(
        queued_bytes(1, 0, 0, 0, usize::MAX),
        (usize::MAX as u64).wrapping_add(1)
    );
    // The int64_t difference wraps (undefined in C; real indices never get
    // near): i64::MIN - 1 is i64::MAX.
    assert_eq!(queued_bytes(i64::MIN, 1, 0, 0, 0), i64::MAX as u64);
    // A difference that wraps negative holds nothing.
    assert_eq!(queued_bytes(i64::MAX, -1, 0, 0, 7), 7);
    // No bytes per second: nothing played.
    assert_eq!(queued_bytes(1000, 0, u64::MAX, 0, 0), 1000);
}

#[test]
fn drop_split_edges() {
    // A zero frame size (C divides by zero; never passed) drops nothing.
    assert_eq!(drop_split(1000, 192_000, 0, 100), DropSplit::default());
    // A frame larger than the bytes: nothing.
    assert_eq!(drop_split(1000, 192_000, 193, 100), DropSplit::default());
    assert_eq!(
        drop_split(1000, 192_000, usize::MAX, 100),
        DropSplit::default()
    );
    // No bytes per second: nothing.
    assert_eq!(drop_split(u64::MAX, 0, 4, 100), DropSplit::default());
}

#[test]
fn devices_match_edges() {
    // "default" with no default sink name given (the C caller looks it up
    // only when an id is "default"; C dereferences NULL): no match.
    assert!(!devices_match(Some(b"default"), Some(b"sinkA"), None));
    assert!(!devices_match(
        Some(b"sinkA.monitor"),
        Some(b"default"),
        None
    ));
    // Near misses of "default" and ".monitor".
    assert!(!devices_match(
        Some(b"defaul"),
        Some(b"sinkA"),
        Some(b"sinkA")
    ));
    assert!(!devices_match(
        Some(b"default "),
        Some(b"sinkA"),
        Some(b"sinkA")
    ));
    assert!(!devices_match(Some(b"sinkA.monito"), Some(b"sinkA"), None));
    assert!(!devices_match(
        Some(b"sinkA.monitorx"),
        Some(b"sinkA"),
        None
    ));
    assert!(!devices_match(Some(b"sinkA."), Some(b"sinkA"), None));
    // The default sink is used as it is: "default" as its name.
    assert!(devices_match(
        Some(b"default"),
        Some(b"default"),
        Some(b"default")
    ));
    assert!(devices_match(
        Some(b"default.monitor"),
        Some(b"default"),
        Some(b"default")
    ));
    // Bytes, not UTF-8.
    let id: &[u8] = b"\xff\xfe";
    assert!(devices_match(Some(b"\xff\xfe.monitor"), Some(id), None));
    assert!(devices_match(Some(b"default"), Some(id), Some(id)));
    assert!(!devices_match(Some(b"\xff\xfd.monitor"), Some(id), None));
}
