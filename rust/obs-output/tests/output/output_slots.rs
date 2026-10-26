//! Tier 1: `obs_output::output_slots`, the encoder slot rules and setter
//! gates.
//!
//! The `steps_*` cases port `test/cmocka/test_output_slots_steps.c` 1:1: its
//! 16 cases under the same names, in the same order, with the same values
//! and expectations, the "characterized, not endorsed" ones included. The C
//! calls the hidden `obs_output_slots_*` functions; here the safe core is
//! called directly. The C's stand-in output (the two encoder setters, the
//! getters, `obs_output_audio()`, `obs_output_set_mixer()`,
//! `obs_output_remove_encoder_internal()` and
//! `obs_output_set_preferred_size2()`, with every warning, active load,
//! reference, hookup, caption track and scaled size recorded in order) is
//! [`StandIn`].
//!
//! The `edge_*` cases are new: every decision against a model of the
//! original C over every low flag word, mask and edge index; proptests of
//! every decision over full-range inputs; and a proptest of the stand-in
//! over call sequences against the origin/master code with its expressions
//! inline.

use obs_output::output_slots::{
    MAX_AUDIO_MIXES, MAX_OUTPUT_AUDIO_ENCODERS, MAX_OUTPUT_VIDEO_ENCODERS, OBS_ENCODER_AUDIO,
    OBS_ENCODER_VIDEO, OBS_OUTPUT_MULTI_TRACK_AUDIO, OBS_OUTPUT_MULTI_TRACK_VIDEO, SlotKind,
    default_index, first_slot, index_ok, mixer_in_range, mixer_mask, scaled_axis_set, slot_exists,
    type_ok,
};
use proptest::prelude::*;

const MAXV: usize = MAX_OUTPUT_VIDEO_ENCODERS;
const MAXA: usize = MAX_OUTPUT_AUDIO_ENCODERS;
const MIXES: usize = MAX_AUDIO_MIXES;
const MT_AUDIO: u32 = OBS_OUTPUT_MULTI_TRACK_AUDIO;
const MT_VIDEO: u32 = OBS_OUTPUT_MULTI_TRACK_VIDEO;
const T_AUDIO: i32 = OBS_ENCODER_AUDIO;
const T_VIDEO: i32 = OBS_ENCODER_VIDEO;

/// `obs-output.h`'s `OBS_OUTPUT_VIDEO`, `_AUDIO`, `_ENCODED`.
const F_VIDEO: u32 = 1 << 0;
const F_AUDIO: u32 = 1 << 1;
const F_ENCODED: u32 = 1 << 2;

const EDGE_IDX: [usize; 15] = [
    0,
    1,
    2,
    5,
    6,
    7,
    9,
    10,
    11,
    31,
    32,
    63,
    64,
    usize::MAX - 1,
    usize::MAX,
];

fn lowest_bit(mask: u32, below: usize) -> usize {
    (0..below)
        .find(|&i| mask & (1u32 << i) != 0)
        .unwrap_or(below)
}

// ------------------------------------------------------------------------
// test_output_slots_steps.c, 1:1

#[test]
fn steps_constants() {
    assert_eq!(OBS_OUTPUT_MULTI_TRACK_AUDIO, 16);
    assert_eq!(OBS_OUTPUT_MULTI_TRACK_VIDEO, 64);
    assert_eq!(MAX_OUTPUT_VIDEO_ENCODERS, 10);
    assert_eq!(MAX_OUTPUT_AUDIO_ENCODERS, 6);
    assert_eq!(MAX_AUDIO_MIXES, 6);
    assert_eq!(OBS_ENCODER_AUDIO, 0);
    assert_eq!(OBS_ENCODER_VIDEO, 1);
}

#[test]
fn steps_first_video() {
    for high in [0, 0x400, 0xffff_fc00, 0x8000_0000u32] {
        for m in 0..0x400u32 {
            let mask = m | high;
            let want = lowest_bit(mask, MAXV);
            assert_eq!(
                first_slot(SlotKind::Video, mask),
                (want < MAXV).then_some(want),
                "{mask:#x}"
            );
        }
    }
    assert_eq!(first_slot(SlotKind::Video, 1 << 9), Some(9));
    // only slots below 10 count
    assert_eq!(first_slot(SlotKind::Video, 1 << 10), None);
    assert_eq!(first_slot(SlotKind::Video, 0), None);
    assert_eq!(first_slot(SlotKind::Video, 0xffff_fc00), None);
}

#[test]
fn steps_first_audio() {
    for high in [0, 0x40, 0xffff_ffc0, 0x8000_0000u32] {
        for m in 0..0x40u32 {
            let mask = m | high;
            let want = lowest_bit(mask, MAXA);
            assert_eq!(
                first_slot(SlotKind::Audio, mask),
                (want < MAXA).then_some(want),
                "{mask:#x}"
            );
        }
    }
    assert_eq!(first_slot(SlotKind::Audio, 1 << 5), Some(5));
    // slots 6 to 9 are video only
    assert_eq!(first_slot(SlotKind::Audio, 1 << 6), None);
    assert_eq!(first_slot(SlotKind::Audio, 0x3c0), None);
}

#[test]
fn steps_mixer() {
    for idx in EDGE_IDX {
        assert_eq!(mixer_in_range(idx), idx < MIXES, "{idx}");
    }
    assert!(mixer_in_range(MIXES - 1));
    assert!(!mixer_in_range(MIXES));
    for i in 0..MIXES {
        assert_eq!(mixer_mask(i), 1usize << i);
    }
    // the shift itself, past the mixes (set_mixer never asks there)
    for i in 0..usize::BITS as usize {
        assert_eq!(mixer_mask(i), 1usize << i);
    }
}

#[test]
fn steps_type_gates() {
    for t in [-2, -1, 0, 1, 2, 3, 9, i32::MAX] {
        assert_eq!(type_ok(SlotKind::Video, t), t == 1, "{t}");
        assert_eq!(type_ok(SlotKind::Audio, t), t == 0, "{t}");
    }
    assert!(type_ok(SlotKind::Video, T_VIDEO));
    assert!(!type_ok(SlotKind::Video, T_AUDIO));
    assert!(type_ok(SlotKind::Audio, T_AUDIO));
    assert!(!type_ok(SlotKind::Audio, T_VIDEO));
}

#[test]
fn steps_index_rules() {
    for f in 0..0x100u32 {
        for idx in EDGE_IDX {
            let v = if f & MT_VIDEO != 0 {
                idx < MAXV
            } else {
                idx == 0
            };
            let a = if f & MT_AUDIO != 0 {
                idx < MAXA
            } else {
                idx == 0
            };
            assert_eq!(index_ok(SlotKind::Video, f, idx), v, "{f:#x} {idx}");
            assert_eq!(index_ok(SlotKind::Audio, f, idx), a, "{f:#x} {idx}");
            // high flag bits change nothing
            assert_eq!(index_ok(SlotKind::Video, f | 0xffff_ff00, idx), v);
            assert_eq!(index_ok(SlotKind::Audio, f | 0xffff_ff00, idx), a);
        }
    }
    // multitrack audio (OBS_OUTPUT_MULTI_TRACK) says nothing about video
    assert!(!index_ok(SlotKind::Video, MT_AUDIO, 1));
    assert!(index_ok(SlotKind::Video, MT_VIDEO, MAXV - 1));
    assert!(!index_ok(SlotKind::Video, MT_VIDEO, MAXV));
    assert!(!index_ok(SlotKind::Audio, MT_VIDEO, 1));
    assert!(index_ok(SlotKind::Audio, MT_AUDIO, MAXA - 1));
    assert!(!index_ok(SlotKind::Audio, MT_AUDIO, MAXA));
    assert!(index_ok(SlotKind::Video, 0, 0));
    assert!(index_ok(SlotKind::Audio, 0, 0));
    assert!(!index_ok(SlotKind::Video, 0, usize::MAX));
    assert!(!index_ok(SlotKind::Audio, MT_AUDIO, usize::MAX));
}

#[test]
fn steps_default_and_bounds() {
    for idx in EDGE_IDX {
        assert_eq!(default_index(idx), idx == 0);
        assert_eq!(slot_exists(SlotKind::Video, idx), idx < MAXV);
        assert_eq!(slot_exists(SlotKind::Audio, idx), idx < MAXA);
    }
    for size in [0, 1, 2, 720, 1280, 0x8000_0000, u32::MAX] {
        assert_eq!(scaled_axis_set(size), size != 0);
    }
}

// ------------------------------------------------------------------------
// The C test's stand-in output

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Encoder {
    id: i32,
    kind: i32,
    /// `obs_encoder_get_ref()` fails (the encoder is being destroyed).
    dying: bool,
}

const V1: Encoder = Encoder {
    id: 1,
    kind: T_VIDEO,
    dying: false,
};
const V2: Encoder = Encoder {
    id: 2,
    kind: T_VIDEO,
    dying: false,
};
const A3: Encoder = Encoder {
    id: 3,
    kind: T_AUDIO,
    dying: false,
};
const A4: Encoder = Encoder {
    id: 4,
    kind: T_AUDIO,
    dying: false,
};
const X5: Encoder = Encoder {
    id: 5,
    kind: 2,
    dying: false,
};
const D6: Encoder = Encoder {
    id: 6,
    kind: T_VIDEO,
    dying: true,
};

fn id(e: Option<Encoder>) -> i32 {
    e.map_or(0, |e| e.id)
}

/// The stand-in output. With `inline`, every decision is origin/master's
/// expression written out instead of the core's function.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct StandIn {
    flags: u32,
    active: bool,
    video: [Option<Encoder>; MAXV],
    audio: [Option<Encoder>; MAXA],
    captions: [u32; MAXV],
    scaled_width: u32,
    scaled_height: u32,
    mixer_mask: usize,
    next_caption: u32,
    ev: Vec<String>,
    inline: bool,
}

impl StandIn {
    fn new(flags: u32) -> Self {
        Self {
            flags,
            ..Self::default()
        }
    }

    fn ev(&self) -> String {
        self.ev.join(" ")
    }

    fn clear(&mut self) {
        self.ev.clear();
    }

    fn push(&mut self, e: impl Into<String>) {
        self.ev.push(e.into());
    }

    fn log_flag(&mut self, flag: u32, inverse_log: bool) -> bool {
        let ret = self.flags & flag != 0;
        if ret == inverse_log {
            self.push("warn-flag");
        }
        ret
    }

    fn load_active(&mut self) -> bool {
        self.push("active?");
        self.active
    }

    fn get_ref(&mut self, e: Option<Encoder>) -> Option<Encoder> {
        self.push(format!("ref{}", id(e)));
        e.filter(|e| !e.dying)
    }

    fn release(&mut self, e: Option<Encoder>) {
        self.push(format!("release{}", id(e)));
    }

    fn destroy_caption(&mut self, idx: usize) {
        if self.captions[idx] != 0 {
            self.push(format!("uncaption{idx}"));
            self.captions[idx] = 0;
        }
    }

    fn mask(slots: &[Option<Encoder>]) -> u32 {
        slots
            .iter()
            .enumerate()
            .filter(|(_, e)| e.is_some())
            .fold(0, |m, (i, _)| m | 1u32 << i)
    }

    // the decisions, from the core or inline

    fn d_type_ok(&self, kind: SlotKind, t: i32) -> bool {
        if self.inline {
            t == if kind == SlotKind::Video {
                T_VIDEO
            } else {
                T_AUDIO
            }
        } else {
            type_ok(kind, t)
        }
    }

    fn d_index_ok(&self, kind: SlotKind, idx: usize) -> bool {
        if self.inline {
            let (flag, max) = match kind {
                SlotKind::Video => (MT_VIDEO, MAXV),
                SlotKind::Audio => (MT_AUDIO, MAXA),
            };
            if self.flags & flag != 0 {
                idx < max
            } else {
                idx == 0
            }
        } else {
            index_ok(kind, self.flags, idx)
        }
    }

    fn d_default(&self, idx: usize) -> bool {
        if self.inline {
            idx == 0
        } else {
            default_index(idx)
        }
    }

    fn d_axis(&self, size: u32) -> bool {
        if self.inline {
            size != 0
        } else {
            scaled_axis_set(size)
        }
    }

    fn d_slot(&self, kind: SlotKind, idx: usize) -> bool {
        if self.inline {
            idx < if kind == SlotKind::Video { MAXV } else { MAXA }
        } else {
            slot_exists(kind, idx)
        }
    }

    fn d_first(&self, kind: SlotKind) -> Option<usize> {
        let slots: &[Option<Encoder>] = match kind {
            SlotKind::Video => &self.video,
            SlotKind::Audio => &self.audio,
        };
        if self.inline {
            slots.iter().position(Option::is_some)
        } else {
            first_slot(kind, Self::mask(slots))
        }
    }

    fn d_mixer_in_range(&self, idx: usize) -> bool {
        if self.inline {
            idx < MIXES
        } else {
            mixer_in_range(idx)
        }
    }

    fn d_mixer_mask(&self, idx: usize) -> usize {
        if self.inline {
            1usize << idx
        } else {
            mixer_mask(idx)
        }
    }

    // the callers

    fn set_video_encoder2(&mut self, encoder: Option<Encoder>, idx: usize) {
        if !self.log_flag(F_ENCODED, false) || !self.log_flag(F_VIDEO, false) {
            return;
        }
        if let Some(e) = encoder
            && !self.d_type_ok(SlotKind::Video, e.kind)
        {
            self.push("warn-type");
            return;
        }
        if self.load_active() {
            self.push("warn-active");
            return;
        }
        if !self.d_index_ok(SlotKind::Video, idx) {
            return;
        }
        if self.video[idx] == encoder {
            return;
        }
        self.push(format!("unhook{}", id(self.video[idx])));
        self.release(self.video[idx]);
        self.video[idx] = self.get_ref(encoder);
        self.push(format!("hook{}", id(self.video[idx])));

        // Decide on the stored slot, as obs_output_set_video_encoder2 does
        // since #2495.
        self.destroy_caption(idx);
        if self.video[idx].is_some() {
            self.next_caption += 1;
            self.captions[idx] = self.next_caption;
            self.push(format!("caption{idx}"));
        } else {
            self.captions[idx] = 0;
        }

        if self.d_default(idx)
            && self.video[idx].is_some()
            && self.d_axis(self.scaled_width)
            && self.d_axis(self.scaled_height)
        {
            self.push(format!(
                "size{}={}x{}",
                id(self.video[idx]),
                self.scaled_width,
                self.scaled_height
            ));
        }
    }

    fn set_audio_encoder(&mut self, encoder: Option<Encoder>, idx: usize) {
        if !self.log_flag(F_ENCODED, false) || !self.log_flag(F_AUDIO, false) {
            return;
        }
        if let Some(e) = encoder
            && !self.d_type_ok(SlotKind::Audio, e.kind)
        {
            self.push("warn-type");
            return;
        }
        if self.load_active() {
            self.push("warn-active");
            return;
        }
        if !self.d_index_ok(SlotKind::Audio, idx) {
            return;
        }
        if self.audio[idx] == encoder {
            return;
        }
        self.push(format!("unhook{}", id(self.audio[idx])));
        self.release(self.audio[idx]);
        self.audio[idx] = self.get_ref(encoder);
        self.push(format!("hook{}", id(self.audio[idx])));
    }

    fn get_video_encoder2(&self, idx: usize) -> i32 {
        if self.d_slot(SlotKind::Video, idx) {
            id(self.video[idx])
        } else {
            -1
        }
    }

    fn get_audio_encoder(&self, idx: usize) -> i32 {
        if self.d_slot(SlotKind::Audio, idx) {
            id(self.audio[idx])
        } else {
            -1
        }
    }

    fn get_video_encoder(&self) -> i32 {
        self.d_first(SlotKind::Video)
            .map_or(-1, |i| self.get_video_encoder2(i))
    }

    /// `obs_output_audio()`: its own audio (100) for a raw output, else the
    /// first audio encoder's (its id), else none (-1).
    fn audio_of(&self) -> i32 {
        if self.flags & F_ENCODED == 0 {
            return 100;
        }
        self.d_first(SlotKind::Audio)
            .map_or(-1, |i| id(self.audio[i]))
    }

    fn set_mixer(&mut self, mixer_idx: usize) {
        if self.log_flag(F_ENCODED, true) {
            return;
        }
        if !self.d_mixer_in_range(mixer_idx) {
            self.push("warn-mixer");
            return;
        }
        if self.load_active() {
            return;
        }
        self.mixer_mask = self.d_mixer_mask(mixer_idx);
    }

    fn remove_encoder_internal(&mut self, encoder: Encoder) {
        if self.d_type_ok(SlotKind::Video, encoder.kind) {
            for i in 0..MAXV {
                if self.video[i] == Some(encoder) {
                    self.video[i] = None;
                    self.release(Some(encoder));
                }
            }
        } else if self.d_type_ok(SlotKind::Audio, encoder.kind) {
            for i in 0..MAXA {
                if self.audio[i] == Some(encoder) {
                    self.audio[i] = None;
                    self.release(Some(encoder));
                }
            }
        }
    }

    fn set_preferred_size2(&mut self, width: u32, height: u32, idx: usize) {
        if !self.log_flag(F_VIDEO, false) {
            return;
        }
        if !self.d_slot(SlotKind::Video, idx) {
            return;
        }
        if self.load_active() {
            self.push("warn-active");
            return;
        }
        if self.d_default(idx) {
            self.scaled_width = width;
            self.scaled_height = height;
        }
        if self.flags & F_ENCODED != 0 && self.video[idx].is_some() {
            self.push(format!("size{}={width}x{height}", id(self.video[idx])));
        }
    }
}

#[test]
fn steps_e2e_video_flags() {
    // a raw output: refused with the encoded warning, active not loaded
    let mut o = StandIn::new(F_VIDEO | F_AUDIO);
    o.set_video_encoder2(Some(V1), 0);
    assert_eq!(o.ev(), "warn-flag");
    assert_eq!(o.video[0], None);
    // an encoded audio-only output: the video warning
    let mut o = StandIn::new(F_ENCODED | F_AUDIO);
    o.set_video_encoder2(Some(V1), 0);
    assert_eq!(o.ev(), "warn-flag");
    // an audio encoder: the type warning, active not loaded
    let mut o = StandIn::new(F_ENCODED | F_VIDEO);
    o.active = true;
    o.set_video_encoder2(Some(A3), 0);
    assert_eq!(o.ev(), "warn-type");
    o.clear();
    o.set_video_encoder2(Some(X5), 0);
    assert_eq!(o.ev(), "warn-type");
    // active: refused after the type gate, even with NULL
    o.clear();
    o.set_video_encoder2(Some(V1), 0);
    assert_eq!(o.ev(), "active? warn-active");
    o.clear();
    o.set_video_encoder2(None, 5);
    assert_eq!(o.ev(), "active? warn-active");
    assert_eq!(o.video[0], None);
}

#[test]
fn steps_e2e_video_index() {
    // single track: index 0 only, others refused silently
    let mut o = StandIn::new(F_ENCODED | F_VIDEO);
    for idx in [1, MAXV - 1, MAXV, usize::MAX] {
        o.clear();
        o.set_video_encoder2(Some(V1), idx);
        assert_eq!(o.ev(), "active?", "{idx}");
    }
    o.clear();
    o.set_video_encoder2(Some(V1), 0);
    assert_eq!(o.ev(), "active? unhook0 release0 ref1 hook1 caption0");
    assert_eq!(o.video[0], Some(V1));
    assert_eq!(o.captions[0], 1);

    // multitrack video: below MAX_OUTPUT_VIDEO_ENCODERS
    let mut o = StandIn::new(F_ENCODED | F_VIDEO | MT_VIDEO);
    o.set_video_encoder2(Some(V2), MAXV - 1);
    assert_eq!(o.ev(), "active? unhook0 release0 ref2 hook2 caption9");
    o.clear();
    o.set_video_encoder2(Some(V2), 1);
    assert_eq!(o.ev(), "active? unhook0 release0 ref2 hook2 caption1");
    o.clear();
    o.set_video_encoder2(Some(V2), MAXV);
    assert_eq!(o.ev(), "active?");
    o.clear();
    o.set_video_encoder2(Some(V2), usize::MAX);
    assert_eq!(o.ev(), "active?");
    // OBS_OUTPUT_MULTI_TRACK alone is audio: video stays single track
    let mut o = StandIn::new(F_ENCODED | F_VIDEO | MT_AUDIO);
    o.set_video_encoder2(Some(V2), 1);
    assert_eq!(o.ev(), "active?");
    assert_eq!(o.video[1], None);
}

#[test]
fn steps_e2e_video_replace() {
    let mut o = StandIn::new(F_ENCODED | F_VIDEO | MT_VIDEO);
    o.set_video_encoder2(Some(V1), 0);
    // the same encoder twice: nothing after the active load
    o.clear();
    o.set_video_encoder2(Some(V1), 0);
    assert_eq!(o.ev(), "active?");
    assert_eq!(o.captions[0], 1);
    // another one: the old is unhooked and released first, the caption
    // track replaced
    o.clear();
    o.set_video_encoder2(Some(V2), 0);
    assert_eq!(
        o.ev(),
        "active? unhook1 release1 ref2 hook2 uncaption0 caption0"
    );
    assert_eq!(o.captions[0], 2);
    // NULL clears the slot and destroys its caption track
    o.clear();
    o.set_video_encoder2(None, 0);
    assert_eq!(o.ev(), "active? unhook2 release2 ref0 hook0 uncaption0");
    assert_eq!(o.video[0], None);
    assert_eq!(o.captions[0], 0);
    // NULL on an empty slot: the same encoder, nothing
    o.clear();
    o.set_video_encoder2(None, 3);
    assert_eq!(o.ev(), "active?");
    // an encoder whose reference fails leaves the slot empty and gets no
    // caption track (#2495)
    o.clear();
    o.set_video_encoder2(Some(D6), 4);
    assert_eq!(o.ev(), "active? unhook0 release0 ref6 hook0");
    assert_eq!(o.video[4], None);
    assert_eq!(o.captions[4], 0);
}

#[test]
fn steps_e2e_video_preferred_size() {
    // index 0 with both axes set: the encoder takes the scaled size
    let mut o = StandIn::new(F_ENCODED | F_VIDEO | MT_VIDEO);
    o.scaled_width = 1280;
    o.scaled_height = 720;
    o.set_video_encoder2(Some(V1), 0);
    assert_eq!(
        o.ev(),
        "active? unhook0 release0 ref1 hook1 caption0 size1=1280x720"
    );
    // any other index: not
    o.clear();
    o.set_video_encoder2(Some(V2), 1);
    assert_eq!(o.ev(), "active? unhook0 release0 ref2 hook2 caption1");
    // one axis 0: not
    let mut o = StandIn::new(F_ENCODED | F_VIDEO);
    o.scaled_width = 1280;
    o.set_video_encoder2(Some(V1), 0);
    assert_eq!(o.ev(), "active? unhook0 release0 ref1 hook1 caption0");
    let mut o = StandIn::new(F_ENCODED | F_VIDEO);
    o.scaled_height = 720;
    o.set_video_encoder2(Some(V1), 0);
    assert_eq!(o.ev(), "active? unhook0 release0 ref1 hook1 caption0");
    // clearing index 0 with a scaled size: no size on the empty slot (#2495)
    let mut o = StandIn::new(F_ENCODED | F_VIDEO);
    o.scaled_width = 1280;
    o.scaled_height = 720;
    o.video[0] = Some(V1);
    o.set_video_encoder2(None, 0);
    assert_eq!(o.ev(), "active? unhook1 release1 ref0 hook0");
}

#[test]
fn steps_e2e_audio() {
    // a raw output, then a video-only encoded output
    let mut o = StandIn::new(F_AUDIO);
    o.set_audio_encoder(Some(A3), 0);
    assert_eq!(o.ev(), "warn-flag");
    let mut o = StandIn::new(F_ENCODED | F_VIDEO);
    o.set_audio_encoder(Some(A3), 0);
    assert_eq!(o.ev(), "warn-flag");
    // the wrong type, then active
    let mut o = StandIn::new(F_ENCODED | F_AUDIO);
    o.active = true;
    o.set_audio_encoder(Some(V1), 0);
    assert_eq!(o.ev(), "warn-type");
    o.clear();
    o.set_audio_encoder(Some(A3), 0);
    assert_eq!(o.ev(), "active? warn-active");
    // single track: 0 only, no caption track, no size
    let mut o = StandIn::new(F_ENCODED | F_AUDIO);
    o.scaled_width = 1280;
    o.scaled_height = 720;
    o.set_audio_encoder(Some(A3), 1);
    assert_eq!(o.ev(), "active?");
    o.clear();
    o.set_audio_encoder(Some(A3), 0);
    assert_eq!(o.ev(), "active? unhook0 release0 ref3 hook3");
    o.clear();
    o.set_audio_encoder(Some(A3), 0);
    assert_eq!(o.ev(), "active?");
    o.clear();
    o.set_audio_encoder(Some(A4), 0);
    assert_eq!(o.ev(), "active? unhook3 release3 ref4 hook4");
    o.clear();
    o.set_audio_encoder(None, 0);
    assert_eq!(o.ev(), "active? unhook4 release4 ref0 hook0");
    // multitrack audio (OBS_OUTPUT_MULTI_TRACK): below 6
    let mut o = StandIn::new(F_ENCODED | F_AUDIO | MT_AUDIO);
    o.set_audio_encoder(Some(A4), MAXA - 1);
    assert_eq!(o.ev(), "active? unhook0 release0 ref4 hook4");
    for idx in [MAXA, MAXV - 1, usize::MAX] {
        o.clear();
        o.set_audio_encoder(Some(A3), idx);
        assert_eq!(o.ev(), "active?", "{idx}");
    }
    // OBS_OUTPUT_MULTI_TRACK_VIDEO says nothing about audio
    let mut o = StandIn::new(F_ENCODED | F_AUDIO | MT_VIDEO);
    o.set_audio_encoder(Some(A3), 1);
    assert_eq!(o.ev(), "active?");
}

#[test]
fn steps_e2e_getters() {
    // the bounds only: a single-track output still answers slot 9
    let mut o = StandIn::new(F_ENCODED | F_VIDEO | F_AUDIO);
    o.video[3] = Some(V2);
    o.video[9] = Some(V1);
    o.audio[2] = Some(A4);
    o.audio[5] = Some(A3);
    for idx in EDGE_IDX {
        let v = if idx < MAXV { id(o.video[idx]) } else { -1 };
        let a = if idx < MAXA { id(o.audio[idx]) } else { -1 };
        assert_eq!(o.get_video_encoder2(idx), v, "{idx}");
        assert_eq!(o.get_audio_encoder(idx), a, "{idx}");
    }
    assert_eq!(o.get_video_encoder2(MAXV - 1), 1);
    assert_eq!(o.get_audio_encoder(MAXA - 1), 3);
    // obs_output_get_video_encoder(): the first occupied slot
    assert_eq!(o.get_video_encoder(), 2);
    o.video[3] = None;
    assert_eq!(o.get_video_encoder(), 1);
    o.video[9] = None;
    assert_eq!(o.get_video_encoder(), -1);
    // obs_output_audio(): the first audio encoder's, or the output's own
    assert_eq!(o.audio_of(), 4);
    o.audio[2] = None;
    assert_eq!(o.audio_of(), 3);
    o.audio[5] = None;
    assert_eq!(o.audio_of(), -1);
    o.flags = F_AUDIO;
    o.audio[0] = Some(A3);
    assert_eq!(o.audio_of(), 100);
}

#[test]
fn steps_e2e_mixer() {
    let mut o = StandIn::new(F_AUDIO);
    o.mixer_mask = 7;
    o.set_mixer(MIXES - 1);
    assert_eq!(o.ev(), "active?");
    assert_eq!(o.mixer_mask, 1 << 5);
    o.clear();
    o.set_mixer(MIXES);
    assert_eq!(o.ev(), "warn-mixer");
    assert_eq!(o.mixer_mask, 1 << 5);
    o.clear();
    o.set_mixer(usize::MAX);
    assert_eq!(o.ev(), "warn-mixer");
    o.clear();
    o.set_mixer(0);
    assert_eq!(o.mixer_mask, 1);
    // active: the range warning first, then the active refusal
    o.active = true;
    o.clear();
    o.set_mixer(MIXES);
    assert_eq!(o.ev(), "warn-mixer");
    o.clear();
    o.set_mixer(3);
    assert_eq!(o.ev(), "active?");
    assert_eq!(o.mixer_mask, 1);
    // an encoded output: the raw warning
    let mut o = StandIn::new(F_ENCODED | F_AUDIO);
    o.set_mixer(1);
    assert_eq!(o.ev(), "warn-flag");
    assert_eq!(o.mixer_mask, 0);
}

#[test]
fn steps_e2e_remove() {
    let mut o = StandIn::new(F_ENCODED | F_VIDEO | F_AUDIO | MT_VIDEO | MT_AUDIO);
    o.video[0] = Some(V1);
    o.video[4] = Some(V1);
    o.video[9] = Some(V2);
    o.audio[1] = Some(A3);
    o.audio[5] = Some(A3);
    // every slot of the encoder's kind that holds it, in order
    o.remove_encoder_internal(V1);
    assert_eq!(o.ev(), "release1 release1");
    assert_eq!(o.video[0], None);
    assert_eq!(o.video[4], None);
    assert_eq!(o.video[9], Some(V2));
    o.clear();
    o.remove_encoder_internal(A3);
    assert_eq!(o.ev(), "release3 release3");
    assert_eq!(o.audio[1], None);
    assert_eq!(o.audio[5], None);
    // an encoder in no slot, and one of neither type: nothing
    o.clear();
    o.remove_encoder_internal(A4);
    o.remove_encoder_internal(X5);
    assert_eq!(o.ev(), "");
    // a video encoder in an audio slot is not looked for there
    // (characterized; the setters refuse to put it there)
    o.audio[0] = Some(V2);
    o.remove_encoder_internal(V2);
    assert_eq!(o.video[9], None);
    assert_eq!(o.audio[0], Some(V2));
}

#[test]
fn steps_e2e_preferred_size() {
    // a raw video output: index 0 sets the scaled size, no encoder
    let mut o = StandIn::new(F_VIDEO);
    o.set_preferred_size2(1280, 720, 0);
    assert_eq!(o.ev(), "active?");
    assert_eq!((o.scaled_width, o.scaled_height), (1280, 720));
    // other indexes below 10 change nothing on a raw output
    o.clear();
    o.set_preferred_size2(640, 360, 1);
    assert_eq!(o.ev(), "active?");
    assert_eq!(o.scaled_width, 1280);
    o.clear();
    o.set_preferred_size2(640, 360, MAXV);
    o.set_preferred_size2(640, 360, usize::MAX);
    assert_eq!(o.ev(), "");
    // an audio-only output: the video warning
    let mut o = StandIn::new(F_AUDIO);
    o.set_preferred_size2(640, 360, 0);
    assert_eq!(o.ev(), "warn-flag");
    // active: the warning, nothing set
    let mut o = StandIn::new(F_VIDEO);
    o.active = true;
    o.set_preferred_size2(640, 360, 0);
    assert_eq!(o.ev(), "active? warn-active");
    assert_eq!(o.scaled_width, 0);
    // encoded: index 0 sets both; the encoder at the index takes the size
    let mut o = StandIn::new(F_ENCODED | F_VIDEO);
    o.video[0] = Some(V1);
    o.video[MAXV - 1] = Some(V2);
    o.set_preferred_size2(1920, 1080, 0);
    assert_eq!(o.ev(), "active? size1=1920x1080");
    assert_eq!(o.scaled_width, 1920);
    o.clear();
    o.set_preferred_size2(640, 360, MAXV - 1);
    assert_eq!(o.ev(), "active? size2=640x360");
    assert_eq!(o.scaled_width, 1920);
    o.clear();
    o.set_preferred_size2(640, 360, 1);
    assert_eq!(o.ev(), "active?");
    // a 0 size is passed on as it is
    o.clear();
    o.set_preferred_size2(0, 0, 0);
    assert_eq!(o.ev(), "active? size1=0x0");
    assert_eq!(o.scaled_width, 0);
}

// ------------------------------------------------------------------------
// New edge cases

/// origin/master's loop over the slots, on a mask.
fn model_first(mask: u32, count: usize) -> Option<usize> {
    (0..count).find(|&i| mask & (1u32 << i) != 0)
}

fn model_index_ok(kind: SlotKind, flags: u32, idx: usize) -> bool {
    let (flag, max) = match kind {
        SlotKind::Video => (MT_VIDEO, MAXV),
        SlotKind::Audio => (MT_AUDIO, MAXA),
    };
    if flags & flag != 0 {
        idx < max
    } else {
        idx == 0
    }
}

#[test]
fn edge_slot_counts() {
    assert_eq!(SlotKind::Video.count(), MAXV);
    assert_eq!(SlotKind::Audio.count(), MAXA);
}

#[test]
fn edge_first_slot_every_mask() {
    // every 16-bit mask under four high words, both kinds
    for high in [0, 0x1_0000, 0xffff_0000, 0x8000_0000u32] {
        for m in 0..0x1_0000u32 {
            let mask = m | high;
            assert_eq!(first_slot(SlotKind::Video, mask), model_first(mask, MAXV));
            assert_eq!(first_slot(SlotKind::Audio, mask), model_first(mask, MAXA));
        }
    }
    assert_eq!(first_slot(SlotKind::Video, u32::MAX), Some(0));
    assert_eq!(first_slot(SlotKind::Video, u32::MAX << 9), Some(9));
    assert_eq!(first_slot(SlotKind::Audio, u32::MAX << 5), Some(5));
    assert_eq!(first_slot(SlotKind::Audio, u32::MAX << 6), None);
}

#[test]
fn edge_index_rules_every_flag() {
    // every low 16-bit flag word under four high words, indexes 0 to 70
    // and the edges
    for high in [0, 0x1_0000, 0xffff_0000, 0x8000_0000u32] {
        for f in 0..0x1_0000u32 {
            let flags = f | high;
            for idx in (0..12).chain(EDGE_IDX) {
                assert_eq!(
                    index_ok(SlotKind::Video, flags, idx),
                    model_index_ok(SlotKind::Video, flags, idx)
                );
                assert_eq!(
                    index_ok(SlotKind::Audio, flags, idx),
                    model_index_ok(SlotKind::Audio, flags, idx)
                );
            }
        }
    }
}

#[test]
fn edge_mixer_past_the_width() {
    // the C's shift is undefined from the width of size_t up; the core says
    // 0 (set_mixer() asks only below MAX_AUDIO_MIXES)
    for idx in [
        usize::BITS as usize,
        usize::BITS as usize + 1,
        1 << 32,
        usize::MAX,
    ] {
        assert_eq!(mixer_mask(idx), 0, "{idx}");
        assert!(!mixer_in_range(idx));
    }
}

#[test]
fn edge_type_gates_every_low_type() {
    for t in -1000..1000 {
        assert_eq!(type_ok(SlotKind::Video, t), t == T_VIDEO);
        assert_eq!(type_ok(SlotKind::Audio, t), t == T_AUDIO);
    }
    for t in [i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX] {
        assert!(!type_ok(SlotKind::Video, t));
        assert!(!type_ok(SlotKind::Audio, t));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn edge_proptest_decisions(mask: u32, flags: u32, idx: usize, small in 0usize..80, t: i32, size: u32) {
        for kind in [SlotKind::Video, SlotKind::Audio] {
            let count = if kind == SlotKind::Video { MAXV } else { MAXA };
            prop_assert_eq!(first_slot(kind, mask), model_first(mask, count));
            for i in [idx, small] {
                prop_assert_eq!(index_ok(kind, flags, i), model_index_ok(kind, flags, i));
                prop_assert_eq!(slot_exists(kind, i), i < count);
            }
            let want = if kind == SlotKind::Video { T_VIDEO } else { T_AUDIO };
            prop_assert_eq!(type_ok(kind, t), t == want);
        }
        for i in [idx, small] {
            prop_assert_eq!(mixer_in_range(i), i < MIXES);
            prop_assert_eq!(default_index(i), i == 0);
            prop_assert_eq!(
                mixer_mask(i),
                if i < usize::BITS as usize { 1usize << i } else { 0 }
            );
        }
        prop_assert_eq!(scaled_axis_set(size), size != 0);
    }
}

/// One call on the stand-in.
#[derive(Debug, Clone)]
enum Call {
    SetVideo(Option<Encoder>, usize),
    SetAudio(Option<Encoder>, usize),
    GetVideo2(usize),
    GetAudio(usize),
    GetVideo,
    Audio,
    SetMixer(usize),
    Remove(Encoder),
    PreferredSize(u32, u32, usize),
    SetActive(bool),
}

fn encoder() -> impl Strategy<Value = Option<Encoder>> {
    prop_oneof![
        Just(None),
        Just(Some(V1)),
        Just(Some(V2)),
        Just(Some(A3)),
        Just(Some(A4)),
        Just(Some(X5)),
        Just(Some(D6)),
    ]
}

fn index() -> impl Strategy<Value = usize> {
    prop_oneof![
        4 => 0usize..12,
        1 => Just(usize::MAX),
        1 => any::<usize>(),
    ]
}

fn size() -> impl Strategy<Value = u32> {
    prop_oneof![Just(0u32), Just(1280u32), any::<u32>()]
}

fn call() -> impl Strategy<Value = Call> {
    prop_oneof![
        (encoder(), index()).prop_map(|(e, i)| Call::SetVideo(e, i)),
        (encoder(), index()).prop_map(|(e, i)| Call::SetAudio(e, i)),
        index().prop_map(Call::GetVideo2),
        index().prop_map(Call::GetAudio),
        Just(Call::GetVideo),
        Just(Call::Audio),
        index().prop_map(Call::SetMixer),
        encoder().prop_map(|e| Call::Remove(e.unwrap_or(V1))),
        (size(), size(), index()).prop_map(|(w, h, i)| Call::PreferredSize(w, h, i)),
        any::<bool>().prop_map(Call::SetActive),
    ]
}

fn apply(o: &mut StandIn, c: &Call) {
    let r = match *c {
        Call::SetVideo(e, i) => {
            o.set_video_encoder2(e, i);
            None
        }
        Call::SetAudio(e, i) => {
            o.set_audio_encoder(e, i);
            None
        }
        Call::GetVideo2(i) => Some(o.get_video_encoder2(i)),
        Call::GetAudio(i) => Some(o.get_audio_encoder(i)),
        Call::GetVideo => Some(o.get_video_encoder()),
        Call::Audio => Some(o.audio_of()),
        Call::SetMixer(i) => {
            o.set_mixer(i);
            None
        }
        Call::Remove(e) => {
            o.remove_encoder_internal(e);
            None
        }
        Call::PreferredSize(w, h, i) => {
            o.set_preferred_size2(w, h, i);
            None
        }
        Call::SetActive(a) => {
            o.active = a;
            None
        }
    };
    if let Some(r) = r {
        o.push(format!("={r}"));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    /// The stand-in on the core against the same stand-in with
    /// origin/master's expressions inline, from any start.
    #[test]
    fn edge_proptest_stand_in(
        flags in prop_oneof![0u32..0x80, any::<u32>()],
        video in proptest::array::uniform10(encoder()),
        audio in proptest::array::uniform6(encoder()),
        scaled in (size(), size()),
        calls in proptest::collection::vec(call(), 1..12),
    ) {
        let mut core = StandIn::new(flags);
        core.video = video;
        core.audio = audio;
        (core.scaled_width, core.scaled_height) = scaled;
        let mut inline = core.clone();
        inline.inline = true;
        for c in &calls {
            apply(&mut core, c);
            apply(&mut inline, c);
        }
        inline.inline = false;
        prop_assert_eq!(core, inline);
    }
}
