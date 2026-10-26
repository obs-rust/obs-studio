//! Tier 1: `obs_output::output_start`, the output start and begin-capture
//! decisions.
//!
//! The `steps_*` cases port `test/cmocka/test_output_start_steps.c` 1:1: its
//! 19 cases under the same names, in the same order, with the same values
//! and expectations, the "characterized, not endorsed" ones included. The C
//! calls the hidden `obs_output_start_*`, `obs_output_raw_audio_delay_on`
//! and `obs_encoder_size_audio_defaults` functions; here the safe core is
//! called directly. The C's stand-in output and its copies of
//! `obs_output_start()`, `obs_output_actual_start()`,
//! `obs_output_initialize_encoders()`, `begin_delayed_capture()`,
//! `reset_raw_output()`, `obs_output_begin_data_capture_internal()` and
//! `obs_output_begin_data_capture()` are [`Stand`].
//!
//! The `edge_*` cases are new: every decision against a model of the
//! original C over every low 16-bit flag word under 16 high words, every
//! restart count from -1,000 to 1,000 and the extremes, every delay to
//! 100,000 and the top of `u32`, the conversion fill over every
//! combination of edge fields, the stand-in through every caller for every
//! low flag word, each atomic set and clear and each call result, and
//! proptests of every decision over full-range values and of the stand-in
//! over call sequences against the origin/master code with its expressions
//! inline, with another thread flipping the atomics between them.

use obs_output::output_start::{
    AudioFill, OBS_OUTPUT_AUDIO, OBS_OUTPUT_ENCODED, OBS_OUTPUT_SERVICE, OBS_OUTPUT_VIDEO,
    audio_defaults, delay_on, drop_restart_ref, init_audio, init_video, pair, reset_raw, service,
};
use proptest::prelude::*;

pub(crate) const F_VIDEO: u32 = 1;
pub(crate) const F_AUDIO: u32 = 2;
pub(crate) const F_ENCODED: u32 = 4;
pub(crate) const F_SERVICE: u32 = 8;

pub(crate) const FORMAT_UNKNOWN: i32 = 0;
pub(crate) const FORMAT_16BIT: i32 = 2;
pub(crate) const FORMAT_FLOAT: i32 = 4;
pub(crate) const FORMAT_FLOAT_PLANAR: i32 = 8;
pub(crate) const SPEAKERS_UNKNOWN: i32 = 0;
pub(crate) const SPEAKERS_MONO: i32 = 1;
pub(crate) const SPEAKERS_STEREO: i32 = 2;
pub(crate) const SPEAKERS_5POINT1: i32 = 6;

const EDGE_LONG: [i64; 8] = [i64::MIN, -2, -1, 0, 1, 2, 3, i64::MAX];
pub(crate) const EDGE_U32: [u32; 7] = [0, 1, 2, 30, 0x7fff_ffff, 0x8000_0000, u32::MAX];

pub(crate) const STREAM: u32 = F_VIDEO | F_AUDIO | F_ENCODED | F_SERVICE;
pub(crate) const RECORDING: u32 = F_VIDEO | F_AUDIO | F_ENCODED;
pub(crate) const RAW_VIDEO: u32 = F_VIDEO;

pub(crate) fn fill(format: i32, samples_per_sec: u32, speakers: i32) -> AudioFill {
    AudioFill {
        format,
        samples_per_sec,
        speakers,
    }
}

// ------------------------------------------------------------------------
// The gates the callers ask, so the stand-in runs on the core ([`CORE`])
// or on the origin/master expressions inline ([`ORIGIN`]).

pub(crate) struct Gates {
    pub(crate) service: fn(u32) -> bool,
    pub(crate) drop_restart_ref: fn(i64) -> bool,
    pub(crate) init_video: fn(u32) -> bool,
    pub(crate) init_audio: fn(u32) -> bool,
    pub(crate) reset_raw: fn(u32) -> bool,
    pub(crate) pair: fn(u32) -> bool,
    pub(crate) delay_on: fn(u32) -> bool,
    /// `reset_raw_output()`'s fill: the conversion, then the mix
    pub(crate) audio_defaults: fn(AudioFill, AudioFill) -> AudioFill,
}

pub(crate) const CORE: Gates = Gates {
    service,
    drop_restart_ref,
    init_video,
    init_audio,
    reset_raw,
    pair,
    delay_on,
    audio_defaults,
};

/// origin/master's obs-output.c, its expressions inline: `flag_service()`,
/// the `delay_restart_refs` load's truth, `flag_video()`, `flag_audio()`,
/// `!flag_encoded()`, `flag_video() && flag_audio()`, `delay_sec`'s truth
/// and `reset_raw_output()`'s three field tests.
pub(crate) const ORIGIN: Gates = Gates {
    service: |f| (f & OBS_OUTPUT_SERVICE) != 0,
    drop_restart_ref: |r| r != 0,
    init_video: |f| (f & OBS_OUTPUT_VIDEO) != 0,
    init_audio: |f| (f & OBS_OUTPUT_AUDIO) != 0,
    reset_raw: |f| (f & OBS_OUTPUT_ENCODED) == 0,
    pair: |f| (f & OBS_OUTPUT_VIDEO) != 0 && (f & OBS_OUTPUT_AUDIO) != 0,
    delay_on: |d| d != 0,
    audio_defaults: |conv, mix| {
        let mut info = mix;
        if conv.samples_per_sec != 0 {
            info.samples_per_sec = conv.samples_per_sec;
        }
        if conv.format != FORMAT_UNKNOWN {
            info.format = conv.format;
        }
        if conv.speakers != SPEAKERS_UNKNOWN {
            info.speakers = conv.speakers;
        }
        info
    },
};

// ------------------------------------------------------------------------
// The stand-in output and the callers, as the C test's.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Track {
    pub(crate) present: bool,
    pub(crate) locks: u32,
    pub(crate) caption_timestamp: u32,
    pub(crate) caption_data_size: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Stand {
    // the output
    pub(crate) has_data: bool,
    pub(crate) flags: u32,
    pub(crate) delay_sec: u32,
    pub(crate) has_audio: bool,
    pub(crate) audio_conversion_set: bool,
    pub(crate) conv: AudioFill,
    pub(crate) has_last_error: bool,
    pub(crate) stop_code: i32,
    pub(crate) tracks: [Track; 3],
    // the atomics
    pub(crate) starting: bool,
    pub(crate) active: bool,
    pub(crate) reconnecting: bool,
    pub(crate) delay_active: bool,
    pub(crate) delay_capturing: bool,
    pub(crate) data_active: bool,
    pub(crate) delay_restart_refs: i64,
    // what the callers wrote
    pub(crate) starting_drawn_count: u32,
    pub(crate) starting_lagged_count: u32,
    pub(crate) total_frames: i32,
    pub(crate) audio: AudioFill,
    pub(crate) total_audio_frames: u64,
    pub(crate) audio_start_ts: u64,
    pub(crate) video_start_ts: u64,
    // the results of the calls the C keeps
    pub(crate) valid: bool,
    pub(crate) can_try: bool,
    pub(crate) service_init_ok: bool,
    pub(crate) start_ok: bool,
    pub(crate) delay_start_ok: bool,
    pub(crate) init_video_ok: bool,
    pub(crate) init_audio_ok: bool,
    pub(crate) can_begin: bool,
    // the mix
    pub(crate) mix: AudioFill,
    // the record
    pub(crate) log: Vec<&'static str>,
}

impl Stand {
    pub(crate) fn new(flags: u32) -> Self {
        Self {
            has_data: true,
            flags,
            delay_sec: 0,
            has_audio: true,
            audio_conversion_set: false,
            conv: AudioFill::default(),
            has_last_error: false,
            stop_code: 0,
            tracks: [Track::default(); 3],
            starting: false,
            active: false,
            reconnecting: false,
            delay_active: false,
            delay_capturing: false,
            data_active: false,
            delay_restart_refs: 0,
            starting_drawn_count: 0,
            starting_lagged_count: 0,
            total_frames: 9,
            audio: AudioFill::default(),
            total_audio_frames: 77,
            audio_start_ts: 5,
            video_start_ts: 6,
            valid: true,
            can_try: true,
            service_init_ok: true,
            start_ok: true,
            delay_start_ok: true,
            init_video_ok: true,
            init_audio_ok: true,
            can_begin: true,
            mix: fill(FORMAT_FLOAT_PLANAR, 48000, SPEAKERS_STEREO),
            log: Vec::new(),
        }
    }

    pub(crate) fn log(&self) -> String {
        self.log.join(" ")
    }

    fn rec(&mut self, what: &'static str) {
        self.log.push(what);
    }

    /// `obs_output_actual_start()`
    pub(crate) fn actual_start(&mut self, g: &Gates) -> bool {
        let mut success = false;

        self.rec("wait-stopping");
        self.stop_code = 0;
        if self.has_last_error {
            self.rec("free-error");
            self.has_last_error = false;
        }

        self.starting = true;
        if self.has_data {
            self.rec("info.start");
            success = self.start_ok;
        }

        if success {
            self.rec("frame-counts");
            self.starting_drawn_count = 100;
            self.starting_lagged_count = 3;
        }

        if !success {
            self.starting = false;
        }

        if (g.drop_restart_ref)(self.delay_restart_refs) {
            self.rec("drop-restart-ref");
            self.delay_restart_refs = self.delay_restart_refs.wrapping_sub(1);
        }

        for t in &mut self.tracks {
            if !t.present {
                continue;
            }
            t.locks += 1;
            t.caption_timestamp = 0;
            t.caption_data_size = 0;
        }

        success
    }

    /// `obs_output_start()`
    pub(crate) fn start(&mut self, g: &Gates) -> bool {
        if !self.valid {
            return false;
        }
        if !self.has_data {
            return false;
        }

        if self.starting && !self.delay_active {
            self.rec("warn-already-starting");
            return false;
        }

        if (g.service)(self.flags) {
            self.rec("can-try");
            if !self.can_try {
                return false;
            }
            self.rec("service-init");
            if !self.service_init_ok {
                return false;
            }
        }

        if (g.delay_on)(self.delay_sec) {
            self.rec("delay-start");
            self.delay_start_ok
        } else {
            if self.actual_start(g) {
                self.rec("signal-starting");
                return true;
            }
            false
        }
    }

    /// `obs_output_initialize_encoders()`
    pub(crate) fn initialize_encoders(&mut self, g: &Gates) -> bool {
        if !self.valid {
            return false;
        }
        if (self.flags & F_ENCODED) == 0 {
            self.rec("warn-raw");
            return false;
        }
        if self.active {
            return self.delay_active;
        }

        if (g.init_video)(self.flags) {
            self.rec("init-video");
            if !self.init_video_ok {
                return false;
            }
        }
        if (g.init_audio)(self.flags) {
            self.rec("init-audio");
            if !self.init_audio_ok {
                return false;
            }
        }

        true
    }

    /// `begin_delayed_capture()`
    pub(crate) fn begin_delayed_capture(&mut self) -> bool {
        if self.delay_capturing {
            return false;
        }

        self.rec("reset-packets");
        self.delay_capturing = true;

        if self.reconnecting {
            self.rec("signal-reconnect-success");
            self.reconnecting = false;
        } else {
            self.rec("signal-start");
        }

        true
    }

    /// `reset_raw_output()`
    pub(crate) fn reset_raw_output(&mut self, g: &Gates) {
        self.rec("clear-raw-audio");

        if self.has_audio {
            let mut info = self.mix;
            if self.audio_conversion_set {
                info = (g.audio_defaults)(self.conv, info);
            }
            self.audio = info;
            self.total_audio_frames = 0;
        }

        self.audio_start_ts = 0;
        self.video_start_ts = 0;

        self.rec("pause-reset");
    }

    /// `obs_output_begin_data_capture_internal()`
    pub(crate) fn begin_internal(&mut self, g: &Gates) -> bool {
        if self.delay_active {
            return self.begin_delayed_capture();
        }
        if self.active {
            return false;
        }

        self.total_frames = 0;

        if (g.reset_raw)(self.flags) {
            self.reset_raw_output(g);
        }

        self.rec("can-begin");
        if !self.can_begin {
            return false;
        }

        if (g.pair)(self.flags) {
            self.rec("pair");
        }

        self.data_active = true;
        self.rec("hook");
        // hook_data_capture(): an encoded output with a delay turns it on
        if (self.flags & F_ENCODED) != 0 && self.delay_sec != 0 {
            self.delay_active = true;
        }

        self.rec("batch-size");

        if (g.service)(self.flags) {
            self.rec("service-activate");
        }

        self.rec("signal-activate");
        self.active = true;

        if self.reconnecting {
            self.rec("signal-reconnect-success");
            self.reconnecting = false;
        } else if self.delay_active {
            self.rec("signal-starting");
        } else {
            self.rec("signal-start");
        }

        true
    }

    /// `obs_output_begin_data_capture()`
    pub(crate) fn begin(&mut self, g: &Gates) -> bool {
        if !self.valid {
            return false;
        }

        let success = self.begin_internal(g);

        self.starting = false;
        success
    }
}

fn conversion_fill(
    format: i32,
    rate: u32,
    speakers: i32,
    mix_format: i32,
    mix_rate: u32,
    mix_speakers: i32,
) -> AudioFill {
    audio_defaults(
        fill(format, rate, speakers),
        fill(mix_format, mix_rate, mix_speakers),
    )
}

// ------------------------------------------------------------------------
// test_output_start_steps.c, 1:1

#[test]
fn steps_constants() {
    assert_eq!(OBS_OUTPUT_VIDEO, 1);
    assert_eq!(OBS_OUTPUT_AUDIO, 2);
    assert_eq!(OBS_OUTPUT_ENCODED, 4);
    assert_eq!(OBS_OUTPUT_SERVICE, 8);
}

#[test]
fn steps_flag_rules() {
    for high in [0, 0x10000u32, 0x8000_0000, 0xffff_0000] {
        for low in 0..=0xffffu32 {
            let f = high | low;
            assert_eq!(service(f), (f & F_SERVICE) != 0);
            assert_eq!(init_video(f), (f & F_VIDEO) != 0);
            assert_eq!(init_audio(f), (f & F_AUDIO) != 0);
            assert_eq!(reset_raw(f), (f & F_ENCODED) == 0);
            assert_eq!(pair(f), (f & F_VIDEO) != 0 && (f & F_AUDIO) != 0);
        }
    }
    assert!(service(F_VIDEO | F_AUDIO | F_ENCODED | F_SERVICE));
    assert!(!service(F_VIDEO | F_AUDIO | F_ENCODED));
    assert!(!reset_raw(F_VIDEO | F_AUDIO | F_ENCODED));
    assert!(reset_raw(F_VIDEO));
    assert!(pair(F_VIDEO | F_AUDIO));
    assert!(!pair(F_VIDEO));
    assert!(!pair(F_AUDIO));
    assert!(!pair(0));
}

#[test]
fn steps_drop_restart_ref() {
    assert!(!drop_restart_ref(0));
    assert!(drop_restart_ref(1));
    assert!(drop_restart_ref(2));
    // a negative count is dropped too (characterized, not endorsed)
    assert!(drop_restart_ref(-1));
    for r in EDGE_LONG {
        assert_eq!(drop_restart_ref(r), r != 0);
    }
}

#[test]
fn steps_delayed_start() {
    assert!(!delay_on(0));
    for d in EDGE_U32 {
        assert_eq!(delay_on(d), d != 0);
    }
}

#[test]
fn steps_audio_conversion_fields() {
    // nothing set: the mix's
    let out = conversion_fill(
        FORMAT_UNKNOWN,
        0,
        SPEAKERS_UNKNOWN,
        FORMAT_FLOAT_PLANAR,
        48000,
        SPEAKERS_STEREO,
    );
    assert_eq!(out, fill(FORMAT_FLOAT_PLANAR, 48000, SPEAKERS_STEREO));

    // each field set alone replaces only the mix's
    let out = conversion_fill(
        FORMAT_UNKNOWN,
        44100,
        SPEAKERS_UNKNOWN,
        FORMAT_FLOAT_PLANAR,
        48000,
        SPEAKERS_STEREO,
    );
    assert_eq!(out, fill(FORMAT_FLOAT_PLANAR, 44100, SPEAKERS_STEREO));
    let out = conversion_fill(
        FORMAT_16BIT,
        0,
        SPEAKERS_UNKNOWN,
        FORMAT_FLOAT_PLANAR,
        48000,
        SPEAKERS_STEREO,
    );
    assert_eq!(out, fill(FORMAT_16BIT, 48000, SPEAKERS_STEREO));
    let out = conversion_fill(
        FORMAT_UNKNOWN,
        0,
        SPEAKERS_MONO,
        FORMAT_FLOAT_PLANAR,
        48000,
        SPEAKERS_STEREO,
    );
    assert_eq!(out, fill(FORMAT_FLOAT_PLANAR, 48000, SPEAKERS_MONO));

    // all set
    let out = conversion_fill(
        FORMAT_16BIT,
        44100,
        SPEAKERS_5POINT1,
        FORMAT_FLOAT_PLANAR,
        48000,
        SPEAKERS_STEREO,
    );
    assert_eq!(out, fill(FORMAT_16BIT, 44100, SPEAKERS_5POINT1));

    // a mix of 0 and unknown fields stays so where nothing is set
    // (characterized, not endorsed)
    let out = conversion_fill(
        FORMAT_UNKNOWN,
        0,
        SPEAKERS_UNKNOWN,
        FORMAT_UNKNOWN,
        0,
        SPEAKERS_UNKNOWN,
    );
    assert_eq!(out, fill(FORMAT_UNKNOWN, 0, SPEAKERS_UNKNOWN));

    // the edges of the rate: only 0 keeps the mix's
    for r in EDGE_U32 {
        let out = conversion_fill(
            FORMAT_UNKNOWN,
            r,
            SPEAKERS_UNKNOWN,
            FORMAT_FLOAT,
            48000,
            SPEAKERS_STEREO,
        );
        assert_eq!(out.samples_per_sec, if r != 0 { r } else { 48000 });
    }
    // any format or speakers value that is not unknown is taken as it is
    // (characterized, not endorsed)
    let out = conversion_fill(-1, 0, 99, FORMAT_FLOAT, 48000, SPEAKERS_STEREO);
    assert_eq!(out.format, -1);
    assert_eq!(out.speakers, 99);
}

#[test]
fn steps_start_routes() {
    let mut o = Stand::new(STREAM);
    assert!(o.start(&CORE));
    assert_eq!(
        o.log(),
        "can-try service-init wait-stopping info.start frame-counts signal-starting"
    );
    assert!(o.starting);
    assert_eq!(o.starting_drawn_count, 100);

    let mut o = Stand::new(RECORDING);
    assert!(o.start(&CORE));
    assert_eq!(
        o.log(),
        "wait-stopping info.start frame-counts signal-starting"
    );

    let mut o = Stand::new(STREAM);
    o.valid = false;
    assert!(!o.start(&CORE));
    assert_eq!(o.log(), "");
    let mut o = Stand::new(STREAM);
    o.has_data = false;
    assert!(!o.start(&CORE));
    assert_eq!(o.log(), "");
}

#[test]
fn steps_start_already_starting() {
    let mut o = Stand::new(STREAM);
    o.starting = true;
    assert!(!o.start(&CORE));
    assert_eq!(o.log(), "warn-already-starting");
    assert!(o.starting);

    // a delayed output handles its own restarts
    let mut o = Stand::new(STREAM);
    o.starting = true;
    o.delay_active = true;
    o.delay_sec = 10;
    assert!(o.start(&CORE));
    assert_eq!(o.log(), "can-try service-init delay-start");

    let mut o = Stand::new(RECORDING);
    o.delay_active = true;
    assert!(o.start(&CORE));
    assert_eq!(
        o.log(),
        "wait-stopping info.start frame-counts signal-starting"
    );

    // only starting refuses here (characterized, not endorsed)
    let mut o = Stand::new(RECORDING);
    o.active = true;
    o.reconnecting = true;
    o.delay_capturing = true;
    assert!(o.start(&CORE));
    assert_eq!(
        o.log(),
        "wait-stopping info.start frame-counts signal-starting"
    );
}

#[test]
fn steps_start_service_gate() {
    let mut o = Stand::new(STREAM);
    o.can_try = false;
    assert!(!o.start(&CORE));
    assert_eq!(o.log(), "can-try");
    assert!(!o.starting);

    let mut o = Stand::new(STREAM);
    o.service_init_ok = false;
    assert!(!o.start(&CORE));
    assert_eq!(o.log(), "can-try service-init");

    let mut o = Stand::new(RECORDING);
    o.can_try = false;
    o.service_init_ok = false;
    assert!(o.start(&CORE));
    assert_eq!(
        o.log(),
        "wait-stopping info.start frame-counts signal-starting"
    );

    let mut o = Stand::new(F_VIDEO | F_SERVICE);
    assert!(o.start(&CORE));
    assert_eq!(
        o.log(),
        "can-try service-init wait-stopping info.start frame-counts signal-starting"
    );
}

#[test]
fn steps_start_delayed() {
    let mut o = Stand::new(STREAM);
    o.delay_sec = 20;
    assert!(o.start(&CORE));
    assert_eq!(o.log(), "can-try service-init delay-start");
    assert!(!o.starting);
    let mut o = Stand::new(STREAM);
    o.delay_sec = u32::MAX;
    o.delay_start_ok = false;
    assert!(!o.start(&CORE));
    assert_eq!(o.log(), "can-try service-init delay-start");

    let mut o = Stand::new(STREAM);
    o.delay_sec = 1;
    o.can_try = false;
    assert!(!o.start(&CORE));
    assert_eq!(o.log(), "can-try");

    // a raw output with a delay is delayed too (characterized, not
    // endorsed)
    let mut o = Stand::new(RAW_VIDEO);
    o.delay_sec = 5;
    assert!(o.start(&CORE));
    assert_eq!(o.log(), "delay-start");
}

#[test]
fn steps_actual_start_steps() {
    let mut o = Stand::new(RECORDING);
    o.stop_code = 3;
    o.has_last_error = true;
    o.tracks[0] = Track {
        present: true,
        locks: 0,
        caption_timestamp: 12,
        caption_data_size: 40,
    };
    o.tracks[2] = Track {
        present: true,
        locks: 0,
        caption_timestamp: 0,
        caption_data_size: 8,
    };
    assert!(o.actual_start(&CORE));
    assert_eq!(o.log(), "wait-stopping free-error info.start frame-counts");
    assert_eq!(o.stop_code, 0);
    assert!(!o.has_last_error);
    assert!(o.starting);
    assert_eq!(o.delay_restart_refs, 0);
    assert_eq!(o.tracks[0].locks, 1);
    assert_eq!(o.tracks[0].caption_timestamp, 0);
    assert_eq!(o.tracks[0].caption_data_size, 0);
    assert_eq!(o.tracks[1].locks, 0);
    assert_eq!(o.tracks[2].locks, 1);
    assert_eq!(o.tracks[2].caption_data_size, 0);

    let mut o = Stand::new(RECORDING);
    o.start_ok = false;
    o.tracks[1] = Track {
        present: true,
        locks: 0,
        caption_timestamp: 0,
        caption_data_size: 4,
    };
    assert!(!o.actual_start(&CORE));
    assert_eq!(o.log(), "wait-stopping info.start");
    assert!(!o.starting);
    assert_eq!(o.starting_drawn_count, 0);
    assert_eq!(o.tracks[1].caption_data_size, 0);

    let mut o = Stand::new(RECORDING);
    o.has_data = false;
    assert!(!o.actual_start(&CORE));
    assert_eq!(o.log(), "wait-stopping");
    assert!(!o.starting);
}

#[test]
fn steps_actual_start_restart_refs() {
    let mut o = Stand::new(STREAM);
    o.delay_restart_refs = 2;
    assert!(o.actual_start(&CORE));
    assert_eq!(
        o.log(),
        "wait-stopping info.start frame-counts drop-restart-ref"
    );
    assert_eq!(o.delay_restart_refs, 1);

    let mut o = Stand::new(STREAM);
    o.delay_restart_refs = 1;
    o.start_ok = false;
    assert!(!o.actual_start(&CORE));
    assert_eq!(o.log(), "wait-stopping info.start drop-restart-ref");
    assert_eq!(o.delay_restart_refs, 0);
    assert!(!o.starting);

    let mut o = Stand::new(STREAM);
    o.delay_restart_refs = 3;
    assert!(o.actual_start(&CORE));
    assert!(o.actual_start(&CORE));
    assert_eq!(o.delay_restart_refs, 1);

    let mut o = Stand::new(STREAM);
    o.start_ok = false;
    assert!(!o.actual_start(&CORE));
    assert_eq!(o.delay_restart_refs, 0);

    // a negative count is decremented too (characterized, not endorsed)
    let mut o = Stand::new(STREAM);
    o.delay_restart_refs = -1;
    assert!(o.actual_start(&CORE));
    assert_eq!(o.delay_restart_refs, -2);
}

#[test]
fn steps_initialize_encoders() {
    let mut o = Stand::new(RAW_VIDEO);
    assert!(!o.initialize_encoders(&CORE));
    assert_eq!(o.log(), "warn-raw");

    let mut o = Stand::new(RECORDING);
    assert!(o.initialize_encoders(&CORE));
    assert_eq!(o.log(), "init-video init-audio");
    let mut o = Stand::new(F_VIDEO | F_ENCODED);
    assert!(o.initialize_encoders(&CORE));
    assert_eq!(o.log(), "init-video");
    let mut o = Stand::new(F_AUDIO | F_ENCODED);
    assert!(o.initialize_encoders(&CORE));
    assert_eq!(o.log(), "init-audio");

    // neither flag (characterized, not endorsed)
    let mut o = Stand::new(F_ENCODED);
    assert!(o.initialize_encoders(&CORE));
    assert_eq!(o.log(), "");

    let mut o = Stand::new(RECORDING);
    o.init_video_ok = false;
    assert!(!o.initialize_encoders(&CORE));
    assert_eq!(o.log(), "init-video");
    let mut o = Stand::new(RECORDING);
    o.init_audio_ok = false;
    assert!(!o.initialize_encoders(&CORE));
    assert_eq!(o.log(), "init-video init-audio");
}

#[test]
fn steps_initialize_encoders_active() {
    let mut o = Stand::new(STREAM);
    o.active = true;
    assert!(!o.initialize_encoders(&CORE));
    assert_eq!(o.log(), "");
    let mut o = Stand::new(STREAM);
    o.active = true;
    o.delay_active = true;
    assert!(o.initialize_encoders(&CORE));
    assert_eq!(o.log(), "");

    let mut o = Stand::new(STREAM);
    o.delay_active = true;
    assert!(o.initialize_encoders(&CORE));
    assert_eq!(o.log(), "init-video init-audio");

    let mut o = Stand::new(RAW_VIDEO);
    o.active = true;
    o.delay_active = true;
    assert!(!o.initialize_encoders(&CORE));
    assert_eq!(o.log(), "warn-raw");
}

#[test]
fn steps_reset_raw_audio() {
    let mix = fill(FORMAT_FLOAT_PLANAR, 48000, SPEAKERS_STEREO);

    let mut o = Stand::new(RAW_VIDEO | F_AUDIO);
    o.reset_raw_output(&CORE);
    assert_eq!(o.log(), "clear-raw-audio pause-reset");
    assert_eq!(o.audio, mix);
    assert_eq!(o.total_audio_frames, 0);
    assert_eq!(o.audio_start_ts, 0);
    assert_eq!(o.video_start_ts, 0);

    // a conversion that is not set is ignored
    let mut o = Stand::new(RAW_VIDEO | F_AUDIO);
    o.conv = fill(FORMAT_16BIT, 44100, SPEAKERS_MONO);
    o.reset_raw_output(&CORE);
    assert_eq!(o.audio, mix);

    // set, each field alone
    let mut o = Stand::new(RAW_VIDEO | F_AUDIO);
    o.audio_conversion_set = true;
    o.conv.samples_per_sec = 44100;
    o.reset_raw_output(&CORE);
    assert_eq!(o.audio, fill(FORMAT_FLOAT_PLANAR, 44100, SPEAKERS_STEREO));
    let mut o = Stand::new(RAW_VIDEO | F_AUDIO);
    o.audio_conversion_set = true;
    o.conv.format = FORMAT_16BIT;
    o.reset_raw_output(&CORE);
    assert_eq!(o.audio, fill(FORMAT_16BIT, 48000, SPEAKERS_STEREO));
    let mut o = Stand::new(RAW_VIDEO | F_AUDIO);
    o.audio_conversion_set = true;
    o.conv.speakers = SPEAKERS_MONO;
    o.reset_raw_output(&CORE);
    assert_eq!(o.audio, fill(FORMAT_FLOAT_PLANAR, 48000, SPEAKERS_MONO));

    // set with 0 and unknown: the mix's
    let mut o = Stand::new(RAW_VIDEO | F_AUDIO);
    o.audio_conversion_set = true;
    o.reset_raw_output(&CORE);
    assert_eq!(o.audio, mix);

    // no audio: only the timestamps and the pause
    let mut o = Stand::new(RAW_VIDEO);
    o.has_audio = false;
    o.audio.samples_per_sec = 32000;
    o.reset_raw_output(&CORE);
    assert_eq!(o.log(), "clear-raw-audio pause-reset");
    assert_eq!(o.audio.samples_per_sec, 32000);
    assert_eq!(o.total_audio_frames, 77);
    assert_eq!(o.audio_start_ts, 0);
    assert_eq!(o.video_start_ts, 0);
}

#[test]
fn steps_begin_routes() {
    let mut o = Stand::new(STREAM);
    o.starting = true;
    assert!(o.begin(&CORE));
    assert_eq!(
        o.log(),
        "can-begin pair hook batch-size service-activate signal-activate signal-start"
    );
    assert!(o.active);
    assert!(o.data_active);
    assert!(!o.starting);
    assert_eq!(o.total_frames, 0);

    let mut o = Stand::new(RECORDING);
    assert!(o.begin(&CORE));
    assert_eq!(
        o.log(),
        "can-begin pair hook batch-size signal-activate signal-start"
    );

    let mut o = Stand::new(RAW_VIDEO);
    assert!(o.begin(&CORE));
    assert_eq!(
        o.log(),
        "clear-raw-audio pause-reset can-begin hook batch-size signal-activate signal-start"
    );

    // a raw AV output is paired too (characterized, not endorsed)
    let mut o = Stand::new(F_VIDEO | F_AUDIO);
    assert!(o.begin(&CORE));
    assert_eq!(
        o.log(),
        "clear-raw-audio pause-reset can-begin pair hook batch-size signal-activate signal-start"
    );

    let mut o = Stand::new(F_AUDIO | F_ENCODED);
    assert!(o.begin(&CORE));
    assert_eq!(
        o.log(),
        "can-begin hook batch-size signal-activate signal-start"
    );
}

#[test]
fn steps_begin_refusals() {
    let mut o = Stand::new(STREAM);
    o.active = true;
    o.starting = true;
    o.total_frames = 9;
    assert!(!o.begin(&CORE));
    assert_eq!(o.log(), "");
    assert!(!o.starting);
    assert_eq!(o.total_frames, 9);

    let mut o = Stand::new(RAW_VIDEO);
    o.can_begin = false;
    o.starting = true;
    assert!(!o.begin(&CORE));
    assert_eq!(o.log(), "clear-raw-audio pause-reset can-begin");
    assert_eq!(o.total_frames, 0);
    assert!(!o.data_active);
    assert!(!o.active);
    assert!(!o.starting);

    let mut o = Stand::new(STREAM);
    o.valid = false;
    o.starting = true;
    assert!(!o.begin(&CORE));
    assert!(o.starting);
}

#[test]
fn steps_begin_signals() {
    let mut o = Stand::new(STREAM);
    o.reconnecting = true;
    assert!(o.begin(&CORE));
    assert_eq!(
        o.log(),
        "can-begin pair hook batch-size service-activate signal-activate signal-reconnect-success"
    );
    assert!(!o.reconnecting);

    let mut o = Stand::new(STREAM);
    o.delay_sec = 30;
    assert!(o.begin(&CORE));
    assert_eq!(
        o.log(),
        "can-begin pair hook batch-size service-activate signal-activate signal-starting"
    );
    assert!(o.delay_active);

    let mut o = Stand::new(STREAM);
    o.delay_sec = 30;
    o.reconnecting = true;
    assert!(o.begin(&CORE));
    assert_eq!(
        o.log(),
        "can-begin pair hook batch-size service-activate signal-activate signal-reconnect-success"
    );

    let mut o = Stand::new(RAW_VIDEO);
    o.delay_sec = 30;
    assert!(o.begin(&CORE));
    assert!(!o.delay_active);
    assert_eq!(
        o.log(),
        "clear-raw-audio pause-reset can-begin hook batch-size signal-activate signal-start"
    );
}

#[test]
fn steps_begin_delayed() {
    let mut o = Stand::new(STREAM);
    o.delay_active = true;
    o.active = true;
    o.starting = true;
    assert!(o.begin(&CORE));
    assert_eq!(o.log(), "reset-packets signal-start");
    assert!(o.delay_capturing);
    assert!(!o.starting);
    assert_eq!(o.total_frames, 9);

    let mut o = Stand::new(STREAM);
    o.delay_active = true;
    o.reconnecting = true;
    assert!(o.begin(&CORE));
    assert_eq!(o.log(), "reset-packets signal-reconnect-success");
    assert!(!o.reconnecting);

    let mut o = Stand::new(STREAM);
    o.delay_active = true;
    o.delay_capturing = true;
    o.starting = true;
    assert!(!o.begin(&CORE));
    assert_eq!(o.log(), "");
    assert!(!o.starting);

    let mut o = Stand::new(STREAM);
    o.delay_active = true;
    o.can_begin = false;
    assert!(o.begin(&CORE));
    assert_eq!(o.log(), "reset-packets signal-start");
}

#[test]
fn steps_start_then_begin() {
    let mut o = Stand::new(STREAM);
    assert!(o.start(&CORE));
    assert!(o.starting);
    assert!(o.begin(&CORE));
    assert!(!o.starting);
    assert!(o.active);
    assert_eq!(
        o.log(),
        "can-try service-init wait-stopping info.start frame-counts signal-starting \
         can-begin pair hook batch-size service-activate signal-activate signal-start"
    );

    let mut o = Stand::new(STREAM);
    assert!(o.start(&CORE));
    assert!(!o.start(&CORE));
    assert_eq!(
        o.log(),
        "can-try service-init wait-stopping info.start frame-counts signal-starting \
         warn-already-starting"
    );

    let mut o = Stand::new(STREAM);
    o.start_ok = false;
    assert!(!o.start(&CORE));
    o.start_ok = true;
    assert!(o.start(&CORE));
    assert_eq!(
        o.log(),
        "can-try service-init wait-stopping info.start \
         can-try service-init wait-stopping info.start frame-counts signal-starting"
    );
}

// ------------------------------------------------------------------------
// Edge cases against origin/master's expressions

#[test]
fn edge_flag_rules_every_word() {
    for high in 0..16u32 {
        for low in 0..=0xffffu32 {
            let f = (high << 16) | (high << 28) | low;
            assert_eq!(service(f), (ORIGIN.service)(f), "{f:#x}");
            assert_eq!(init_video(f), (ORIGIN.init_video)(f), "{f:#x}");
            assert_eq!(init_audio(f), (ORIGIN.init_audio)(f), "{f:#x}");
            assert_eq!(reset_raw(f), (ORIGIN.reset_raw)(f), "{f:#x}");
            assert_eq!(pair(f), (ORIGIN.pair)(f), "{f:#x}");
        }
    }
}

#[test]
fn edge_restart_refs_and_delay_ranges() {
    for r in (-1000..=1000).chain([
        i64::MIN,
        i64::MIN + 1,
        i64::from(i32::MIN),
        i64::from(i32::MAX),
        i64::MAX - 1,
        i64::MAX,
    ]) {
        assert_eq!(drop_restart_ref(r), (ORIGIN.drop_restart_ref)(r), "{r}");
    }
    for d in (0..=100_000u32).chain(u32::MAX - 1000..=u32::MAX) {
        assert_eq!(delay_on(d), (ORIGIN.delay_on)(d), "{d}");
    }
}

#[test]
fn edge_conversion_fill_every_combination() {
    let formats = [
        i32::MIN,
        -1,
        FORMAT_UNKNOWN,
        1,
        FORMAT_16BIT,
        8,
        9,
        i32::MAX,
    ];
    let speakers = [
        i32::MIN,
        -1,
        SPEAKERS_UNKNOWN,
        1,
        SPEAKERS_STEREO,
        8,
        9,
        i32::MAX,
    ];
    for &cf in &formats {
        for &cr in &EDGE_U32 {
            for &cs in &speakers {
                for &mf in &formats {
                    for &mr in &[0, 44100, u32::MAX] {
                        for &ms in &speakers {
                            let conv = fill(cf, cr, cs);
                            let mix = fill(mf, mr, ms);
                            assert_eq!(
                                audio_defaults(conv, mix),
                                (ORIGIN.audio_defaults)(conv, mix)
                            );
                        }
                    }
                }
            }
        }
    }
}

/// Every caller, in turn, on a copy.
pub(crate) fn run_all(o: &Stand, g: &Gates) -> (Vec<bool>, Vec<Stand>) {
    let mut rets = Vec::new();
    let mut ends = Vec::new();
    for which in 0..7 {
        let mut s = o.clone();
        rets.push(call(&mut s, g, which));
        ends.push(s);
    }
    (rets, ends)
}

pub(crate) fn call(s: &mut Stand, g: &Gates, which: u8) -> bool {
    match which {
        0 => s.start(g),
        1 => s.actual_start(g),
        2 => s.initialize_encoders(g),
        3 => s.begin(g),
        4 => s.begin_internal(g),
        5 => s.begin_delayed_capture(),
        _ => {
            s.reset_raw_output(g);
            true
        }
    }
}

/// The stand-in with each atomic from `bits` (starting, active,
/// reconnecting, delay_active, delay_capturing, data_active), restart refs
/// and one call result made to fail (`fail`, 0 for none).
pub(crate) fn stand_for(flags: u32, bits: u32, refs: i64, delay_sec: u32, fail: u32) -> Stand {
    let mut o = Stand::new(flags);
    o.starting = bits & 1 != 0;
    o.active = bits & 2 != 0;
    o.reconnecting = bits & 4 != 0;
    o.delay_active = bits & 8 != 0;
    o.delay_capturing = bits & 16 != 0;
    o.data_active = bits & 32 != 0;
    o.delay_restart_refs = refs;
    o.delay_sec = delay_sec;
    o.valid = fail != 1;
    o.can_try = fail != 2;
    o.service_init_ok = fail != 3;
    o.start_ok = fail != 4;
    o.delay_start_ok = fail != 5;
    o.init_video_ok = fail != 6;
    o.init_audio_ok = fail != 7;
    o.can_begin = fail != 8;
    o.has_data = fail != 9;
    o.has_audio = fail != 10;
    o.audio_conversion_set = fail != 11;
    o.conv = fill(FORMAT_16BIT, 0, SPEAKERS_MONO);
    o.has_last_error = bits & 1 == 0;
    o.tracks[1].present = true;
    o.tracks[1].caption_data_size = 3;
    o
}

#[test]
fn edge_stand_in_every_route() {
    // every low flag word, each atomic set and clear, restart refs, a
    // delay or none, and each call result failing alone, through every
    // caller
    for flags in 0..16u32 {
        for bits in 0..64u32 {
            for refs in [0, 1, -1] {
                for delay_sec in [0, 5] {
                    for fail in 0..12u32 {
                        let o = stand_for(flags, bits, refs, delay_sec, fail);
                        assert_eq!(run_all(&o, &CORE), run_all(&o, &ORIGIN));
                    }
                }
            }
        }
    }
}

// ------------------------------------------------------------------------
// Proptests

#[derive(Clone, Debug)]
pub(crate) enum Call {
    Caller(u8),
    /// another thread: one atomic, the restart refs or a call result
    Flip(u8, i64),
}

pub(crate) fn step() -> impl Strategy<Value = Call> {
    prop_oneof![
        3 => (0u8..7).prop_map(Call::Caller),
        2 => (0u8..16, prop_oneof![Just(0i64), Just(1), Just(-1), any::<i64>()])
            .prop_map(|(w, v)| Call::Flip(w, v)),
    ]
}

pub(crate) fn apply(s: &mut Stand, g: &Gates, c: &Call) -> Option<bool> {
    match c {
        Call::Caller(which) => return Some(call(s, g, *which)),
        Call::Flip(0, _) => s.starting = !s.starting,
        Call::Flip(1, _) => s.active = !s.active,
        Call::Flip(2, _) => s.reconnecting = !s.reconnecting,
        Call::Flip(3, _) => s.delay_active = !s.delay_active,
        Call::Flip(4, _) => s.delay_capturing = !s.delay_capturing,
        Call::Flip(5, _) => s.data_active = !s.data_active,
        Call::Flip(6, v) => s.delay_restart_refs = *v,
        Call::Flip(7, _) => s.can_try = !s.can_try,
        Call::Flip(8, _) => s.service_init_ok = !s.service_init_ok,
        Call::Flip(9, _) => s.start_ok = !s.start_ok,
        Call::Flip(10, _) => s.init_video_ok = !s.init_video_ok,
        Call::Flip(11, _) => s.init_audio_ok = !s.init_audio_ok,
        Call::Flip(12, _) => s.can_begin = !s.can_begin,
        Call::Flip(13, _) => s.delay_start_ok = !s.delay_start_ok,
        Call::Flip(14, v) => s.delay_sec = u32::try_from(v.rem_euclid(3)).unwrap_or(0),
        Call::Flip(_, _) => s.audio_conversion_set = !s.audio_conversion_set,
    }
    None
}

pub(crate) fn audio_fill() -> impl Strategy<Value = AudioFill> {
    (
        prop_oneof![Just(FORMAT_UNKNOWN), 0..10i32, any::<i32>()],
        prop_oneof![Just(0u32), Just(48000), any::<u32>()],
        prop_oneof![Just(SPEAKERS_UNKNOWN), 0..10i32, any::<i32>()],
    )
        .prop_map(|(format, samples_per_sec, speakers)| AudioFill {
            format,
            samples_per_sec,
            speakers,
        })
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 4096,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn edge_proptest_decisions(
        f in any::<u32>(),
        r in any::<i64>(),
        d in any::<u32>(),
        conv in audio_fill(),
        mix in audio_fill(),
    ) {
        prop_assert_eq!(service(f), (ORIGIN.service)(f));
        prop_assert_eq!(init_video(f), (ORIGIN.init_video)(f));
        prop_assert_eq!(init_audio(f), (ORIGIN.init_audio)(f));
        prop_assert_eq!(reset_raw(f), (ORIGIN.reset_raw)(f));
        prop_assert_eq!(pair(f), (ORIGIN.pair)(f));
        prop_assert_eq!(drop_restart_ref(r), (ORIGIN.drop_restart_ref)(r));
        prop_assert_eq!(delay_on(d), (ORIGIN.delay_on)(d));
        prop_assert_eq!(audio_defaults(conv, mix), (ORIGIN.audio_defaults)(conv, mix));
    }

    #[test]
    fn edge_proptest_stand_in(
        flags in prop_oneof![0u32..16, any::<u32>()],
        bits in 0u32..64,
        refs in prop_oneof![Just(0i64), Just(1), Just(2), any::<i64>()],
        delay_sec in prop_oneof![Just(0u32), Just(5), any::<u32>()],
        conv in audio_fill(),
        mix in audio_fill(),
        calls in proptest::collection::vec(step(), 1..12),
    ) {
        let mut a = stand_for(flags, bits, refs, delay_sec, 0);
        a.conv = conv;
        a.mix = mix;
        let mut b = a.clone();
        for c in &calls {
            prop_assert_eq!(apply(&mut a, &CORE, c), apply(&mut b, &ORIGIN, c));
            prop_assert_eq!(&a, &b);
        }
    }
}
