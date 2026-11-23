//! Safe mirrors of test_reset_audio.c over a model of `obs_reset_audio2()`:
//! a core that is present or not, takes each reset with the Rust buffering
//! rules it uses (`core_setup_rules::buffer_set`, `buffer_ticks`,
//! `buffer_default_ticks`) and reads back through `buffer_ms`, as
//! `obs_get_audio_info2()` does, and counts the "audio settings reset"
//! messages it would log. Plus what the C test cannot see: `obs_reset_audio2`
//! is called exactly once, with only the two fields taken and the rest
//! zeroed, and its result is returned unchanged, including a refusal by an
//! active output.
use core::ffi::c_int;

use obs_core::audio_info_queries::{AudioInfo, AudioInfo2};
use obs_core::core_setup_rules::{buffer_default_ticks, buffer_ms, buffer_set, buffer_ticks};
use obs_core::reset_audio::reset_audio;

const SPEAKERS_MONO: c_int = 1;
const SPEAKERS_STEREO: c_int = 2;
const SPEAKERS_2POINT1: c_int = 3;
const SPEAKERS_4POINT0: c_int = 4;
const SPEAKERS_4POINT1: c_int = 5;
const SPEAKERS_5POINT1: c_int = 6;
const SPEAKERS_7POINT1: c_int = 8;

/// What the model core holds after a reset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stored {
    samples_per_sec: u32,
    speakers: c_int,
    fixed_buffer: bool,
    max_buffering_ticks: u32,
}

#[derive(Default)]
struct Core {
    present: bool,
    active: bool,
    stored: Option<Stored>,
    /// The info each `obs_reset_audio2` call received.
    calls: Vec<AudioInfo2>,
    /// "audio settings reset" messages logged.
    messages: usize,
}

impl Core {
    fn started() -> Self {
        Self {
            present: true,
            ..Self::default()
        }
    }

    /// The model `obs_reset_audio2()`.
    fn reset2(&mut self, info: AudioInfo2) -> bool {
        self.calls.push(info);
        if !self.present || self.active {
            return false;
        }
        let ticks = if buffer_set(info.max_buffering_ms) {
            buffer_ticks(info.max_buffering_ms, info.samples_per_sec)
        } else {
            buffer_default_ticks()
        };
        self.stored = Some(Stored {
            samples_per_sec: info.samples_per_sec,
            speakers: info.speakers,
            fixed_buffer: info.fixed_buffering,
            max_buffering_ticks: ticks,
        });
        self.messages += 1;
        true
    }

    fn reset(&mut self, info: AudioInfo) -> bool {
        reset_audio(info, |info2| self.reset2(info2))
    }

    /// The model `obs_get_audio_info2()`.
    fn info2(&self) -> AudioInfo2 {
        let stored = self.stored.unwrap();
        AudioInfo2 {
            samples_per_sec: stored.samples_per_sec,
            speakers: stored.speakers,
            fixed_buffering: stored.fixed_buffer,
            max_buffering_ms: u32::try_from(buffer_ms(
                i32::try_from(stored.max_buffering_ticks).unwrap(),
                stored.samples_per_sec,
            ))
            .unwrap(),
        }
    }
}

fn info(samples_per_sec: u32, speakers: c_int) -> AudioInfo {
    AudioInfo {
        samples_per_sec,
        speakers,
    }
}

fn info2(samples_per_sec: u32, speakers: c_int, fixed: bool, max_ms: u32) -> AudioInfo2 {
    AudioInfo2 {
        samples_per_sec,
        speakers,
        max_buffering_ms: max_ms,
        fixed_buffering: fixed,
    }
}

/// test_reset_audio.c: omitted_fields_fall_back_to_defaults.
#[test]
fn omitted_fields_fall_back_to_defaults() {
    let mut core = Core::started();
    let fixed = info2(48000, SPEAKERS_STEREO, true, 200);

    assert!(core.reset2(fixed));
    assert_eq!(core.info2(), info2(48000, SPEAKERS_STEREO, true, 213));

    assert!(core.reset(info(44100, SPEAKERS_5POINT1)));
    assert_eq!(core.info2(), info2(44100, SPEAKERS_5POINT1, false, 1044));

    assert!(core.reset2(fixed));
    assert_eq!(core.info2(), info2(48000, SPEAKERS_STEREO, true, 213));

    assert!(core.reset(info(48000, SPEAKERS_STEREO)));
    assert_eq!(core.info2(), info2(48000, SPEAKERS_STEREO, false, 960));
}

/// test_reset_audio.c: same_as_reset_audio2_with_zeroed_extras.
#[test]
fn same_as_reset_audio2_with_zeroed_extras() {
    let cases = [
        info(8000, SPEAKERS_MONO),
        info(22050, SPEAKERS_2POINT1),
        info(44100, SPEAKERS_4POINT0),
        info(48000, SPEAKERS_4POINT1),
        info(96000, SPEAKERS_7POINT1),
    ];
    let mut core = Core::started();
    for case in cases {
        assert!(core.reset2(info2(case.samples_per_sec, case.speakers, false, 0)));
        let via2 = core.info2();

        assert!(core.reset(case));
        let via1 = core.info2();

        assert_eq!(via1.samples_per_sec, case.samples_per_sec);
        assert_eq!(via1.speakers, case.speakers);
        assert!(!via1.fixed_buffering);
        assert_eq!(via1, via2);
    }
}

/// test_reset_audio.c: logs_one_reset_message. The message itself is
/// `obs_reset_audio2`'s; this side calls it once per reset and logs nothing.
#[test]
fn logs_one_reset_message() {
    let mut core = Core::started();

    assert!(core.reset(info(44100, SPEAKERS_5POINT1)));
    assert_eq!(core.messages, 1);
    assert_eq!(core.info2(), info2(44100, SPEAKERS_5POINT1, false, 1044));

    core.messages = 0;
    assert!(core.reset(info(48000, SPEAKERS_MONO)));
    assert_eq!(core.messages, 1);
    assert_eq!(core.info2(), info2(48000, SPEAKERS_MONO, false, 960));
}

/// test_reset_audio.c: no_core_returns_false, before the first startup and
/// after a shutdown.
#[test]
fn no_core_returns_false() {
    let stereo = info(48000, SPEAKERS_STEREO);

    let mut core = Core::default();
    assert!(!core.reset(stereo));
    assert_eq!(core.messages, 0);
    assert_eq!(core.stored, None);

    core.present = true;
    assert!(core.reset(stereo));

    core.present = false;
    core.messages = 0;
    assert!(!core.reset(stereo));
    assert_eq!(core.messages, 0);
}

/// Exactly one `obs_reset_audio2` call per reset, with the two fields taken
/// and the others zeroed, whatever the core answers.
#[test]
fn one_reset2_call_with_the_extras_zeroed() {
    for (present, active) in [(false, false), (true, false), (true, true)] {
        let mut core = Core {
            present,
            active,
            ..Core::default()
        };
        core.reset(info(32000, SPEAKERS_4POINT1));
        assert_eq!(core.calls, [info2(32000, SPEAKERS_4POINT1, false, 0)]);
    }
}

/// The result is `obs_reset_audio2`'s, unchanged: an active output refuses
/// the reset and the stored settings stay (test_get_audio pins this on the
/// real core).
#[test]
fn returns_the_reset2_result() {
    let mut core = Core::started();
    assert!(core.reset(info(44100, SPEAKERS_STEREO)));
    let before = core.stored;

    core.active = true;
    assert!(!core.reset(info(48000, SPEAKERS_5POINT1)));
    assert_eq!(core.stored, before);

    for result in [false, true] {
        assert_eq!(reset_audio(info(48000, SPEAKERS_STEREO), |_| result), result);
    }
}

/// Values outside the usual rates and layouts pass through untouched: the
/// shim does not validate (a zero rate is `obs_reset_audio2`'s to refuse,
/// #3272).
#[test]
fn passes_any_rate_and_layout_through() {
    for (rate, speakers) in [(0, 0), (u32::MAX, -1), (1, c_int::MAX), (192_000, c_int::MIN)] {
        let mut seen = None;
        reset_audio(info(rate, speakers), |info2| {
            seen = Some(info2);
            false
        });
        assert_eq!(seen, Some(self::info2(rate, speakers, false, 0)));
    }
}
