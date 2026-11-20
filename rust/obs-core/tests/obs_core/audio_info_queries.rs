//! Safe mirrors of test_audio_info_queries.c (#5169, #5363), over a
//! recording core. The C test logs only the two calls it can probe ('i' for
//! audio_output_get_info, 'm' for the milliseconds helper); the trace here
//! also logs the core field reads ('h' the audio output handle, 'f' the
//! fixed-buffering flag, 't' the buffering ticks), so the full read order
//! is pinned and the C log is its 'i'/'m' subsequence.
use core::ffi::c_int;

use obs_core::audio_info_queries::{
    AudioCore, AudioInfo, AudioInfo2, get_audio_info, get_audio_info2,
};
use proptest::prelude::*;

// enum speaker_layout values from media-io/audio-io.h.
const SPEAKERS_UNKNOWN: c_int = 0;
const SPEAKERS_MONO: c_int = 1;
const SPEAKERS_STEREO: c_int = 2;
const SPEAKERS_4POINT0: c_int = 4;
const SPEAKERS_5POINT1: c_int = 6;
const SPEAKERS_7POINT1: c_int = 8;

#[derive(Default)]
struct Core {
    has_output: bool,
    info: AudioInfo,
    fixed_buffer: bool,
    ticks: c_int,
    ms_result: c_int,
    ms_args: Option<(c_int, u32)>,
    trace: String,
}

impl Core {
    fn with_output() -> Self {
        Core {
            has_output: true,
            ..Core::default()
        }
    }

    /// The calls the C test's probes log.
    fn c_calls(&self) -> String {
        self.trace.chars().filter(|c| matches!(c, 'i' | 'm')).collect()
    }
}

impl AudioCore for Core {
    fn has_output(&mut self) -> bool {
        self.trace.push('h');
        self.has_output
    }
    fn output_info(&mut self) -> AudioInfo {
        assert!(self.has_output, "info read without an audio output");
        self.trace.push('i');
        self.info
    }
    fn fixed_buffer(&mut self) -> bool {
        self.trace.push('f');
        self.fixed_buffer
    }
    fn max_buffering_ticks(&mut self) -> c_int {
        self.trace.push('t');
        self.ticks
    }
    fn buffer_ms(&mut self, ticks: c_int, samples_per_sec: u32) -> c_int {
        self.trace.push('m');
        self.ms_args = Some((ticks, samples_per_sec));
        self.ms_result
    }
}

/// test_no_core: both reject before any read, output or not.
#[test]
fn no_core() {
    for output in [true, false] {
        assert_eq!(get_audio_info::<Core>(None, output), None);
        assert_eq!(get_audio_info2::<Core>(None, output), None);
    }
}

/// test_info_null_output: a NULL oai reads nothing, not even the handle.
#[test]
fn info_null_output() {
    let mut core = Core::with_output();
    assert_eq!(get_audio_info(Some(&mut core), false), None);
    assert_eq!(core.trace, "");
}

/// test_info_no_audio: no audio output, no info read.
#[test]
fn info_no_audio() {
    let mut core = Core::default();
    assert_eq!(get_audio_info(Some(&mut core), true), None);
    assert_eq!(core.trace, "h");
    assert_eq!(core.c_calls(), "");
}

/// test_info_copies_rate_and_speakers: one info read, both values copied.
#[test]
fn info_copies_rate_and_speakers() {
    for (rate, speakers) in [
        (44100, SPEAKERS_STEREO),
        (48000, SPEAKERS_7POINT1),
        (0, SPEAKERS_UNKNOWN),
        (u32::MAX, SPEAKERS_MONO),
    ] {
        let mut core = Core::with_output();
        core.info = AudioInfo {
            samples_per_sec: rate,
            speakers,
        };
        assert_eq!(
            get_audio_info(Some(&mut core), true),
            Some(AudioInfo {
                samples_per_sec: rate,
                speakers,
            })
        );
        assert_eq!(core.trace, "hi");
        assert_eq!(core.c_calls(), "i");
    }
}

/// test_info2_null_output_still_reads_info: the info read into the local
/// comes first; the NULL output then stops it before the handle is read again.
#[test]
fn info2_null_output_still_reads_info() {
    let mut core = Core::with_output();
    core.info = AudioInfo {
        samples_per_sec: 48000,
        speakers: SPEAKERS_STEREO,
    };
    assert_eq!(get_audio_info2(Some(&mut core), false), None);
    assert_eq!(core.trace, "hi");
    assert_eq!(core.c_calls(), "i");
}

/// test_info2_no_audio: the inner info fails, nothing else is read.
#[test]
fn info2_no_audio() {
    let mut core = Core::default();
    assert_eq!(get_audio_info2(Some(&mut core), true), None);
    assert_eq!(core.trace, "h");
    assert_eq!(core.c_calls(), "");
}

/// test_info2_copies_all_fields: rate and speakers from the info read, the
/// flag from the core, then the helper with the core's ticks and the rate.
#[test]
fn info2_copies_all_fields() {
    for (rate, speakers, fixed, ticks, ms) in [
        (48000, SPEAKERS_STEREO, false, 1, 21),
        (44100, SPEAKERS_5POINT1, true, 45, 1000),
        (96000, SPEAKERS_4POINT0, true, 0, 0),
        (8000, SPEAKERS_MONO, false, i32::MAX, i32::MAX),
    ] {
        let mut core = Core::with_output();
        core.info = AudioInfo {
            samples_per_sec: rate,
            speakers,
        };
        core.fixed_buffer = fixed;
        core.ticks = ticks;
        core.ms_result = ms;
        assert_eq!(
            get_audio_info2(Some(&mut core), true),
            Some(AudioInfo2 {
                samples_per_sec: rate,
                speakers,
                max_buffering_ms: ms.cast_unsigned(),
                fixed_buffering: fixed,
            })
        );
        assert_eq!(core.ms_args, Some((ticks, rate)));
        assert_eq!(core.trace, "hihftm");
        assert_eq!(core.c_calls(), "im");
    }
}

/// test_info2_casts_helper_result: a negative helper result wraps.
#[test]
fn info2_casts_helper_result() {
    let mut core = Core::with_output();
    core.info = AudioInfo {
        samples_per_sec: 48000,
        speakers: SPEAKERS_STEREO,
    };
    core.ms_result = -1;
    let got = get_audio_info2(Some(&mut core), true).map(|info| info.max_buffering_ms);
    assert_eq!(got, Some(u32::MAX));
    assert_eq!(core.c_calls(), "im");
}

proptest! {
    /// Any core: the C's decision table and read order.
    #[test]
    fn any_core_follows_the_c(
        output in any::<bool>(),
        has_output in any::<bool>(),
        rate in any::<u32>(),
        speakers in any::<c_int>(),
        fixed in any::<bool>(),
        ticks in any::<c_int>(),
        ms in any::<c_int>(),
    ) {
        let info = AudioInfo { samples_per_sec: rate, speakers };
        let fresh = || Core { has_output, info, fixed_buffer: fixed, ticks, ms_result: ms, ..Core::default() };

        let mut core = fresh();
        let got = get_audio_info(Some(&mut core), output);
        prop_assert_eq!(got, (output && has_output).then_some(info));
        let trace = match (output, has_output) {
            (false, _) => "",
            (true, false) => "h",
            (true, true) => "hi",
        };
        prop_assert_eq!(core.trace.as_str(), trace);

        let mut core = fresh();
        let got = get_audio_info2(Some(&mut core), output);
        let want = (output && has_output).then_some(AudioInfo2 {
            samples_per_sec: rate,
            speakers,
            max_buffering_ms: ms.cast_unsigned(),
            fixed_buffering: fixed,
        });
        prop_assert_eq!(got, want);
        let trace = match (has_output, output) {
            (false, _) => "h",
            (true, false) => "hi",
            (true, true) => "hihftm",
        };
        prop_assert_eq!(core.trace.as_str(), trace);
        if want.is_some() {
            prop_assert_eq!(core.ms_args, Some((ticks, rate)));
        }
    }
}
