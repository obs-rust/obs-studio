//! Tier 1: `obs_output::encoder_bind`, the encoder media binding and
//! shutdown checks.
//!
//! The `steps_*` cases port `test/cmocka/test_encoder_bind_steps.c` 1:1: its
//! 13 cases under the same names, in the same order, with the same values
//! and expectations, the "characterized, not endorsed" one included. The C
//! calls the hidden `obs_encoder_bind_*` and
//! `obs_encoder_audio_start_not_video` functions; here the safe core is
//! called directly. The C's stand-in encoder and its copies of the setters,
//! getters, unstarted shutdown, unbind and rebind are [`Stand`].
//!
//! The `edge_*` cases are new: every decision against a model of the
//! original C over every type from -1,000 to 1,000 and the extremes, every
//! trylock result in that range, every boolean combination, and proptests
//! of every decision over full-range values and of the stand-in over call
//! sequences against the origin/master code with its expressions inline.

use obs_output::encoder_bind::{
    OBS_ENCODER_AUDIO, Timebase, audio_timebase, divisor_override, locked, not_audio, not_video,
    shutdown_unstarted, video_timebase,
};
use proptest::prelude::*;

pub(crate) const TYPE_AUDIO: i32 = 0;
pub(crate) const TYPE_VIDEO: i32 = 1;

const EDGE_TYPE: [i32; 7] = [i32::MIN, -1, TYPE_AUDIO, TYPE_VIDEO, 2, 3, i32::MAX];
const EDGE_U32: [u32; 11] = [
    0,
    1,
    2,
    60,
    1001,
    30000,
    44100,
    48000,
    0x7fff_ffff,
    0x8000_0000,
    u32::MAX,
];

pub(crate) fn tb(num: u32, den: u32) -> Timebase {
    Timebase { num, den }
}

// ------------------------------------------------------------------------
// The stand-in encoder and the callers, as the C test's.

/// What an encoder's `media` (or `fps_override`) points at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Media {
    Video(Video),
    Audio(u32),
    /// The frame rate divisor video `encoder_set_video()` creates.
    Override,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Video {
    pub(crate) fps_num: u32,
    pub(crate) fps_den: u32,
}

pub(crate) const V_NTSC: Video = Video {
    fps_num: 30000,
    fps_den: 1001,
};
pub(crate) const V_60: Video = Video {
    fps_num: 60,
    fps_den: 1,
};
pub(crate) const V_FPS0: Video = Video {
    fps_num: 0,
    fps_den: 1,
};
pub(crate) const A_48K: u32 = 48000;

/// The gates the callers ask, so the stand-in runs on the core
/// ([`CORE`]) or on the origin/master expressions inline ([`ORIGIN`]).
pub(crate) struct Gates {
    pub(crate) not_video: fn(i32) -> bool,
    pub(crate) not_audio: fn(i32) -> bool,
    pub(crate) video_timebase: fn(bool, u32, u32) -> Timebase,
    pub(crate) audio_timebase: fn(bool, u32) -> Timebase,
    pub(crate) divisor_override: fn(u32) -> bool,
    pub(crate) locked: fn(i32) -> bool,
    pub(crate) shutdown_unstarted: fn(bool, bool) -> bool,
}

pub(crate) const CORE: Gates = Gates {
    not_video,
    not_audio,
    video_timebase,
    audio_timebase,
    divisor_override,
    locked,
    shutdown_unstarted,
};

/// origin/master's obs-encoder.c, its expressions inline.
pub(crate) const ORIGIN: Gates = Gates {
    not_video: |t| t != TYPE_VIDEO,
    not_audio: |t| t != TYPE_AUDIO,
    video_timebase: |has, num, den| if has { tb(den, num) } else { tb(0, 0) },
    audio_timebase: |has, rate| if has { tb(1, rate) } else { tb(0, 0) },
    divisor_override: |d| d != 0,
    locked: |r| r == 0,
    shutdown_unstarted: |init, needed| init && !needed,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Stand {
    pub(crate) encoder_type: i32,
    pub(crate) active: bool,
    pub(crate) initialized: bool,
    pub(crate) media: Option<Media>,
    pub(crate) original_video: Option<Video>,
    pub(crate) fps_override: bool,
    pub(crate) frame_rate_divisor: u32,
    pub(crate) timebase: Timebase,
    /// the outputs' `obs_output_needs_encoders()`
    pub(crate) needs: Vec<bool>,
    pub(crate) warnings: u32,
    pub(crate) shutdowns: u32,
    pub(crate) overrides_created: u32,
    pub(crate) overrides_freed: u32,
    pub(crate) needs_asked: usize,
    pub(crate) unlocks: u32,
}

impl Stand {
    pub(crate) fn new(encoder_type: i32, active: bool, initialized: bool) -> Self {
        Self {
            encoder_type,
            active,
            initialized,
            media: None,
            original_video: None,
            fps_override: false,
            frame_rate_divisor: 0,
            timebase: tb(7, 7),
            needs: Vec::new(),
            warnings: 0,
            shutdowns: 0,
            overrides_created: 0,
            overrides_freed: 0,
            needs_asked: 0,
            unlocks: 0,
        }
    }

    /// `encoder_set_video()`
    pub(crate) fn encoder_set_video(&mut self, g: &Gates, video: Option<Video>) {
        if self.fps_override {
            self.overrides_freed += 1;
            self.fps_override = false;
        }
        if let Some(v) = video {
            self.media = Some(Media::Video(v));
            self.timebase = (g.video_timebase)(true, v.fps_num, v.fps_den);
            if (g.divisor_override)(self.frame_rate_divisor) {
                self.overrides_created += 1;
                self.fps_override = true;
            }
        } else {
            self.media = None;
            self.timebase = (g.video_timebase)(false, 0, 0);
        }
    }

    /// `obs_encoder_set_video()` after `obs_encoder_valid()`
    pub(crate) fn set_video(&mut self, g: &Gates, video: Option<Video>) {
        if (g.not_video)(self.encoder_type) || self.active || self.initialized {
            self.warnings += 1;
            return;
        }
        self.encoder_set_video(g, video);
    }

    /// `obs_encoder_set_audio()` after `obs_encoder_valid()`
    pub(crate) fn set_audio(&mut self, g: &Gates, audio: Option<u32>) {
        if (g.not_audio)(self.encoder_type) || self.active {
            self.warnings += 1;
            return;
        }
        if let Some(rate) = audio {
            self.media = Some(Media::Audio(rate));
            self.timebase = (g.audio_timebase)(true, rate);
        } else {
            self.media = None;
            self.timebase = (g.audio_timebase)(false, 0);
        }
    }

    /// `obs_encoder_video()`
    pub(crate) fn video(&mut self, g: &Gates) -> Option<Media> {
        if (g.not_video)(self.encoder_type) {
            self.warnings += 1;
            return None;
        }
        if self.fps_override {
            Some(Media::Override)
        } else {
            self.media
        }
    }

    /// `obs_encoder_parent_video()`
    pub(crate) fn parent_video(&mut self, g: &Gates) -> Option<Media> {
        if (g.not_video)(self.encoder_type) {
            self.warnings += 1;
            return None;
        }
        self.media
    }

    /// `obs_encoder_audio()`
    pub(crate) fn audio(&mut self, g: &Gates) -> Option<Media> {
        if (g.not_audio)(self.encoder_type) {
            self.warnings += 1;
            return None;
        }
        self.media
    }

    /// `obs_encoder_shutdown_unstarted()`, with the two trylock results
    pub(crate) fn shutdown_unstarted(
        &mut self,
        g: &Gates,
        outputs_trylock: i32,
        init_trylock: i32,
    ) {
        if !(g.locked)(outputs_trylock) {
            return;
        }
        if !(g.locked)(init_trylock) {
            self.unlocks += 1;
            return;
        }
        let mut needed = self.active;
        let mut i = 0;
        while !needed && i < self.needs.len() {
            self.needs_asked += 1;
            needed = self.needs[i];
            i += 1;
        }
        if (g.shutdown_unstarted)(self.initialized, needed) {
            self.shutdowns += 1;
            self.initialized = false;
        }
        self.unlocks += 2;
    }

    /// `obs_encoder_unbind_video()`
    pub(crate) fn unbind_video(&mut self, g: &Gates, videos: &[Video]) -> Option<Media> {
        if (g.not_video)(self.encoder_type) {
            return None;
        }
        let mut bound = false;
        let mut i = 0;
        while self.media.is_some() && i < videos.len() && !bound {
            bound = self.media == Some(Media::Video(videos[i]));
            i += 1;
        }
        if bound && !self.active {
            if self.initialized {
                self.shutdowns += 1;
                self.initialized = false;
            }
            let video = self.media;
            self.encoder_set_video(g, None);
            self.original_video = None;
            return video;
        }
        None
    }

    /// `obs_encoder_rebind_video()`
    pub(crate) fn rebind_video(&mut self, g: &Gates, video: Option<Video>) {
        if self.media.is_none() && !self.initialized && !self.active {
            self.encoder_set_video(g, video);
        }
    }
}

// ------------------------------------------------------------------------
// test_encoder_bind_steps.c, 1:1

#[test]
fn steps_constants() {
    // OBS_ENCODER_AUDIO; the video refusal's OBS_ENCODER_VIDEO
    assert_eq!(OBS_ENCODER_AUDIO, 0);
    assert_eq!(obs_output::encoder_audio_start::OBS_ENCODER_VIDEO, 1);
}

#[test]
fn steps_type_refusals() {
    for t in EDGE_TYPE {
        assert_eq!(not_audio(t), t != TYPE_AUDIO);
        assert_eq!(not_video(t), t != TYPE_VIDEO);
    }
    // an unknown type is refused by both
    assert!(not_audio(2));
    assert!(not_video(2));
    assert!(not_audio(-1));
    assert!(not_video(-1));
}

#[test]
fn steps_video_timebase() {
    // the frame period: fps_den over fps_num
    assert_eq!(video_timebase(true, 30000, 1001), tb(1001, 30000));
    assert_eq!(video_timebase(true, 60, 1), tb(1, 60));

    // fps_num 0: a timebase den of 0, stored as is (the fps-0 class: pinned,
    // held; the frame rate comes from the video settings, which refuse it)
    assert_eq!(video_timebase(true, 0, 1), tb(1, 0));
    assert_eq!(video_timebase(true, 0, 0), tb(0, 0));

    for num in EDGE_U32 {
        for den in EDGE_U32 {
            assert_eq!(video_timebase(true, num, den), tb(den, num));
            // no video: 0 over 0, whatever is passed
            assert_eq!(video_timebase(false, num, den), tb(0, 0));
        }
    }
}

#[test]
fn steps_audio_timebase() {
    assert_eq!(audio_timebase(true, 48000), tb(1, 48000));
    assert_eq!(audio_timebase(true, 44100), tb(1, 44100));

    // a rate of 0: 1 over 0, stored as is (characterized, not endorsed; the
    // audio settings never give 0)
    assert_eq!(audio_timebase(true, 0), tb(1, 0));

    for rate in EDGE_U32 {
        assert_eq!(audio_timebase(true, rate), tb(1, rate));
        assert_eq!(audio_timebase(false, rate), tb(0, 0));
    }
}

#[test]
fn steps_divisor_override() {
    assert!(!divisor_override(0));
    assert!(divisor_override(1));
    assert!(divisor_override(2));
    for d in EDGE_U32 {
        assert_eq!(divisor_override(d), d != 0);
    }
}

#[test]
fn steps_locked() {
    // EAGAIN, EBUSY, EINVAL among them
    const RESULTS: [i32; 7] = [i32::MIN, -1, 1, 11, 16, 22, i32::MAX];
    assert!(locked(0));
    for r in RESULTS {
        assert!(!locked(r));
    }
}

#[test]
fn steps_shutdown_unstarted() {
    assert!(shutdown_unstarted(true, false));
    assert!(!shutdown_unstarted(true, true));
    assert!(!shutdown_unstarted(false, false));
    assert!(!shutdown_unstarted(false, true));
}

#[test]
fn steps_video_setter_gates() {
    let g = &CORE;
    for encoder_type in [TYPE_AUDIO, TYPE_VIDEO] {
        for active in [false, true] {
            for init in [false, true] {
                let mut e = Stand::new(encoder_type, active, init);
                e.set_video(g, Some(V_60));
                let binds = encoder_type == TYPE_VIDEO && !active && !init;
                assert_eq!(e.media == Some(Media::Video(V_60)), binds);
                assert_eq!(e.warnings, u32::from(!binds));
                assert_eq!(e.timebase, if binds { tb(1, 60) } else { tb(7, 7) });
            }
        }
    }

    // an unknown type is refused before the active flag
    let mut e = Stand::new(2, false, false);
    e.set_video(g, Some(V_60));
    assert_eq!(e.media, None);
    assert_eq!(e.warnings, 1);

    // NULL unbinds: no media, a timebase of 0 over 0
    let mut e = Stand::new(TYPE_VIDEO, false, false);
    e.set_video(g, Some(V_NTSC));
    assert_eq!(e.media, Some(Media::Video(V_NTSC)));
    assert_eq!(e.timebase, tb(1001, 30000));
    e.set_video(g, None);
    assert_eq!(e.media, None);
    assert_eq!(e.timebase, tb(0, 0));

    // fps_num 0: den 0 (pinned, held)
    e.set_video(g, Some(V_FPS0));
    assert_eq!(e.media, Some(Media::Video(V_FPS0)));
    assert_eq!(e.timebase, tb(1, 0));
}

#[test]
fn steps_audio_setter_gates() {
    let g = &CORE;
    for encoder_type in [TYPE_AUDIO, TYPE_VIDEO] {
        for active in [false, true] {
            for init in [false, true] {
                let mut e = Stand::new(encoder_type, active, init);
                e.set_audio(g, Some(A_48K));
                // characterized, not endorsed: the audio setter has no
                // initialized refusal, so an initialized idle audio encoder
                // takes the new audio (the video setter refuses it)
                let binds = encoder_type == TYPE_AUDIO && !active;
                assert_eq!(e.media == Some(Media::Audio(A_48K)), binds);
                assert_eq!(e.warnings, u32::from(!binds));
                assert_eq!(e.timebase, if binds { tb(1, 48000) } else { tb(7, 7) });
            }
        }
    }

    let mut e = Stand::new(TYPE_AUDIO, false, true);
    e.set_audio(g, Some(A_48K));
    assert_eq!(e.media, Some(Media::Audio(A_48K)));
    assert!(e.initialized);
    e.set_audio(g, None);
    assert_eq!(e.media, None);
    assert_eq!(e.timebase, tb(0, 0));

    let mut e = Stand::new(-1, false, false);
    e.set_audio(g, Some(A_48K));
    assert_eq!(e.media, None);
    assert_eq!(e.warnings, 1);
}

#[test]
fn steps_divisor_video() {
    let g = &CORE;

    // divisor 0: no frame rate divisor video; the getter gives the media
    let mut e = Stand::new(TYPE_VIDEO, false, false);
    e.set_video(g, Some(V_60));
    assert!(!e.fps_override);
    assert_eq!(e.overrides_created, 0);
    assert_eq!(e.video(g), Some(Media::Video(V_60)));
    assert_eq!(e.parent_video(g), Some(Media::Video(V_60)));

    // divisor 1: one is created (the same rate), and the getter gives it
    let mut e = Stand::new(TYPE_VIDEO, false, false);
    e.frame_rate_divisor = 1;
    e.set_video(g, Some(V_60));
    assert!(e.fps_override);
    assert_eq!(e.overrides_created, 1);
    assert_eq!(e.video(g), Some(Media::Override));
    assert_eq!(e.parent_video(g), Some(Media::Video(V_60)));

    // binding again frees the old one first; unbinding frees it
    e.set_video(g, Some(V_NTSC));
    assert_eq!(e.overrides_freed, 1);
    assert_eq!(e.overrides_created, 2);
    e.set_video(g, None);
    assert_eq!(e.overrides_freed, 2);
    assert_eq!(e.overrides_created, 2);
    assert!(!e.fps_override);
    assert_eq!(e.video(g), None);
}

#[test]
fn steps_getters() {
    let g = &CORE;

    let mut e = Stand::new(TYPE_AUDIO, false, false);
    e.media = Some(Media::Audio(A_48K));
    assert_eq!(e.audio(g), Some(Media::Audio(A_48K)));
    assert_eq!(e.video(g), None);
    assert_eq!(e.parent_video(g), None);
    assert_eq!(e.warnings, 2);

    let mut e = Stand::new(TYPE_VIDEO, true, true);
    e.media = Some(Media::Video(V_60));
    // the getters have no active or initialized gate
    assert_eq!(e.video(g), Some(Media::Video(V_60)));
    assert_eq!(e.parent_video(g), Some(Media::Video(V_60)));
    assert_eq!(e.audio(g), None);
    assert_eq!(e.warnings, 1);

    let mut e = Stand::new(2, false, false);
    e.media = Some(Media::Video(V_60));
    assert_eq!(e.video(g), None);
    assert_eq!(e.parent_video(g), None);
    assert_eq!(e.audio(g), None);
    assert_eq!(e.warnings, 3);
}

#[test]
fn steps_unstarted_shutdown() {
    let g = &CORE;
    let none = vec![false, false, false];
    let second = vec![false, true, false];

    // initialized, idle, no output needs it: shut down
    for encoder_type in [TYPE_AUDIO, TYPE_VIDEO] {
        let mut e = Stand::new(encoder_type, false, true);
        e.needs.clone_from(&none);
        e.shutdown_unstarted(g, 0, 0);
        assert_eq!(e.shutdowns, 1);
        assert!(!e.initialized);
        assert_eq!(e.needs_asked, 3);
        assert_eq!(e.unlocks, 2);
    }

    // either mutex busy: left alone; outputs_mutex released when init_mutex
    // is the busy one
    let mut e = Stand::new(TYPE_VIDEO, false, true);
    e.needs.clone_from(&none);
    e.shutdown_unstarted(g, 16, 0);
    assert_eq!(e.shutdowns, 0);
    assert_eq!(e.unlocks, 0);
    e.shutdown_unstarted(g, 0, 16);
    assert_eq!(e.shutdowns, 0);
    assert_eq!(e.unlocks, 1);
    assert!(e.initialized);
    assert_eq!(e.needs_asked, 0);

    // active: needed, and no output is asked
    let mut e = Stand::new(TYPE_VIDEO, true, true);
    e.needs.clone_from(&none);
    e.shutdown_unstarted(g, 0, 0);
    assert_eq!(e.shutdowns, 0);
    assert_eq!(e.needs_asked, 0);
    assert!(e.initialized);

    // an output needs it: the walk stops there
    let mut e = Stand::new(TYPE_AUDIO, false, true);
    e.needs.clone_from(&second);
    e.shutdown_unstarted(g, 0, 0);
    assert_eq!(e.shutdowns, 0);
    assert_eq!(e.needs_asked, 2);
    assert!(e.initialized);

    // not initialized: nothing to shut down, after the whole walk
    let mut e = Stand::new(TYPE_VIDEO, false, false);
    e.needs.clone_from(&none);
    e.shutdown_unstarted(g, 0, 0);
    assert_eq!(e.shutdowns, 0);
    assert_eq!(e.needs_asked, 3);
    assert_eq!(e.unlocks, 2);

    // no outputs
    let mut e = Stand::new(TYPE_VIDEO, false, true);
    e.shutdown_unstarted(g, 0, 0);
    assert_eq!(e.shutdowns, 1);
}

#[test]
fn steps_unbind_and_rebind() {
    let g = &CORE;
    let videos = [V_NTSC, V_60];

    for bound in [false, true] {
        for active in [false, true] {
            for init in [false, true] {
                let mut e = Stand::new(TYPE_VIDEO, false, false);
                e.frame_rate_divisor = 2;
                e.set_video(g, Some(if bound { V_60 } else { V_FPS0 }));
                e.original_video = Some(V_NTSC);
                e.active = active;
                e.initialized = init;

                let ret = e.unbind_video(g, &videos);
                let unbinds = bound && !active;
                assert_eq!(ret.is_some(), unbinds);
                if unbinds {
                    assert_eq!(ret, Some(Media::Video(V_60)));
                    assert_eq!(e.media, None);
                    assert!(!e.fps_override);
                    assert_eq!(e.original_video, None);
                    assert_eq!(e.timebase, tb(0, 0));
                    assert_eq!(e.shutdowns, u32::from(init));
                    assert!(!e.initialized);
                } else {
                    assert!(e.media.is_some());
                    assert_eq!(e.original_video, Some(V_NTSC));
                    assert_eq!(e.shutdowns, 0);
                    assert_eq!(e.initialized, init);
                }
            }
        }
    }

    // an audio encoder is never unbound, whatever it holds
    let mut e = Stand::new(TYPE_AUDIO, false, true);
    e.media = Some(Media::Video(V_60));
    assert_eq!(e.unbind_video(g, &videos), None);
    assert_eq!(e.media, Some(Media::Video(V_60)));
    assert!(e.initialized);

    // no media, or no videos: nothing bound
    let mut e = Stand::new(TYPE_VIDEO, false, true);
    assert_eq!(e.unbind_video(g, &videos), None);
    assert!(e.initialized);
    let mut e = Stand::new(TYPE_VIDEO, false, true);
    e.media = Some(Media::Video(V_60));
    assert_eq!(e.unbind_video(g, &[]), None);
    assert!(e.initialized);

    // the rebind: only with no media, not initialized and not active
    for media in [false, true] {
        for active in [false, true] {
            for init in [false, true] {
                let mut e = Stand::new(TYPE_VIDEO, active, init);
                e.media = media.then_some(Media::Video(V_FPS0));
                e.rebind_video(g, Some(V_NTSC));
                let rebinds = !media && !init && !active;
                assert_eq!(e.media == Some(Media::Video(V_NTSC)), rebinds);
                assert_eq!(e.timebase.den, if rebinds { 30000 } else { 7 });
            }
        }
    }

    // unbind, then rebind to the new video, keeps the divisor
    let mut e = Stand::new(TYPE_VIDEO, false, false);
    e.frame_rate_divisor = 2;
    e.set_video(g, Some(V_60));
    e.initialized = true;
    assert_eq!(e.unbind_video(g, &videos), Some(Media::Video(V_60)));
    e.rebind_video(g, Some(V_NTSC));
    assert_eq!(e.media, Some(Media::Video(V_NTSC)));
    assert!(e.fps_override);
    assert_eq!(e.timebase, tb(1001, 30000));
    assert_eq!(e.shutdowns, 1);

    // the rebind of a NULL video (no canvas) leaves it unbound
    let mut e = Stand::new(TYPE_VIDEO, false, false);
    e.rebind_video(g, None);
    assert_eq!(e.media, None);
    assert_eq!(e.timebase, tb(0, 0));
}

// ------------------------------------------------------------------------
// Edge cases: the core against the origin/master expressions.

#[test]
fn edge_type_refusals_every_type() {
    let extremes = [i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX];
    for t in (-1000..=1000).chain(extremes) {
        assert_eq!(not_audio(t), (ORIGIN.not_audio)(t), "{t}");
        assert_eq!(not_video(t), (ORIGIN.not_video)(t), "{t}");
        // no type is both
        assert!(not_audio(t) || not_video(t), "{t}");
    }
}

#[test]
fn edge_timebases_small_values() {
    for num in 0..=256 {
        for den in 0..=256 {
            for has in [false, true] {
                assert_eq!(
                    video_timebase(has, num, den),
                    (ORIGIN.video_timebase)(has, num, den)
                );
            }
        }
        for has in [false, true] {
            assert_eq!(audio_timebase(has, num), (ORIGIN.audio_timebase)(has, num));
        }
    }
}

#[test]
fn edge_divisor_and_locked_ranges() {
    for d in (0..=10_000).chain([u32::MAX - 1, u32::MAX, 0x8000_0000]) {
        assert_eq!(divisor_override(d), (ORIGIN.divisor_override)(d), "{d}");
    }
    let extremes = [i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX];
    for r in (-1000..=1000).chain(extremes) {
        assert_eq!(locked(r), (ORIGIN.locked)(r), "{r}");
    }
}

#[test]
fn edge_shutdown_every_combination() {
    for init in [false, true] {
        for needed in [false, true] {
            assert_eq!(
                shutdown_unstarted(init, needed),
                (ORIGIN.shutdown_unstarted)(init, needed)
            );
        }
    }
}

#[test]
fn edge_stand_in_every_gate_combination() {
    // every type, active, initialized, media, divisor and trylock pair,
    // through every caller
    let videos = [V_NTSC, V_60];
    let medias = [
        None,
        Some(Media::Video(V_60)),
        Some(Media::Video(V_FPS0)),
        Some(Media::Audio(A_48K)),
    ];
    for encoder_type in [-1, TYPE_AUDIO, TYPE_VIDEO, 2] {
        for active in [false, true] {
            for init in [false, true] {
                for media in medias {
                    for divisor in [0, 1, 2] {
                        for (ot, it) in [(0, 0), (16, 0), (0, 16), (16, 16)] {
                            for needs in [vec![], vec![false, true], vec![false, false]] {
                                let mut a = Stand::new(encoder_type, active, init);
                                a.media = media;
                                a.frame_rate_divisor = divisor;
                                a.needs = needs;
                                let mut b = a.clone();
                                let run = |e: &mut Stand, g: &Gates| {
                                    e.set_video(g, Some(V_NTSC));
                                    e.set_audio(g, Some(A_48K));
                                    let got = [e.video(g), e.parent_video(g), e.audio(g)];
                                    e.shutdown_unstarted(g, ot, it);
                                    let unbound = e.unbind_video(g, &videos);
                                    e.rebind_video(g, Some(V_60));
                                    e.set_video(g, None);
                                    (got, unbound)
                                };
                                assert_eq!(run(&mut a, &CORE), run(&mut b, &ORIGIN));
                                assert_eq!(a, b);
                            }
                        }
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
    SetVideo(Option<Video>),
    SetAudio(Option<u32>),
    Video,
    ParentVideo,
    Audio,
    ShutdownUnstarted(i32, i32),
    Unbind(Vec<Video>),
    Rebind(Option<Video>),
    /// another thread: the active flag, initialized or the divisor
    Flip(u8, u32),
}

fn video() -> impl Strategy<Value = Video> {
    prop_oneof![
        Just(V_NTSC),
        Just(V_60),
        Just(V_FPS0),
        (any::<u32>(), any::<u32>()).prop_map(|(fps_num, fps_den)| Video { fps_num, fps_den }),
    ]
}

pub(crate) fn call() -> impl Strategy<Value = Call> {
    prop_oneof![
        proptest::option::of(video()).prop_map(Call::SetVideo),
        proptest::option::of(any::<u32>()).prop_map(Call::SetAudio),
        Just(Call::Video),
        Just(Call::ParentVideo),
        Just(Call::Audio),
        (
            prop_oneof![Just(0), Just(16), any::<i32>()],
            prop_oneof![Just(0), Just(16), any::<i32>()]
        )
            .prop_map(|(a, b)| Call::ShutdownUnstarted(a, b)),
        proptest::collection::vec(video(), 0..4).prop_map(Call::Unbind),
        proptest::option::of(video()).prop_map(Call::Rebind),
        (0u8..3, any::<u32>()).prop_map(|(w, v)| Call::Flip(w, v)),
    ]
}

pub(crate) fn apply(e: &mut Stand, g: &Gates, c: &Call) -> Option<Media> {
    match c {
        Call::SetVideo(v) => e.set_video(g, *v),
        Call::SetAudio(a) => e.set_audio(g, *a),
        Call::Video => return e.video(g),
        Call::ParentVideo => return e.parent_video(g),
        Call::Audio => return e.audio(g),
        Call::ShutdownUnstarted(a, b) => e.shutdown_unstarted(g, *a, *b),
        Call::Unbind(v) => return e.unbind_video(g, v),
        Call::Rebind(v) => e.rebind_video(g, *v),
        Call::Flip(0, _) => e.active = !e.active,
        Call::Flip(1, _) => e.initialized = !e.initialized,
        Call::Flip(_, v) => e.frame_rate_divisor = *v % 3,
    }
    None
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 4096,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn edge_proptest_decisions(
        t in any::<i32>(),
        num in any::<u32>(),
        den in any::<u32>(),
        has in any::<bool>(),
        r in any::<i32>(),
        init in any::<bool>(),
        needed in any::<bool>(),
    ) {
        prop_assert_eq!(not_audio(t), (ORIGIN.not_audio)(t));
        prop_assert_eq!(not_video(t), (ORIGIN.not_video)(t));
        prop_assert_eq!(video_timebase(has, num, den), (ORIGIN.video_timebase)(has, num, den));
        prop_assert_eq!(audio_timebase(has, num), (ORIGIN.audio_timebase)(has, num));
        prop_assert_eq!(divisor_override(den), (ORIGIN.divisor_override)(den));
        prop_assert_eq!(locked(r), (ORIGIN.locked)(r));
        prop_assert_eq!(shutdown_unstarted(init, needed), (ORIGIN.shutdown_unstarted)(init, needed));
    }

    #[test]
    fn edge_proptest_stand_in(
        encoder_type in prop_oneof![Just(TYPE_AUDIO), Just(TYPE_VIDEO), any::<i32>()],
        active in any::<bool>(),
        init in any::<bool>(),
        divisor in 0u32..3,
        needs in proptest::collection::vec(any::<bool>(), 0..5),
        calls in proptest::collection::vec(call(), 1..12),
    ) {
        let mut a = Stand::new(encoder_type, active, init);
        a.frame_rate_divisor = divisor;
        a.needs = needs;
        let mut b = a.clone();
        for c in &calls {
            prop_assert_eq!(apply(&mut a, &CORE, c), apply(&mut b, &ORIGIN, c));
            prop_assert_eq!(&a, &b);
        }
    }
}
