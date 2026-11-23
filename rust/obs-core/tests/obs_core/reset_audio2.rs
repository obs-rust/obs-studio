//! Safe mirrors of test_reset_audio2.c over a host core: present or not,
//! with an audio output (its rate, layout and whether it is active) or none,
//! the two buffering fields, and a log. `obs_free_audio()` clears the output
//! and the fields, as its memset does; `obs_init_audio()` opens an output at
//! the rate and layout it is given (a zero rate is refused, as
//! `audio_output_open` refuses it). Reads back as `obs_get_audio_info2()`
//! does, through `core_setup_rules::buffer_ms`. Plus what the C test cannot
//! see: the order of the steps, the active check asked only with an output,
//! the info read only after the free, the fields written before the message,
//! the message before the open, and the open's result returned.
use core::ffi::c_int;

use obs_core::audio_info_queries::AudioInfo2;
use obs_core::core_setup_rules::buffer_ms;
use obs_core::reset_audio2::{Line, Steps, reset_audio2};

const SPEAKERS_MONO: c_int = 1;
const SPEAKERS_STEREO: c_int = 2;
const SPEAKERS_4POINT1: c_int = 5;
const SPEAKERS_5POINT1: c_int = 6;
const SPEAKERS_7POINT1: c_int = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Core,
    HasOutput,
    Active,
    Free,
    Info,
    SetBuffering(c_int, bool),
    Log(Line),
    Init(u32, c_int),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Output {
    rate: u32,
    speakers: c_int,
    active: bool,
}

#[derive(Default)]
struct Host {
    core: bool,
    output: Option<Output>,
    ticks: c_int,
    fixed: bool,
    /// The info the next reset is given; `None` for NULL.
    next: Option<AudioInfo2>,
    /// Overrides `obs_init_audio()`'s answer (the output still opens).
    init_answer: Option<bool>,
    events: Vec<Event>,
}

impl Steps for Host {
    fn core(&mut self) -> bool {
        self.events.push(Event::Core);
        self.core
    }
    fn has_output(&mut self) -> bool {
        self.events.push(Event::HasOutput);
        self.output.is_some()
    }
    fn output_active(&mut self) -> bool {
        self.events.push(Event::Active);
        self.output.unwrap().active
    }
    fn free_audio(&mut self) {
        self.events.push(Event::Free);
        self.output = None;
        self.ticks = 0;
        self.fixed = false;
    }
    fn info(&mut self) -> Option<AudioInfo2> {
        self.events.push(Event::Info);
        self.next
    }
    fn set_buffering(&mut self, max_buffering_ticks: c_int, fixed_buffer: bool) {
        self.events
            .push(Event::SetBuffering(max_buffering_ticks, fixed_buffer));
        self.ticks = max_buffering_ticks;
        self.fixed = fixed_buffer;
    }
    fn log(&mut self, line: Line) {
        self.events.push(Event::Log(line));
    }
    fn init_audio(&mut self, samples_per_sec: u32, speakers: c_int) -> bool {
        self.events.push(Event::Init(samples_per_sec, speakers));
        let opened = samples_per_sec != 0;
        if opened {
            self.output = Some(Output {
                rate: samples_per_sec,
                speakers,
                active: false,
            });
        }
        self.init_answer.unwrap_or(opened)
    }
}

impl Host {
    /// A core after `obs_startup()`: no audio yet.
    fn started() -> Self {
        Host {
            core: true,
            ..Host::default()
        }
    }

    fn reset(&mut self, info: Option<AudioInfo2>) -> bool {
        self.events.clear();
        self.next = info;
        reset_audio2(self)
    }

    /// The model `obs_get_audio_info2()`.
    fn info2(&self) -> Option<AudioInfo2> {
        let output = self.output.filter(|_| self.core)?;
        Some(AudioInfo2 {
            samples_per_sec: output.rate,
            speakers: output.speakers,
            fixed_buffering: self.fixed,
            max_buffering_ms: u32::try_from(buffer_ms(self.ticks, output.rate)).unwrap(),
        })
    }

    /// The texts logged by the last reset.
    fn texts(&self) -> Vec<String> {
        self.events
            .iter()
            .filter_map(|event| match event {
                Event::Log(line) => Some(line.text()),
                _ => None,
            })
            .collect()
    }
}

fn info2(samples_per_sec: u32, speakers: c_int, max_buffering_ms: u32, fixed: bool) -> AudioInfo2 {
    AudioInfo2 {
        samples_per_sec,
        speakers,
        max_buffering_ms,
        fixed_buffering: fixed,
    }
}

/// test_reset_audio2.c's cases and the maximum each reads back.
const CASES: [(AudioInfo2, u32); 5] = [
    (
        AudioInfo2 {
            samples_per_sec: 48000,
            speakers: SPEAKERS_STEREO,
            max_buffering_ms: 200,
            fixed_buffering: true,
        },
        213,
    ),
    (
        AudioInfo2 {
            samples_per_sec: 44100,
            speakers: SPEAKERS_5POINT1,
            max_buffering_ms: 500,
            fixed_buffering: false,
        },
        510,
    ),
    (
        AudioInfo2 {
            samples_per_sec: 96000,
            speakers: SPEAKERS_7POINT1,
            max_buffering_ms: 1000,
            fixed_buffering: true,
        },
        1002,
    ),
    (
        AudioInfo2 {
            samples_per_sec: 32000,
            speakers: SPEAKERS_MONO,
            max_buffering_ms: 0,
            fixed_buffering: false,
        },
        1440,
    ),
    (
        AudioInfo2 {
            samples_per_sec: 22050,
            speakers: SPEAKERS_4POINT1,
            max_buffering_ms: 0,
            fixed_buffering: true,
        },
        2089,
    ),
];

fn read_back((info, max_ms): (AudioInfo2, u32)) -> AudioInfo2 {
    AudioInfo2 {
        max_buffering_ms: max_ms,
        ..info
    }
}

/// test_reset_audio2.c: reads_back_each_reset.
#[test]
fn reads_back_each_reset() {
    let mut host = Host::started();
    for case in CASES.iter().chain(CASES.iter().rev()) {
        assert!(host.reset(Some(case.0)));
        assert!(host.output.is_some());
        assert_eq!(host.info2(), Some(read_back(*case)));
    }
}

/// test_reset_audio2.c: null_info_closes_until_next_reset.
#[test]
fn null_info_closes_until_next_reset() {
    let mut host = Host::started();
    assert!(host.reset(Some(CASES[0].0)));
    assert!(host.output.is_some());

    assert!(host.reset(None));
    assert!(host.texts().is_empty());
    assert_eq!(host.output, None);
    assert_eq!(host.info2(), None);
    assert_eq!(
        host.events,
        [
            Event::Core,
            Event::HasOutput,
            Event::Active,
            Event::Free,
            Event::Info
        ]
    );

    assert!(host.reset(Some(CASES[1].0)));
    assert_eq!(
        host.info2(),
        Some(info2(44100, SPEAKERS_5POINT1, 510, false))
    );
}

/// test_reset_audio2.c: logs_one_reset_message, after the separator.
#[test]
fn logs_one_reset_message() {
    let mut host = Host::started();

    assert!(host.reset(Some(CASES[0].0)));
    assert_eq!(
        host.texts(),
        [
            "---------------------------------",
            "audio settings reset:\n\
             \tsamples per sec: 48000\n\
             \tspeakers:        2\n\
             \tmax buffering:   213 milliseconds\n\
             \tbuffering type:  fixed",
        ]
    );

    assert!(host.reset(Some(CASES[3].0)));
    assert_eq!(
        host.texts(),
        [
            "---------------------------------",
            "audio settings reset:\n\
             \tsamples per sec: 32000\n\
             \tspeakers:        1\n\
             \tmax buffering:   1440 milliseconds\n\
             \tbuffering type:  dynamically increasing",
        ]
    );
}

/// test_reset_audio2.c: active_output_refuses_reset.
#[test]
fn active_output_refuses_reset() {
    let mut host = Host::started();
    assert!(host.reset(Some(CASES[0].0)));
    host.output.as_mut().unwrap().active = true;
    let output = host.output;

    for info in [Some(CASES[1].0), None] {
        assert!(!host.reset(info));
        assert_eq!(host.events, [Event::Core, Event::HasOutput, Event::Active]);
        assert_eq!(host.output, output);
        assert_eq!(host.info2(), Some(info2(48000, SPEAKERS_STEREO, 213, true)));
    }

    host.output.as_mut().unwrap().active = false;
    assert!(host.reset(Some(CASES[1].0)));
    assert_eq!(
        host.info2(),
        Some(info2(44100, SPEAKERS_5POINT1, 510, false))
    );
}

/// test_reset_audio2.c: no_core_returns_false, before the first startup and
/// after a shutdown: nothing but the core is asked.
#[test]
fn no_core_returns_false() {
    let mut host = Host::default();
    for info in [Some(CASES[0].0), None] {
        assert!(!host.reset(info));
        assert_eq!(host.events, [Event::Core]);
    }

    host.core = true;
    assert!(host.reset(Some(CASES[0].0)));
    host.core = false;
    host.output = None;

    for info in [Some(CASES[0].0), None] {
        assert!(!host.reset(info));
        assert_eq!(host.events, [Event::Core]);
    }
}

/// Every step of a reset, in the C's order: the active check only with an
/// output, the info after the free, the fields before the message and the
/// message before the open.
#[test]
fn steps_in_order() {
    let mut host = Host::started();
    let reset = Line::Reset {
        samples_per_sec: 48000,
        speakers: SPEAKERS_STEREO,
        max_buffering_ms: 213,
        fixed: true,
    };

    assert!(host.reset(Some(CASES[0].0)));
    assert_eq!(
        host.events,
        [
            Event::Core,
            Event::HasOutput,
            Event::Free,
            Event::Info,
            Event::SetBuffering(10, true),
            Event::Log(Line::Separator),
            Event::Log(reset),
            Event::Init(48000, SPEAKERS_STEREO),
        ]
    );

    assert!(host.reset(Some(CASES[0].0)));
    assert_eq!(
        host.events,
        [
            Event::Core,
            Event::HasOutput,
            Event::Active,
            Event::Free,
            Event::Info,
            Event::SetBuffering(10, true),
            Event::Log(Line::Separator),
            Event::Log(reset),
            Event::Init(48000, SPEAKERS_STEREO),
        ]
    );
}

/// `obs_init_audio()`'s answer is the result, after the fields and the
/// message.
#[test]
fn returns_the_open_result() {
    for answer in [false, true] {
        let mut host = Host {
            init_answer: Some(answer),
            ..Host::started()
        };
        assert_eq!(host.reset(Some(CASES[1].0)), answer);
        assert_eq!(host.texts().len(), 2);
        assert_eq!(
            host.events.last(),
            Some(&Event::Init(44100, SPEAKERS_5POINT1))
        );
    }
}

/// The ticks are the Rust buffering rules': a set maximum rounds up to whole
/// ticks of 1024 frames with the product wrapping in `u32` (22370 ms at
/// 192 kHz is one tick), 0 is the default of 45 ticks. A rate above
/// `INT_MAX` is logged as a negative `int`, as the C's cast does, and a zero
/// rate logs 0 ms (core_setup_rules' intentional difference, #3272) and
/// goes on to the open, which refuses it.
#[test]
fn buffering_and_message_edges() {
    let mut host = Host::started();

    assert!(host.reset(Some(info2(192_000, SPEAKERS_STEREO, 22370, false))));
    assert!(host.events.contains(&Event::SetBuffering(1, false)));

    assert!(host.reset(Some(info2(48000, SPEAKERS_STEREO, 0, true))));
    assert!(host.events.contains(&Event::SetBuffering(45, true)));

    assert!(host.reset(Some(info2(u32::MAX, -1, 0, false))));
    assert!(host.events.contains(&Event::Log(Line::Reset {
        samples_per_sec: -1,
        speakers: -1,
        max_buffering_ms: -46080000,
        fixed: false,
    })));

    assert!(!host.reset(Some(info2(0, SPEAKERS_STEREO, 200, false))));
    let after_info = host.events.iter().position(|e| *e == Event::Info).unwrap() + 1;
    assert_eq!(
        &host.events[after_info..],
        [
            Event::SetBuffering(0, false),
            Event::Log(Line::Separator),
            Event::Log(Line::Reset {
                samples_per_sec: 0,
                speakers: SPEAKERS_STEREO,
                max_buffering_ms: 0,
                fixed: false,
            }),
            Event::Init(0, SPEAKERS_STEREO),
        ]
    );
    assert_eq!(host.output, None);
}
