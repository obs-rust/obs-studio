//! Tier 1: `obs_output::output_stop`, the output stop and end-capture
//! decisions.
//!
//! The `steps_*` cases port `test/cmocka/test_output_stop_steps.c` 1:1: its
//! 19 cases under the same names, in the same order, with the same values
//! and expectations, the "characterized, not endorsed" ones included. The C
//! calls the hidden `obs_output_stop_*`, `obs_output_raw_audio_interleave`,
//! `obs_output_raw_audio_mix_on` and `obs_output_first_mixer` functions;
//! here the safe core is called directly. The C's stand-in output and its
//! copies of `obs_output_stop()`, `obs_output_force_stop()`,
//! `obs_output_actual_stop()`, `stop_raw_audio()`,
//! `end_data_capture_thread()`, `shutdown_unstarted_encoders_thread()`,
//! `obs_output_end_data_capture_internal()`, `obs_output_end_data_capture()`
//! and `obs_output_signal_stop()` are [`Stand`].
//!
//! The `edge_*` cases are new: every decision against the origin/master
//! expressions inline over every low 16-bit flag word under 16 high words,
//! every restart count and thread result from -1,000 to 1,000 and the
//! extremes, every delay to 100,000 and the top of `u64`, every mixer mask
//! to 4,096, the stand-in through every caller for every low flag word,
//! each atomic and the stopping event set and clear, a restart reference
//! or none, a delay or none and each call result changed alone, and
//! proptests of every decision over full-range values and of the stand-in
//! over call sequences, with another thread flipping the atomics, the
//! events, the delay, the restart refs and the call results, or running a
//! created thread, between them.

use obs_output::output_stop::{
    OBS_OUTPUT_AUDIO, OBS_OUTPUT_ENCODED, OBS_OUTPUT_SERVICE, OBS_OUTPUT_VIDEO, delay_on, encoded,
    end_delay, first_mixer, has_audio, has_video, interleave, mix_on, service, thread_failed,
};
use proptest::prelude::*;

pub(crate) const F_VIDEO: u32 = 1;
pub(crate) const F_AUDIO: u32 = 2;
pub(crate) const F_ENCODED: u32 = 4;
pub(crate) const F_SERVICE: u32 = 8;

pub(crate) const STREAM: u32 = F_VIDEO | F_AUDIO | F_ENCODED | F_SERVICE;
pub(crate) const RECORDING: u32 = F_VIDEO | F_AUDIO | F_ENCODED;
pub(crate) const RAW_VIDEO: u32 = F_VIDEO;
pub(crate) const RAW_AV: u32 = F_VIDEO | F_AUDIO;

pub(crate) const CODE_SUCCESS: i32 = 0;
pub(crate) const CODE_DISCONNECTED: i32 = -2;
pub(crate) const CODE_ERROR: i32 = -4;

/// `EAGAIN` and `EINVAL` (Linux): the C test uses the host's, any error
/// number is not 0.
pub(crate) const EAGAIN: i32 = 11;
pub(crate) const EINVAL: i32 = 22;

const EDGE_LONG: [i64; 8] = [i64::MIN, -2, -1, 0, 1, 2, 3, i64::MAX];
const EDGE_U64: [u64; 7] = [
    0,
    1,
    2,
    1_000_000_000,
    20_000_000_000,
    0x7fff_ffff_ffff_ffff,
    u64::MAX,
];
const EDGE_INT: [i32; 7] = [i32::MIN, -1, 0, 1, EAGAIN, EINVAL, i32::MAX];

pub(crate) const SLOTS: usize = 2;

// ------------------------------------------------------------------------
// The gates the callers ask, so the stand-in runs on the core ([`CORE`])
// or on the origin/master expressions inline ([`ORIGIN`]).

pub(crate) struct Gates {
    pub(crate) encoded: fn(u32) -> bool,
    pub(crate) delay_on: fn(u64) -> bool,
    pub(crate) has_video: fn(u32) -> bool,
    pub(crate) has_audio: fn(u32) -> bool,
    pub(crate) service: fn(u32) -> bool,
    pub(crate) end_delay: fn(i64) -> bool,
    pub(crate) thread_failed: fn(i32) -> bool,
    pub(crate) interleave: fn(bool, bool) -> bool,
    pub(crate) mix_on: fn(usize, usize) -> bool,
    pub(crate) first_mixer: fn(usize) -> usize,
}

pub(crate) const CORE: Gates = Gates {
    encoded,
    delay_on,
    has_video,
    has_audio,
    service,
    end_delay,
    thread_failed,
    interleave,
    mix_on,
    first_mixer,
};

/// origin/master's obs-output.c, its expressions inline: `flag_encoded()`,
/// `active_delay_ns`'s truth, `flag_video()`, `flag_audio()`,
/// `flag_service()`, the `delay_restart_refs` load's falsity, `ret != 0`,
/// `has_video && has_audio`, the mask test and `get_first_mixer()`'s loop.
pub(crate) const ORIGIN: Gates = Gates {
    encoded: |f| (f & OBS_OUTPUT_ENCODED) != 0,
    delay_on: |d| d != 0,
    has_video: |f| (f & OBS_OUTPUT_VIDEO) != 0,
    has_audio: |f| (f & OBS_OUTPUT_AUDIO) != 0,
    service: |f| (f & OBS_OUTPUT_SERVICE) != 0,
    end_delay: |r| r == 0,
    thread_failed: |r| r != 0,
    interleave: |v, a| v && a,
    mix_on: |mask, idx| (mask & (1usize << idx)) != 0,
    first_mixer: |mask| (0..6).find(|i| (1usize << i) & mask != 0).unwrap_or(0),
};

// ------------------------------------------------------------------------
// The stand-in output and the callers, as the C test's.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Track {
    pub(crate) present: bool,
    pub(crate) captions: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pending {
    None,
    End,
    Unstarted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Stand {
    // the output
    pub(crate) has_data: bool,
    pub(crate) flags: u32,
    pub(crate) raw_audio2: bool,
    pub(crate) mixer_mask: usize,
    pub(crate) active_delay_ns: u64,
    pub(crate) video_enc: [bool; SLOTS],
    pub(crate) audio_enc: [bool; SLOTS],
    pub(crate) tracks: [Track; SLOTS],
    pub(crate) keyframes: usize,
    pub(crate) stop_code: i32,
    // the atomics and events
    pub(crate) active: bool,
    pub(crate) data_active: bool,
    pub(crate) reconnecting: bool,
    pub(crate) delay_active: bool,
    pub(crate) delay_capturing: bool,
    pub(crate) starting: bool,
    pub(crate) thread_active: bool,
    pub(crate) thread_joinable: bool,
    pub(crate) delay_restart_refs: i64,
    pub(crate) stopping_set: bool,
    pub(crate) reconnect_stop_set: bool,
    pub(crate) reconnect_thread_active: bool,
    // the results of the calls the C keeps
    pub(crate) valid: bool,
    pub(crate) create_result: i32,
    pub(crate) can_reconnect: bool,
    // a created thread that has not run yet
    pub(crate) pending: Pending,
    // the record
    pub(crate) log: Vec<String>,
}

impl Stand {
    pub(crate) fn new(flags: u32) -> Self {
        let mut tracks = [Track::default(); SLOTS];
        tracks[0] = Track {
            present: true,
            captions: 2,
        };
        Self {
            has_data: true,
            flags,
            raw_audio2: false,
            mixer_mask: 0,
            active_delay_ns: 0,
            video_enc: [true, false],
            audio_enc: [true, false],
            tracks,
            keyframes: 3,
            stop_code: CODE_ERROR,
            valid: true,
            // a running output: active, its data active, the stopping event
            // set (it is reset only while a stop is under way)
            active: true,
            data_active: true,
            reconnecting: false,
            delay_active: false,
            delay_capturing: false,
            starting: true,
            thread_active: false,
            thread_joinable: false,
            delay_restart_refs: 0,
            stopping_set: true,
            reconnect_stop_set: false,
            reconnect_thread_active: false,
            create_result: 0,
            can_reconnect: false,
            pending: Pending::None,
            log: Vec::new(),
        }
    }

    pub(crate) fn log(&self) -> String {
        self.log.join(" ")
    }

    fn rec(&mut self, what: impl Into<String>) {
        self.log.push(what.into());
    }

    fn clear_log(&mut self) {
        self.log.clear();
    }

    /// `stopping()`: the stopping event is not set
    fn stopping(&self) -> bool {
        !self.stopping_set
    }

    /// `obs_output_actual_stop()`
    pub(crate) fn actual_stop(&mut self, g: &Gates, force: bool, ts: u64) {
        let mut call_stop = true;

        if self.stopping() && !force {
            return;
        }

        self.rec("unpause");

        self.stopping_set = false;
        self.rec("reset-stopping");

        let was_reconnecting = self.reconnecting && !self.delay_active;
        if self.reconnecting {
            self.reconnect_stop_set = true;
            self.rec("set-reconnect-stop");
            if self.reconnect_thread_active {
                self.rec("join-reconnect");
            }
        }

        if force {
            if self.delay_active {
                call_stop = self.delay_capturing;
                self.delay_active = false;
                self.delay_capturing = false;
                self.stop_code = CODE_SUCCESS;
                self.end_data_capture(g);
                self.stopping_set = true;
                self.rec("set-stopping");
            } else {
                call_stop = true;
            }
        }

        if self.has_data && call_stop {
            self.rec(format!("info.stop-{ts}"));
        } else if was_reconnecting {
            self.stop_code = CODE_SUCCESS;
            self.rec(format!("signal-stop-{}", self.stop_code));
            self.stopping_set = true;
            self.rec("set-stopping");
        }

        self.starting = false;

        for i in 0..SLOTS {
            if !self.tracks[i].present {
                continue;
            }
            let captions = self.tracks[i].captions;
            self.rec(format!("free-captions-{i}-{captions}"));
            self.tracks[i].captions = 0;
        }

        self.keyframes = 0;
    }

    /// `obs_output_stop()`
    pub(crate) fn stop(&mut self, g: &Gates) {
        if !self.valid {
            return;
        }
        if !self.has_data {
            return;
        }
        if !self.active && !self.reconnecting {
            return;
        }
        if self.reconnecting {
            self.force_stop(g);
            return;
        }

        if (g.encoded)(self.flags) && (g.delay_on)(self.active_delay_ns) {
            self.rec("delay-stop");
        } else if !self.stopping() {
            self.rec("signal-stopping");
            self.actual_stop(g, false, 7);
        }
    }

    /// `obs_output_force_stop()`
    pub(crate) fn force_stop(&mut self, g: &Gates) {
        if !self.valid {
            return;
        }

        if !self.stopping() {
            self.stop_code = 0;
            self.rec("signal-stopping");
        }
        self.actual_stop(g, true, 0);
    }

    fn callback_name(&self, g: &Gates, has_video: bool, has_audio: bool) -> &'static str {
        if (g.delay_on)(self.active_delay_ns) {
            "delay"
        } else if (g.interleave)(has_video, has_audio) {
            "interleave"
        } else {
            "default"
        }
    }

    /// `stop_raw_audio()`
    fn stop_raw_audio(&mut self, g: &Gates) {
        if self.raw_audio2 {
            for idx in 0..6 {
                if (g.mix_on)(self.mixer_mask, idx) {
                    self.rec(format!("disconnect-mix-{idx}"));
                }
            }
        } else {
            let first = (g.first_mixer)(self.mixer_mask);
            self.rec(format!("disconnect-mix-{first}"));
        }
    }

    /// `end_data_capture_thread()`
    pub(crate) fn end_data_capture_thread(&mut self, g: &Gates) {
        let has_video = (g.has_video)(self.flags);
        let has_audio = (g.has_audio)(self.flags);

        if (g.encoded)(self.flags) {
            let cb = self.callback_name(g, has_video, has_audio);

            if has_video {
                for i in 0..SLOTS {
                    if self.video_enc[i] {
                        self.rec(format!("stop-video-{i}-{cb}"));
                    }
                }
            }
            if has_audio {
                for i in 0..SLOTS {
                    if self.audio_enc[i] {
                        self.rec(format!("stop-audio-{i}-{cb}"));
                    }
                }
            }
        } else {
            if has_video {
                self.rec("stop-raw-video");
            }
            if has_audio {
                self.stop_raw_audio(g);
            }
        }

        if (g.service)(self.flags) {
            self.rec("deactivate-service");
        }

        if (g.delay_on)(self.active_delay_ns) {
            self.rec("cleanup-delay");
        }

        self.active = false;
        self.rec("signal-deactivate");
        self.stopping_set = true;
        self.rec("set-stopping");
        self.thread_active = false;
    }

    /// `shutdown_unstarted_encoders_thread()`
    fn shutdown_unstarted_thread(&mut self) {
        for i in 0..SLOTS {
            if self.video_enc[i] {
                self.rec(format!("shutdown-video-{i}"));
            }
        }
        for i in 0..SLOTS {
            if self.audio_enc[i] {
                self.rec(format!("shutdown-audio-{i}"));
            }
        }
        self.thread_active = false;
    }

    /// the created thread runs
    pub(crate) fn run_thread(&mut self, g: &Gates) {
        let p = self.pending;
        self.pending = Pending::None;
        match p {
            Pending::End => self.end_data_capture_thread(g),
            Pending::Unstarted => self.shutdown_unstarted_thread(),
            Pending::None => {}
        }
    }

    /// `join_end_data_capture_thread()`
    fn join_end_thread(&mut self) {
        let joinable = self.thread_joinable;
        self.thread_joinable = false;
        if joinable {
            self.rec("join-end-thread");
        }
    }

    /// `obs_output_end_data_capture_internal()`
    pub(crate) fn end_data_capture_internal(&mut self, g: &Gates, signal: bool) {
        if !self.valid {
            return;
        }

        self.starting = false;

        if !self.active || !self.data_active {
            if signal {
                let params_code = self.stop_code;
                self.rec("stop-params");
                if !self.active && (g.encoded)(self.flags) && !self.thread_active {
                    self.join_end_thread();
                    self.thread_active = true;
                    let ret = self.create_result;
                    self.rec("create-unstarted-thread");
                    if (g.thread_failed)(ret) {
                        self.thread_active = false;
                    } else {
                        self.thread_joinable = true;
                        self.pending = Pending::Unstarted;
                    }
                }
                self.rec(format!("signal-stop-{params_code}"));
                self.stop_code = CODE_SUCCESS;
                self.stopping_set = true;
                self.rec("set-stopping");
            }
            return;
        }

        if self.delay_active {
            self.delay_capturing = false;

            if (g.end_delay)(self.delay_restart_refs) {
                self.delay_active = false;
            } else {
                self.stopping_set = true;
                self.rec("set-stopping");
                return;
            }
        }

        self.data_active = false;

        if (g.has_video)(self.flags) {
            self.rec("log-frames");
        }

        self.join_end_thread();

        let mut params_code = 0;
        if signal {
            params_code = self.stop_code;
            self.rec("stop-params");
        }

        self.thread_active = true;
        let ret = self.create_result;
        self.rec("create-end-thread");
        if (g.thread_failed)(ret) {
            self.rec("warn-thread");
            self.end_data_capture_thread(g);
        } else {
            self.thread_joinable = true;
            self.pending = Pending::End;
        }

        if signal {
            self.rec(format!("signal-stop-{params_code}"));
            self.stop_code = CODE_SUCCESS;
        }
    }

    /// `obs_output_end_data_capture()`
    pub(crate) fn end_data_capture(&mut self, g: &Gates) {
        self.end_data_capture_internal(g, true);
    }

    /// `obs_output_signal_stop()`
    pub(crate) fn signal_stop(&mut self, g: &Gates, code: i32) {
        if !self.valid {
            return;
        }

        self.stop_code = code;

        if self.can_reconnect {
            if self.delay_active {
                self.delay_restart_refs = self.delay_restart_refs.wrapping_add(1);
            }
            self.end_data_capture_internal(g, false);
            self.rec("reconnect");
        } else {
            if self.delay_active {
                self.delay_capturing = false;
                self.delay_active = false;
            }
            if self.reconnecting {
                self.reconnecting = false;
            }
            self.end_data_capture(g);
        }
    }
}

// ------------------------------------------------------------------------
// test_output_stop_steps.c, 1:1

#[test]
fn steps_constants() {
    // OBS_OUTPUT_VIDEO, _AUDIO, _ENCODED, _SERVICE
    assert_eq!(OBS_OUTPUT_VIDEO, 1);
    assert_eq!(OBS_OUTPUT_AUDIO, 2);
    assert_eq!(OBS_OUTPUT_ENCODED, 4);
    assert_eq!(OBS_OUTPUT_SERVICE, 8);
}

#[test]
fn steps_flag_rules() {
    // every low 16-bit flag word, with and without high bits: each rule
    // looks at its own bit only
    for high in [0, 0x10000u32, 0x8000_0000, 0xffff_0000] {
        for low in 0..=0xffffu32 {
            let f = high | low;
            assert_eq!(encoded(f), (f & F_ENCODED) != 0);
            assert_eq!(has_video(f), (f & F_VIDEO) != 0);
            assert_eq!(has_audio(f), (f & F_AUDIO) != 0);
            assert_eq!(service(f), (f & F_SERVICE) != 0);
        }
    }
    // the common outputs: an encoded stream (AV, encoded, service), a
    // recording (AV, encoded), a raw virtual camera (video only)
    assert!(encoded(STREAM));
    assert!(service(STREAM));
    assert!(encoded(RECORDING));
    assert!(!service(RECORDING));
    assert!(!encoded(RAW_VIDEO));
    assert!(has_video(RAW_VIDEO));
    assert!(!has_audio(RAW_VIDEO));
    assert!(!has_video(0));
    assert!(!has_audio(0));
}

#[test]
fn steps_delay_on() {
    // a delay runs while active_delay_ns is not 0, however short
    assert!(!delay_on(0));
    assert!(delay_on(1));
    for d in EDGE_U64 {
        assert_eq!(delay_on(d), d != 0);
    }
}

#[test]
fn steps_end_delay() {
    // only no restart reference ends the delay
    assert!(end_delay(0));
    assert!(!end_delay(1));
    assert!(!end_delay(2));
    // a negative count keeps the delay too: the test is "0", not "not
    // positive" (characterized, not endorsed; never reached)
    assert!(!end_delay(-1));
    for r in EDGE_LONG {
        assert_eq!(end_delay(r), r == 0);
    }
}

#[test]
fn steps_thread_failed() {
    // pthread_create() returns 0 or an error number; any other value fails
    // too
    assert!(!thread_failed(0));
    assert!(thread_failed(EAGAIN));
    for r in EDGE_INT {
        assert_eq!(thread_failed(r), r != 0);
    }
}

#[test]
fn steps_shared_rules() {
    // end_data_capture_thread(): an undelayed encoded output stops its
    // encoders with interleave_packets() only with both video and audio
    assert!(interleave(true, true));
    assert!(!interleave(true, false));
    assert!(!interleave(false, true));
    assert!(!interleave(false, false));

    // stop_raw_audio(): each mix whose bit is set, with raw_audio2
    for mask in 0..64usize {
        for idx in 0..6usize {
            assert_eq!(mix_on(mask, idx), (mask & (1 << idx)) != 0);
        }
    }
    // bits past the mixes are never asked (MAX_AUDIO_MIXES is 6)
    assert!(!mix_on(1 << 6, 0));

    // without raw_audio2: the lowest mix whose bit is set, 0 for none
    assert_eq!(first_mixer(0), 0);
    assert_eq!(first_mixer(1), 0);
    assert_eq!(first_mixer(6), 1);
    assert_eq!(first_mixer(0x20), 5);
    assert_eq!(first_mixer(0x40), 0);
}

#[test]
fn steps_stop_routes() {
    let g = &CORE;

    // a stream: "stopping", then the plugin's stop
    let mut o = Stand::new(STREAM);
    o.stop(g);
    assert_eq!(
        o.log(),
        "signal-stopping unpause reset-stopping info.stop-7 free-captions-0-2"
    );
    assert!(!o.stopping_set);
    assert!(!o.starting);
    assert_eq!(o.keyframes, 0);
    assert_eq!(o.tracks[0].captions, 0);
    // the stop code is left for the plugin's stop to set
    assert_eq!(o.stop_code, CODE_ERROR);
    // still active: only the end of the capture deactivates it
    assert!(o.active);

    // a raw output stops the same way
    let mut o = Stand::new(RAW_VIDEO);
    o.stop(g);
    assert_eq!(
        o.log(),
        "signal-stopping unpause reset-stopping info.stop-7 free-captions-0-2"
    );

    // invalid, no data, or neither active nor reconnecting: nothing
    let mut o = Stand::new(STREAM);
    o.valid = false;
    o.stop(g);
    assert_eq!(o.log(), "");
    let mut o = Stand::new(STREAM);
    o.has_data = false;
    o.stop(g);
    assert_eq!(o.log(), "");
    let mut o = Stand::new(STREAM);
    o.active = false;
    o.stop(g);
    assert_eq!(o.log(), "");
    assert!(o.starting);

    // already stopping: nothing, not even "stopping"
    let mut o = Stand::new(STREAM);
    o.stopping_set = false;
    o.stop(g);
    assert_eq!(o.log(), "");

    // data_active, delay_active and delay_capturing do not change the route
    // (characterized, not endorsed: a delay without active_delay_ns stops
    // at once)
    let mut o = Stand::new(RECORDING);
    o.data_active = false;
    o.delay_active = true;
    o.delay_capturing = true;
    o.stop(g);
    assert_eq!(
        o.log(),
        "signal-stopping unpause reset-stopping info.stop-7 free-captions-0-2"
    );
    assert!(o.delay_active);
}

#[test]
fn steps_stop_delayed() {
    let g = &CORE;

    // an encoded output with a delay running stops through the delay, even
    // while stopping
    let mut o = Stand::new(STREAM);
    o.active_delay_ns = 20_000_000_000;
    o.stop(g);
    assert_eq!(o.log(), "delay-stop");
    assert!(o.starting);
    let mut o = Stand::new(RECORDING);
    o.active_delay_ns = 1;
    o.stopping_set = false;
    o.stop(g);
    assert_eq!(o.log(), "delay-stop");

    // a raw output with active_delay_ns stops at once (characterized, not
    // endorsed: only an encoded output sets it)
    let mut o = Stand::new(RAW_AV);
    o.active_delay_ns = 1;
    o.stop(g);
    assert_eq!(
        o.log(),
        "signal-stopping unpause reset-stopping info.stop-7 free-captions-0-2"
    );

    // reconnecting comes first: a delayed output that is reconnecting is
    // force stopped
    let mut o = Stand::new(STREAM);
    o.active_delay_ns = 1;
    o.reconnecting = true;
    o.stop(g);
    assert_eq!(
        o.log(),
        "signal-stopping unpause reset-stopping set-reconnect-stop info.stop-0 free-captions-0-2"
    );
}

#[test]
fn steps_stop_reconnecting() {
    let g = &CORE;

    // reconnecting with data: a forced stop, the plugin's stop runs
    let mut o = Stand::new(STREAM);
    o.active = false;
    o.reconnecting = true;
    o.reconnect_thread_active = true;
    o.stop(g);
    assert_eq!(
        o.log(),
        "signal-stopping unpause reset-stopping set-reconnect-stop join-reconnect info.stop-0 \
         free-captions-0-2"
    );
    assert_eq!(o.stop_code, CODE_SUCCESS);
    assert!(o.reconnect_stop_set);
    // the reconnect thread clears reconnecting itself
    assert!(o.reconnecting);

    // reconnecting, the thread already gone: no join
    let mut o = Stand::new(STREAM);
    o.reconnecting = true;
    o.stop(g);
    assert_eq!(
        o.log(),
        "signal-stopping unpause reset-stopping set-reconnect-stop info.stop-0 free-captions-0-2"
    );

    // reconnecting without data: obs_output_stop() refuses before
    let mut o = Stand::new(STREAM);
    o.reconnecting = true;
    o.has_data = false;
    o.stop(g);
    assert_eq!(o.log(), "");

    // forced without data while reconnecting: the stop is signalled here
    let mut o = Stand::new(STREAM);
    o.reconnecting = true;
    o.has_data = false;
    o.force_stop(g);
    assert_eq!(
        o.log(),
        "signal-stopping unpause reset-stopping set-reconnect-stop signal-stop-0 set-stopping \
         free-captions-0-2"
    );
    assert!(o.stopping_set);

    // reconnecting with a delay active: not "was reconnecting"; the forced
    // stop clears the delay before it ends the capture, so the end takes the
    // full route whatever the restart refs
    let mut o = Stand::new(STREAM);
    o.reconnecting = true;
    o.has_data = false;
    o.delay_active = true;
    o.delay_restart_refs = 1;
    o.force_stop(g);
    assert_eq!(
        o.log(),
        "signal-stopping unpause reset-stopping set-reconnect-stop log-frames stop-params \
         create-end-thread signal-stop-0 set-stopping free-captions-0-2"
    );
    assert!(!o.delay_active);
    assert_eq!(o.delay_restart_refs, 1);
}

#[test]
fn steps_force_stop() {
    let g = &CORE;

    // not stopping: the code is cleared and "stopping" signalled
    let mut o = Stand::new(STREAM);
    o.force_stop(g);
    assert_eq!(
        o.log(),
        "signal-stopping unpause reset-stopping info.stop-0 free-captions-0-2"
    );
    assert_eq!(o.stop_code, CODE_SUCCESS);

    // stopping: no signal and the code kept, the stop goes on
    let mut o = Stand::new(STREAM);
    o.stopping_set = false;
    o.force_stop(g);
    assert_eq!(
        o.log(),
        "unpause reset-stopping info.stop-0 free-captions-0-2"
    );
    assert_eq!(o.stop_code, CODE_ERROR);

    // not active: a forced stop still calls the plugin's stop
    // (characterized, not endorsed)
    let mut o = Stand::new(STREAM);
    o.active = false;
    o.data_active = false;
    o.force_stop(g);
    assert_eq!(
        o.log(),
        "signal-stopping unpause reset-stopping info.stop-0 free-captions-0-2"
    );

    // invalid: nothing
    let mut o = Stand::new(STREAM);
    o.valid = false;
    o.force_stop(g);
    assert_eq!(o.log(), "");
}

#[test]
fn steps_actual_stop_refusal() {
    let g = &CORE;

    // not forced while stopping: refused, nothing cleared
    let mut o = Stand::new(STREAM);
    o.stopping_set = false;
    o.actual_stop(g, false, 5);
    assert_eq!(o.log(), "");
    assert!(o.starting);
    assert_eq!(o.keyframes, 3);

    // forced while stopping: goes on
    let mut o = Stand::new(STREAM);
    o.stopping_set = false;
    o.actual_stop(g, true, 5);
    assert_eq!(
        o.log(),
        "unpause reset-stopping info.stop-5 free-captions-0-2"
    );

    // each caption track present is freed, the others skipped
    let mut o = Stand::new(STREAM);
    o.tracks[0].present = false;
    o.tracks[1].present = true;
    o.tracks[1].captions = 0;
    o.actual_stop(g, false, 5);
    assert_eq!(
        o.log(),
        "unpause reset-stopping info.stop-5 free-captions-1-0"
    );
}

#[test]
fn steps_actual_stop_forced_delay() {
    let g = &CORE;

    // a delayed output that is capturing: the capture ends (the end-capture
    // thread is created), then the plugin's stop
    let mut o = Stand::new(STREAM);
    o.delay_active = true;
    o.delay_capturing = true;
    o.actual_stop(g, true, 0);
    assert_eq!(
        o.log(),
        "unpause reset-stopping log-frames stop-params create-end-thread signal-stop-0 \
         set-stopping info.stop-0 free-captions-0-2"
    );
    assert!(!o.delay_active);
    assert!(!o.delay_capturing);
    assert_eq!(o.pending, Pending::End);

    // not capturing yet: the plugin's stop is not called
    let mut o = Stand::new(STREAM);
    o.delay_active = true;
    o.actual_stop(g, true, 0);
    assert_eq!(
        o.log(),
        "unpause reset-stopping log-frames stop-params create-end-thread signal-stop-0 \
         set-stopping free-captions-0-2"
    );

    // not forced: the delay is left alone
    let mut o = Stand::new(STREAM);
    o.delay_active = true;
    o.actual_stop(g, false, 9);
    assert_eq!(
        o.log(),
        "unpause reset-stopping info.stop-9 free-captions-0-2"
    );
    assert!(o.delay_active);

    // forced, delayed, data not active: the end signals the stop and shuts
    // nothing down (still active)
    let mut o = Stand::new(STREAM);
    o.delay_active = true;
    o.data_active = false;
    o.actual_stop(g, true, 0);
    assert_eq!(
        o.log(),
        "unpause reset-stopping stop-params signal-stop-0 set-stopping set-stopping \
         free-captions-0-2"
    );
}

#[test]
fn steps_end_not_active() {
    let g = &CORE;

    // an encoded output stopped before its capture began: the stop is
    // signalled with the code copied first, and the encoders it initialized
    // are shut down on a thread
    let mut o = Stand::new(STREAM);
    o.active = false;
    o.data_active = false;
    o.thread_joinable = true;
    o.end_data_capture(g);
    assert_eq!(
        o.log(),
        "stop-params join-end-thread create-unstarted-thread signal-stop--4 set-stopping"
    );
    assert_eq!(o.stop_code, CODE_SUCCESS);
    assert!(!o.starting);
    assert!(o.thread_active);
    assert!(o.thread_joinable);
    o.run_thread(g);
    assert_eq!(
        o.log(),
        "stop-params join-end-thread create-unstarted-thread signal-stop--4 set-stopping \
         shutdown-video-0 shutdown-audio-0"
    );
    assert!(!o.thread_active);

    // no end signal (a reconnect): only starting is cleared
    let mut o = Stand::new(STREAM);
    o.active = false;
    o.end_data_capture_internal(g, false);
    assert_eq!(o.log(), "");
    assert!(!o.starting);
    assert_eq!(o.stop_code, CODE_ERROR);

    // a raw output: no thread
    let mut o = Stand::new(RAW_AV);
    o.active = false;
    o.end_data_capture(g);
    assert_eq!(o.log(), "stop-params signal-stop--4 set-stopping");

    // an end-capture thread still running: no second one
    let mut o = Stand::new(RECORDING);
    o.active = false;
    o.thread_active = true;
    o.end_data_capture(g);
    assert_eq!(o.log(), "stop-params signal-stop--4 set-stopping");

    // active but its data not active (a stream still connecting): signalled,
    // no thread (its encoders belong to the capture that never began)
    let mut o = Stand::new(STREAM);
    o.data_active = false;
    o.end_data_capture(g);
    assert_eq!(o.log(), "stop-params signal-stop--4 set-stopping");

    // the thread cannot be created: the flag is cleared, the stop signalled
    let mut o = Stand::new(STREAM);
    o.active = false;
    o.create_result = EAGAIN;
    o.end_data_capture(g);
    assert_eq!(
        o.log(),
        "stop-params create-unstarted-thread signal-stop--4 set-stopping"
    );
    assert!(!o.thread_active);
    assert!(!o.thread_joinable);
    assert_eq!(o.pending, Pending::None);

    // invalid: nothing, starting kept
    let mut o = Stand::new(STREAM);
    o.valid = false;
    o.end_data_capture(g);
    assert_eq!(o.log(), "");
    assert!(o.starting);
}

#[test]
fn steps_end_delay_route() {
    let g = &CORE;

    // no restart reference: the delay ends with the capture
    let mut o = Stand::new(STREAM);
    o.delay_active = true;
    o.delay_capturing = true;
    o.end_data_capture(g);
    assert_eq!(
        o.log(),
        "log-frames stop-params create-end-thread signal-stop--4"
    );
    assert!(!o.delay_active);
    assert!(!o.delay_capturing);
    assert!(!o.data_active);

    // a restart reference keeps the delay: the stopping event is set and the
    // capture goes on
    let mut o = Stand::new(STREAM);
    o.delay_active = true;
    o.delay_capturing = true;
    o.delay_restart_refs = 1;
    o.end_data_capture(g);
    assert_eq!(o.log(), "set-stopping");
    assert!(o.delay_active);
    assert!(!o.delay_capturing);
    assert!(o.data_active);
    assert_eq!(o.stop_code, CODE_ERROR);
    assert_eq!(o.delay_restart_refs, 1);

    // a negative count keeps it too (characterized, not endorsed)
    let mut o = Stand::new(STREAM);
    o.delay_active = true;
    o.delay_restart_refs = -1;
    o.end_data_capture(g);
    assert_eq!(o.log(), "set-stopping");
    assert!(o.delay_active);
}

#[test]
fn steps_end_full() {
    let g = &CORE;

    // a stream: the frame log, the last thread joined, the code copied
    // before the thread starts, the stop signalled after
    let mut o = Stand::new(STREAM);
    o.thread_joinable = true;
    o.end_data_capture(g);
    assert_eq!(
        o.log(),
        "log-frames join-end-thread stop-params create-end-thread signal-stop--4"
    );
    assert_eq!(o.stop_code, CODE_SUCCESS);
    assert!(o.active);
    assert!(o.thread_active);
    assert!(o.thread_joinable);
    o.run_thread(g);
    assert_eq!(
        o.log(),
        "log-frames join-end-thread stop-params create-end-thread signal-stop--4 \
         stop-video-0-interleave stop-audio-0-interleave deactivate-service signal-deactivate \
         set-stopping"
    );
    assert!(!o.active);
    assert!(!o.thread_active);
    assert!(o.stopping_set);

    // no end signal: no code copied, none signalled, the code kept
    let mut o = Stand::new(STREAM);
    o.end_data_capture_internal(g, false);
    assert_eq!(o.log(), "log-frames create-end-thread");
    assert_eq!(o.stop_code, CODE_ERROR);

    // an audio-only output: no frame log
    let mut o = Stand::new(F_AUDIO | F_ENCODED);
    o.end_data_capture(g);
    assert_eq!(o.log(), "stop-params create-end-thread signal-stop--4");

    // the thread cannot be created: a warning and the thread's work inline,
    // before the stop is signalled
    let mut o = Stand::new(RECORDING);
    o.create_result = EAGAIN;
    o.end_data_capture(g);
    assert_eq!(
        o.log(),
        "log-frames stop-params create-end-thread warn-thread stop-video-0-interleave \
         stop-audio-0-interleave signal-deactivate set-stopping signal-stop--4"
    );
    assert!(!o.active);
    assert!(!o.thread_active);
    assert!(!o.thread_joinable);
    assert_eq!(o.pending, Pending::None);
}

#[test]
fn steps_end_thread_callbacks() {
    let g = &CORE;

    // a delay running: process_delay(), and the delay cleaned up
    let mut o = Stand::new(STREAM);
    o.active_delay_ns = 1;
    o.video_enc[1] = true;
    o.end_data_capture_thread(g);
    assert_eq!(
        o.log(),
        "stop-video-0-delay stop-video-1-delay stop-audio-0-delay deactivate-service \
         cleanup-delay signal-deactivate set-stopping"
    );

    // video only and audio only: the default callback
    let mut o = Stand::new(F_VIDEO | F_ENCODED);
    o.audio_enc[1] = true;
    o.end_data_capture_thread(g);
    assert_eq!(
        o.log(),
        "stop-video-0-default signal-deactivate set-stopping"
    );
    let mut o = Stand::new(F_AUDIO | F_ENCODED);
    o.audio_enc[1] = true;
    o.end_data_capture_thread(g);
    assert_eq!(
        o.log(),
        "stop-audio-0-default stop-audio-1-default signal-deactivate set-stopping"
    );

    // an encoded output with neither flag stops nothing (characterized, not
    // endorsed)
    let mut o = Stand::new(F_ENCODED | F_SERVICE);
    o.end_data_capture_thread(g);
    assert_eq!(o.log(), "deactivate-service signal-deactivate set-stopping");

    // empty slots are skipped
    let mut o = Stand::new(RECORDING);
    o.video_enc[0] = false;
    o.audio_enc[0] = false;
    o.audio_enc[1] = true;
    o.end_data_capture_thread(g);
    assert_eq!(
        o.log(),
        "stop-audio-1-interleave signal-deactivate set-stopping"
    );
}

#[test]
fn steps_end_thread_raw() {
    let g = &CORE;

    // raw video
    let mut o = Stand::new(RAW_VIDEO);
    o.end_data_capture_thread(g);
    assert_eq!(o.log(), "stop-raw-video signal-deactivate set-stopping");

    // raw audio with raw_audio2: each mix in the mask
    let mut o = Stand::new(RAW_AV);
    o.raw_audio2 = true;
    o.mixer_mask = 0x25;
    o.end_data_capture_thread(g);
    assert_eq!(
        o.log(),
        "stop-raw-video disconnect-mix-0 disconnect-mix-2 disconnect-mix-5 signal-deactivate \
         set-stopping"
    );
    // bits past the mixes are never asked
    let mut o = Stand::new(F_AUDIO);
    o.raw_audio2 = true;
    o.mixer_mask = 0xc0;
    o.end_data_capture_thread(g);
    assert_eq!(o.log(), "signal-deactivate set-stopping");

    // without raw_audio2: the first mixer only, 0 for an empty mask
    let mut o = Stand::new(F_AUDIO);
    o.mixer_mask = 0x06;
    o.end_data_capture_thread(g);
    assert_eq!(o.log(), "disconnect-mix-1 signal-deactivate set-stopping");
    let mut o = Stand::new(F_AUDIO);
    o.end_data_capture_thread(g);
    assert_eq!(o.log(), "disconnect-mix-0 signal-deactivate set-stopping");

    // a raw output with the service flag deactivates it, and one with
    // active_delay_ns cleans the delay up (characterized, not endorsed)
    let mut o = Stand::new(RAW_VIDEO | F_SERVICE);
    o.active_delay_ns = 5;
    o.end_data_capture_thread(g);
    assert_eq!(
        o.log(),
        "stop-raw-video deactivate-service cleanup-delay signal-deactivate set-stopping"
    );
}

#[test]
fn steps_signal_stop_steps() {
    let g = &CORE;

    // no reconnect: the code kept for the stop signal, the capture ended
    let mut o = Stand::new(STREAM);
    o.signal_stop(g, CODE_DISCONNECTED);
    assert_eq!(
        o.log(),
        "log-frames stop-params create-end-thread signal-stop--2"
    );
    assert_eq!(o.stop_code, CODE_SUCCESS);

    // no reconnect, a delay active and reconnecting: all three cleared
    // before the end, so the delay ends whatever its restart refs
    let mut o = Stand::new(STREAM);
    o.delay_active = true;
    o.delay_capturing = true;
    o.reconnecting = true;
    o.delay_restart_refs = 1;
    o.signal_stop(g, CODE_ERROR);
    assert_eq!(
        o.log(),
        "log-frames stop-params create-end-thread signal-stop--4"
    );
    assert!(!o.delay_active);
    assert!(!o.delay_capturing);
    assert!(!o.reconnecting);
    assert_eq!(o.delay_restart_refs, 1);

    // a reconnect: a delay takes a restart reference and keeps going, no
    // stop signal
    let mut o = Stand::new(STREAM);
    o.can_reconnect = true;
    o.delay_active = true;
    o.signal_stop(g, CODE_DISCONNECTED);
    assert_eq!(o.log(), "set-stopping reconnect");
    assert_eq!(o.delay_restart_refs, 1);
    assert!(o.delay_active);
    assert_eq!(o.stop_code, CODE_DISCONNECTED);

    // a reconnect without a delay: the capture ends quietly
    let mut o = Stand::new(STREAM);
    o.can_reconnect = true;
    o.signal_stop(g, CODE_DISCONNECTED);
    assert_eq!(o.log(), "log-frames create-end-thread reconnect");
    assert_eq!(o.stop_code, CODE_DISCONNECTED);

    // a stream that failed to connect: the unstarted encoders shut down
    let mut o = Stand::new(STREAM);
    o.active = false;
    o.data_active = false;
    o.signal_stop(g, CODE_ERROR);
    assert_eq!(
        o.log(),
        "stop-params create-unstarted-thread signal-stop--4 set-stopping"
    );

    // invalid: nothing, the code kept
    let mut o = Stand::new(STREAM);
    o.valid = false;
    o.signal_stop(g, CODE_DISCONNECTED);
    assert_eq!(o.log(), "");
    assert_eq!(o.stop_code, CODE_ERROR);
}

#[test]
fn steps_stop_sequences() {
    let g = &CORE;

    // a stream stops, its plugin ends the capture, the thread runs, and a
    // second stop finds it inactive
    let mut o = Stand::new(STREAM);
    o.stop(g);
    o.end_data_capture(g);
    o.run_thread(g);
    assert_eq!(
        o.log(),
        "signal-stopping unpause reset-stopping info.stop-7 free-captions-0-2 log-frames \
         stop-params create-end-thread signal-stop--4 stop-video-0-interleave \
         stop-audio-0-interleave deactivate-service signal-deactivate set-stopping"
    );
    o.clear_log();
    o.stop(g);
    assert_eq!(o.log(), "");

    // a second stop while the first is under way: nothing
    let mut o = Stand::new(RECORDING);
    o.stop(g);
    o.clear_log();
    o.stop(g);
    assert_eq!(o.log(), "");
    // but a forced one goes through
    o.force_stop(g);
    assert_eq!(
        o.log(),
        "unpause reset-stopping info.stop-0 free-captions-0-0"
    );

    // an end while the last end-capture thread has not run: it is joined
    // first
    let mut o = Stand::new(RECORDING);
    o.end_data_capture(g);
    o.data_active = true;
    o.clear_log();
    o.end_data_capture(g);
    assert_eq!(
        o.log(),
        "log-frames join-end-thread stop-params create-end-thread signal-stop-0"
    );

    // a delayed stop, then the delay's end
    let mut o = Stand::new(STREAM);
    o.active_delay_ns = 1_000_000_000;
    o.delay_active = true;
    o.delay_capturing = true;
    o.stop(g);
    o.end_data_capture(g);
    o.run_thread(g);
    assert_eq!(
        o.log(),
        "delay-stop log-frames stop-params create-end-thread signal-stop--4 stop-video-0-delay \
         stop-audio-0-delay deactivate-service cleanup-delay signal-deactivate set-stopping"
    );
    assert!(!o.delay_active);
}

// ------------------------------------------------------------------------
// Edge cases against origin/master's expressions

#[test]
fn edge_flag_rules_every_word() {
    for high in 0..16u32 {
        for low in 0..=0xffffu32 {
            let f = (high << 16) | (high << 28) | low;
            assert_eq!(encoded(f), (ORIGIN.encoded)(f), "{f:#x}");
            assert_eq!(has_video(f), (ORIGIN.has_video)(f), "{f:#x}");
            assert_eq!(has_audio(f), (ORIGIN.has_audio)(f), "{f:#x}");
            assert_eq!(service(f), (ORIGIN.service)(f), "{f:#x}");
        }
    }
}

#[test]
fn edge_counts_delays_and_results() {
    for r in (-1000..=1000).chain([
        i64::MIN,
        i64::MIN + 1,
        i64::from(i32::MIN),
        i64::from(i32::MAX),
        i64::MAX - 1,
        i64::MAX,
    ]) {
        assert_eq!(end_delay(r), (ORIGIN.end_delay)(r), "{r}");
    }
    for d in (0..=100_000u64).chain(u64::MAX - 1000..=u64::MAX) {
        assert_eq!(delay_on(d), (ORIGIN.delay_on)(d), "{d}");
    }
    for r in (-1000..=1000).chain([i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX]) {
        assert_eq!(thread_failed(r), (ORIGIN.thread_failed)(r), "{r}");
    }
}

#[test]
fn edge_shared_rules() {
    for v in [false, true] {
        for a in [false, true] {
            assert_eq!(interleave(v, a), (ORIGIN.interleave)(v, a));
        }
    }
    for mask in (0..4096usize).chain([usize::MAX, usize::MAX - 1, 1 << 40]) {
        for idx in 0..6 {
            assert_eq!(
                mix_on(mask, idx),
                (ORIGIN.mix_on)(mask, idx),
                "{mask:#x} {idx}"
            );
        }
        assert_eq!(first_mixer(mask), (ORIGIN.first_mixer)(mask), "{mask:#x}");
    }
}

pub(crate) const CALLERS: u8 = 11;

/// Every caller, in turn, on a copy.
pub(crate) fn run_all(o: &Stand, g: &Gates) -> Vec<Stand> {
    (0..CALLERS)
        .map(|which| {
            let mut s = o.clone();
            call(&mut s, g, which);
            s.run_thread(g);
            s
        })
        .collect()
}

pub(crate) fn call(s: &mut Stand, g: &Gates, which: u8) {
    match which {
        0 => s.stop(g),
        1 => s.force_stop(g),
        2 => s.actual_stop(g, false, 5),
        3 => s.actual_stop(g, true, 5),
        4 => s.end_data_capture(g),
        5 => s.end_data_capture_internal(g, false),
        6 => s.end_data_capture_thread(g),
        7 => s.signal_stop(g, CODE_DISCONNECTED),
        8 => s.signal_stop(g, CODE_ERROR),
        9 => s.signal_stop(g, CODE_SUCCESS),
        _ => s.run_thread(g),
    }
}

/// The stand-in with each atomic and the stopping event from `bits`
/// (active, data_active, reconnecting, delay_active, delay_capturing,
/// starting, the end-capture thread's two flags, the stopping event), a
/// restart reference and a delay from the next two bits, and one input
/// changed (`change`, 0 for none).
pub(crate) fn stand_for(flags: u32, bits: u32, change: u32) -> Stand {
    let mut o = Stand::new(flags);
    o.active = bits & 1 != 0;
    o.data_active = bits & 2 != 0;
    o.reconnecting = bits & 4 != 0;
    o.delay_active = bits & 8 != 0;
    o.delay_capturing = bits & 16 != 0;
    o.starting = bits & 32 != 0;
    o.thread_active = bits & 64 != 0;
    o.thread_joinable = bits & 128 != 0;
    o.stopping_set = bits & 256 != 0;
    o.delay_restart_refs = i64::from(bits & 512 != 0);
    o.active_delay_ns = if bits & 1024 != 0 { 1_000_000_000 } else { 0 };
    o.valid = change != 1;
    o.has_data = change != 2;
    o.create_result = if change == 3 { EAGAIN } else { 0 };
    o.can_reconnect = change == 4;
    o.raw_audio2 = change == 5;
    o.reconnect_thread_active = change == 6;
    o.mixer_mask = if change == 7 { 0x26 } else { 0x01 };
    o.video_enc[1] = change == 8;
    o
}

#[test]
fn edge_stand_in_every_route() {
    // every low flag word, each atomic and the stopping event set and clear,
    // a restart reference or none, a delay or none, and each input changed
    // alone, through every caller
    for flags in 0..16u32 {
        for bits in 0..2048u32 {
            for change in 0..9u32 {
                let o = stand_for(flags, bits, change);
                assert_eq!(run_all(&o, &CORE), run_all(&o, &ORIGIN));
            }
        }
    }
}

// ------------------------------------------------------------------------
// Proptests

#[derive(Clone, Debug)]
pub(crate) enum Call {
    Caller(u8),
    /// another thread: one atomic, an event, the delay, the restart refs or
    /// a call result
    Flip(u8, i64),
}

pub(crate) fn step() -> impl Strategy<Value = Call> {
    prop_oneof![
        3 => (0u8..CALLERS).prop_map(Call::Caller),
        2 => (0u8..16, prop_oneof![Just(0i64), Just(1), Just(-1), any::<i64>()])
            .prop_map(|(w, v)| Call::Flip(w, v)),
    ]
}

pub(crate) fn apply(s: &mut Stand, g: &Gates, c: &Call) {
    match c {
        Call::Caller(which) => call(s, g, *which),
        Call::Flip(0, _) => s.active = !s.active,
        Call::Flip(1, _) => s.data_active = !s.data_active,
        Call::Flip(2, _) => s.reconnecting = !s.reconnecting,
        Call::Flip(3, _) => s.delay_active = !s.delay_active,
        Call::Flip(4, _) => s.delay_capturing = !s.delay_capturing,
        Call::Flip(5, _) => s.starting = !s.starting,
        Call::Flip(6, _) => s.thread_active = !s.thread_active,
        Call::Flip(7, _) => s.thread_joinable = !s.thread_joinable,
        Call::Flip(8, _) => s.stopping_set = !s.stopping_set,
        Call::Flip(9, v) => s.delay_restart_refs = *v,
        Call::Flip(10, v) => s.active_delay_ns = v.unsigned_abs() % 3,
        Call::Flip(11, v) => s.create_result = i32::try_from(*v % 3).unwrap_or(0) * EAGAIN,
        Call::Flip(12, _) => s.can_reconnect = !s.can_reconnect,
        Call::Flip(13, _) => s.valid = !s.valid,
        Call::Flip(14, _) => s.reconnect_thread_active = !s.reconnect_thread_active,
        // the created thread runs now
        Call::Flip(_, _) => s.run_thread(g),
    }
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
        d in any::<u64>(),
        ret in any::<i32>(),
        mask in any::<usize>(),
        idx in 0usize..6,
        v in any::<bool>(),
        a in any::<bool>(),
    ) {
        prop_assert_eq!(encoded(f), (ORIGIN.encoded)(f));
        prop_assert_eq!(has_video(f), (ORIGIN.has_video)(f));
        prop_assert_eq!(has_audio(f), (ORIGIN.has_audio)(f));
        prop_assert_eq!(service(f), (ORIGIN.service)(f));
        prop_assert_eq!(end_delay(r), (ORIGIN.end_delay)(r));
        prop_assert_eq!(delay_on(d), (ORIGIN.delay_on)(d));
        prop_assert_eq!(thread_failed(ret), (ORIGIN.thread_failed)(ret));
        prop_assert_eq!(mix_on(mask, idx), (ORIGIN.mix_on)(mask, idx));
        prop_assert_eq!(first_mixer(mask), (ORIGIN.first_mixer)(mask));
        prop_assert_eq!(interleave(v, a), (ORIGIN.interleave)(v, a));
    }

    #[test]
    fn edge_proptest_stand_in(
        flags in prop_oneof![0u32..16, any::<u32>()],
        bits in 0u32..2048,
        change in 0u32..9,
        refs in prop_oneof![Just(0i64), Just(1), Just(-1), any::<i64>()],
        mask in prop_oneof![0usize..64, any::<usize>()],
        calls in proptest::collection::vec(step(), 1..12),
    ) {
        let mut a = stand_for(flags, bits, change);
        a.delay_restart_refs = refs;
        a.mixer_mask = mask;
        let mut b = a.clone();
        for c in &calls {
            apply(&mut a, &CORE, c);
            apply(&mut b, &ORIGIN, c);
            prop_assert_eq!(&a, &b);
        }
    }
}
