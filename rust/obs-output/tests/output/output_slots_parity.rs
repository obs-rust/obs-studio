//! Tier 3 parity: the shims of `obs_output::ffi::output_slots` (and the safe
//! core under them) against the original `libobs/obs-output-slots.c.h`,
//! compiled as `oracle_obs_output_slots_*` (obs-c-oracle,
//! oracle/output_slots.c).
//!
//! Every output is compared exactly, on all three sides (oracle, shim,
//! core): the decisions as bools, the mixer mask and the slot as integers,
//! and whether the index was written (it starts as a sentinel).
//!
//! Runs: the seven constants; the first slot over every 16-bit mask under
//! 16 high words and a stride through all `u32`; the mixer rules over 0 to
//! 1023 and the edge indexes, the mask at every shift below the width of
//! `usize`; the type gates over -100,000 to 100,000 and the `i32` edges;
//! the index rules over every low 16-bit flag word under four high words at
//! indexes 0 to 11 and the edges; the default index, the scaled-size axis
//! and the bounds over 0 to 1023, the edges and a stride through all `u32`;
//! the callers (the two encoder setters, the getters,
//! `obs_output_get_video_encoder()`, `obs_output_audio()`, `set_mixer()`,
//! `remove_encoder_internal()` and `set_preferred_size2()`, as the C test's
//! stand-in output) on the oracle and on the shims, over every one- and
//! two-call sequence of 14 calls from 64 starts (13,440 sequences) and each
//! call from those starts made active, comparing every warning, active
//! load, reference, hookup, caption track, scaled size and result; and
//! proptests (20,000 cases each) of every decision over full-range values
//! and of the callers over sequences of up to 16 calls from any start.
//!
//! Intentional difference: `mixer_mask` with an index at or past the width
//! of `usize`, where the C's shift is undefined, is 0 in the port. The C
//! asks only 0 to 5. Tier 1 pins it; here the oracle is called only below
//! the width, which excludes exactly those inputs. Otherwise no input is
//! excluded. `test_output_slots_steps` runs the C test against the C; the
//! real-libobs output tests run against the hybrid libobs, where these
//! shims replace the C.

use obs_c_oracle::output_slots::{
    CONSTANT_NAMES, oracle_obs_output_slots_audio_index_ok, oracle_obs_output_slots_audio_slot,
    oracle_obs_output_slots_audio_type_ok, oracle_obs_output_slots_default_index,
    oracle_obs_output_slots_first_audio, oracle_obs_output_slots_first_video,
    oracle_obs_output_slots_mixer_in_range, oracle_obs_output_slots_mixer_mask,
    oracle_obs_output_slots_scaled_axis_set, oracle_obs_output_slots_video_index_ok,
    oracle_obs_output_slots_video_slot, oracle_obs_output_slots_video_type_ok,
    oracle_output_slots_constant,
};
use obs_output::ffi::output_slots::{
    obs_output_slots_audio_index_ok, obs_output_slots_audio_slot, obs_output_slots_audio_type_ok,
    obs_output_slots_default_index, obs_output_slots_first_audio, obs_output_slots_first_video,
    obs_output_slots_mixer_in_range, obs_output_slots_mixer_mask, obs_output_slots_scaled_axis_set,
    obs_output_slots_video_index_ok, obs_output_slots_video_slot, obs_output_slots_video_type_ok,
};
use obs_output::output_slots::{
    self as core, MAX_AUDIO_MIXES, MAX_OUTPUT_AUDIO_ENCODERS, MAX_OUTPUT_VIDEO_ENCODERS,
    OBS_ENCODER_AUDIO, OBS_ENCODER_VIDEO, OBS_OUTPUT_MULTI_TRACK_AUDIO,
    OBS_OUTPUT_MULTI_TRACK_VIDEO, SlotKind,
};
use proptest::prelude::*;

const MAXV: usize = MAX_OUTPUT_VIDEO_ENCODERS;
const MAXA: usize = MAX_OUTPUT_AUDIO_ENCODERS;
const F_VIDEO: u32 = 1 << 0;
const F_AUDIO: u32 = 1 << 1;
const F_ENCODED: u32 = 1 << 2;
const SENTINEL: usize = 0x5a5a_5a5a;

/// Calls one decision on the oracle and the shim, and checks they agree with
/// the core.
macro_rules! same {
    ($core:expr, $name:ident ($($arg:expr),*)) => {{
        let core_side = $core;
        // SAFETY: plain values in, a plain value out.
        let oracle = unsafe { paste_oracle!($name)($($arg),*) };
        let shim = $name($($arg),*);
        three(core_side, shim, oracle)
    }};
}

macro_rules! paste_oracle {
    (obs_output_slots_mixer_in_range) => {
        oracle_obs_output_slots_mixer_in_range
    };
    (obs_output_slots_mixer_mask) => {
        oracle_obs_output_slots_mixer_mask
    };
    (obs_output_slots_video_type_ok) => {
        oracle_obs_output_slots_video_type_ok
    };
    (obs_output_slots_audio_type_ok) => {
        oracle_obs_output_slots_audio_type_ok
    };
    (obs_output_slots_video_index_ok) => {
        oracle_obs_output_slots_video_index_ok
    };
    (obs_output_slots_audio_index_ok) => {
        oracle_obs_output_slots_audio_index_ok
    };
    (obs_output_slots_default_index) => {
        oracle_obs_output_slots_default_index
    };
    (obs_output_slots_scaled_axis_set) => {
        oracle_obs_output_slots_scaled_axis_set
    };
    (obs_output_slots_video_slot) => {
        oracle_obs_output_slots_video_slot
    };
    (obs_output_slots_audio_slot) => {
        oracle_obs_output_slots_audio_slot
    };
}

#[track_caller]
fn three<T: PartialEq + std::fmt::Debug + Copy>(core_side: T, shim: T, oracle: T) -> T {
    assert_eq!(shim, oracle, "shim vs oracle");
    assert_eq!(core_side, oracle, "core vs oracle");
    oracle
}

/// The first slot on all three sides: (found, index written or the
/// sentinel).
fn first(kind: SlotKind, mask: u32) -> (bool, usize) {
    let (mut o, mut s) = (SENTINEL, SENTINEL);
    let (fo, fs) = match kind {
        // SAFETY: both write at most one usize to a local.
        SlotKind::Video => unsafe {
            (
                oracle_obs_output_slots_first_video(mask, &raw mut o),
                obs_output_slots_first_video(mask, &raw mut s),
            )
        },
        // SAFETY: as above.
        SlotKind::Audio => unsafe {
            (
                oracle_obs_output_slots_first_audio(mask, &raw mut o),
                obs_output_slots_first_audio(mask, &raw mut s),
            )
        },
    };
    let c = core::first_slot(kind, mask);
    let c = (c.is_some(), c.unwrap_or(SENTINEL));
    three(c, (fs, s), (fo, o))
}

fn edge_usize() -> Vec<usize> {
    let mut v: Vec<usize> = (0..12).collect();
    v.extend([
        31,
        32,
        63,
        64,
        65,
        255,
        256,
        1 << 31,
        u32::MAX as usize,
        usize::MAX / 2,
        usize::MAX - 1,
        usize::MAX,
    ]);
    #[cfg(target_pointer_width = "64")]
    v.extend([1usize << 32, (1usize << 32) + 1, 1usize << 63]);
    v
}

#[test]
fn constants_match_c() {
    let rust: [i64; 7] = [
        i64::from(OBS_OUTPUT_MULTI_TRACK_AUDIO),
        i64::from(OBS_OUTPUT_MULTI_TRACK_VIDEO),
        MAXV as i64,
        MAXA as i64,
        MAX_AUDIO_MIXES as i64,
        i64::from(OBS_ENCODER_AUDIO),
        i64::from(OBS_ENCODER_VIDEO),
    ];
    for (i, (name, want)) in CONSTANT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: plain value in and out.
        assert_eq!(unsafe { oracle_output_slots_constant(i) }, want, "{name}");
    }
    // SAFETY: as above.
    assert_eq!(unsafe { oracle_output_slots_constant(7) }, -1);
}

#[test]
fn first_slot_over_masks() {
    for h in 0..16u32 {
        let high = if h == 0 {
            0
        } else {
            (h.wrapping_mul(0x1357_9bdf)) << 16
        };
        for m in 0..0x1_0000u32 {
            first(SlotKind::Video, m | high);
            first(SlotKind::Audio, m | high);
        }
    }
    let mut m = 0u32;
    loop {
        first(SlotKind::Video, m);
        first(SlotKind::Audio, m);
        match m.checked_add(0x0001_0003) {
            Some(n) => m = n,
            None => break,
        }
    }
    assert_eq!(first(SlotKind::Video, 1 << 9), (true, 9));
    assert_eq!(first(SlotKind::Audio, 1 << 6), (false, SENTINEL));
}

#[test]
fn mixer_rules() {
    for idx in (0..1024).chain(edge_usize()) {
        same!(
            core::mixer_in_range(idx),
            obs_output_slots_mixer_in_range(idx)
        );
    }
    for idx in 0..usize::BITS as usize {
        same!(core::mixer_mask(idx), obs_output_slots_mixer_mask(idx));
    }
}

#[test]
fn type_gates() {
    let edges = [i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX];
    for t in (-100_000..100_000).chain(edges) {
        same!(
            core::type_ok(SlotKind::Video, t),
            obs_output_slots_video_type_ok(t)
        );
        same!(
            core::type_ok(SlotKind::Audio, t),
            obs_output_slots_audio_type_ok(t)
        );
    }
}

#[test]
fn index_rules() {
    let idx = edge_usize();
    for high in [0, 0x1_0000, 0xffff_0000, 0x8000_0000u32] {
        for f in 0..0x1_0000u32 {
            let flags = f | high;
            for &i in &idx {
                same!(
                    core::index_ok(SlotKind::Video, flags, i),
                    obs_output_slots_video_index_ok(flags, i)
                );
                same!(
                    core::index_ok(SlotKind::Audio, flags, i),
                    obs_output_slots_audio_index_ok(flags, i)
                );
            }
        }
    }
}

#[test]
fn default_axis_and_bounds() {
    for i in (0..1024).chain(edge_usize()) {
        same!(core::default_index(i), obs_output_slots_default_index(i));
        same!(
            core::slot_exists(SlotKind::Video, i),
            obs_output_slots_video_slot(i)
        );
        same!(
            core::slot_exists(SlotKind::Audio, i),
            obs_output_slots_audio_slot(i)
        );
    }
    let mut size = 0u32;
    loop {
        same!(
            core::scaled_axis_set(size),
            obs_output_slots_scaled_axis_set(size)
        );
        match size.checked_add(if size < 1024 { 1 } else { 0x0001_0007 }) {
            Some(n) => size = n,
            None => break,
        }
    }
    same!(
        core::scaled_axis_set(u32::MAX),
        obs_output_slots_scaled_axis_set(u32::MAX)
    );
}

// ------------------------------------------------------------------------
// The callers, on the oracle and on the shims

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Oracle,
    Shim,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Encoder {
    id: i32,
    kind: i32,
    dying: bool,
}

const ENCODERS: [Encoder; 6] = [
    Encoder {
        id: 1,
        kind: OBS_ENCODER_VIDEO,
        dying: false,
    },
    Encoder {
        id: 2,
        kind: OBS_ENCODER_VIDEO,
        dying: false,
    },
    Encoder {
        id: 3,
        kind: OBS_ENCODER_AUDIO,
        dying: false,
    },
    Encoder {
        id: 4,
        kind: OBS_ENCODER_AUDIO,
        dying: false,
    },
    Encoder {
        id: 5,
        kind: 2,
        dying: false,
    },
    Encoder {
        id: 6,
        kind: OBS_ENCODER_VIDEO,
        dying: true,
    },
];

fn id(e: Option<Encoder>) -> i32 {
    e.map_or(0, |e| e.id)
}

/// The C test's stand-in output, its decisions asked of one side.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StandIn {
    flags: u32,
    active: bool,
    video: [Option<Encoder>; MAXV],
    audio: [Option<Encoder>; MAXA],
    captions: [u32; MAXV],
    scaled: (u32, u32),
    mixer_mask: usize,
    next_caption: u32,
    ev: Vec<String>,
}

impl StandIn {
    fn new(flags: u32) -> Self {
        Self {
            flags,
            active: false,
            video: [None; MAXV],
            audio: [None; MAXA],
            captions: [0; MAXV],
            scaled: (0, 0),
            mixer_mask: 0,
            next_caption: 0,
            ev: Vec::new(),
        }
    }

    fn push(&mut self, e: String) {
        self.ev.push(e);
    }
}

/// One side's decisions.
fn type_ok(s: Side, kind: SlotKind, t: i32) -> bool {
    // SAFETY: plain values.
    unsafe {
        match (s, kind) {
            (Side::Oracle, SlotKind::Video) => oracle_obs_output_slots_video_type_ok(t),
            (Side::Oracle, SlotKind::Audio) => oracle_obs_output_slots_audio_type_ok(t),
            (Side::Shim, SlotKind::Video) => obs_output_slots_video_type_ok(t),
            (Side::Shim, SlotKind::Audio) => obs_output_slots_audio_type_ok(t),
        }
    }
}

fn index_ok(s: Side, kind: SlotKind, flags: u32, idx: usize) -> bool {
    // SAFETY: plain values.
    unsafe {
        match (s, kind) {
            (Side::Oracle, SlotKind::Video) => oracle_obs_output_slots_video_index_ok(flags, idx),
            (Side::Oracle, SlotKind::Audio) => oracle_obs_output_slots_audio_index_ok(flags, idx),
            (Side::Shim, SlotKind::Video) => obs_output_slots_video_index_ok(flags, idx),
            (Side::Shim, SlotKind::Audio) => obs_output_slots_audio_index_ok(flags, idx),
        }
    }
}

fn slot(s: Side, kind: SlotKind, idx: usize) -> bool {
    // SAFETY: plain values.
    unsafe {
        match (s, kind) {
            (Side::Oracle, SlotKind::Video) => oracle_obs_output_slots_video_slot(idx),
            (Side::Oracle, SlotKind::Audio) => oracle_obs_output_slots_audio_slot(idx),
            (Side::Shim, SlotKind::Video) => obs_output_slots_video_slot(idx),
            (Side::Shim, SlotKind::Audio) => obs_output_slots_audio_slot(idx),
        }
    }
}

fn default_index(s: Side, idx: usize) -> bool {
    match s {
        // SAFETY: plain values.
        Side::Oracle => unsafe { oracle_obs_output_slots_default_index(idx) },
        Side::Shim => obs_output_slots_default_index(idx),
    }
}

fn axis(s: Side, size: u32) -> bool {
    match s {
        // SAFETY: plain values.
        Side::Oracle => unsafe { oracle_obs_output_slots_scaled_axis_set(size) },
        Side::Shim => obs_output_slots_scaled_axis_set(size),
    }
}

fn mixer_in_range(s: Side, idx: usize) -> bool {
    match s {
        // SAFETY: plain values.
        Side::Oracle => unsafe { oracle_obs_output_slots_mixer_in_range(idx) },
        Side::Shim => obs_output_slots_mixer_in_range(idx),
    }
}

fn mixer_mask(s: Side, idx: usize) -> usize {
    // Asked only in range, where the C's shift is defined.
    assert!(idx < usize::BITS as usize);
    match s {
        // SAFETY: plain values, idx below the width.
        Side::Oracle => unsafe { oracle_obs_output_slots_mixer_mask(idx) },
        Side::Shim => obs_output_slots_mixer_mask(idx),
    }
}

fn first_of(s: Side, kind: SlotKind, slots: &[Option<Encoder>]) -> Option<usize> {
    let mask = slots
        .iter()
        .enumerate()
        .filter(|(_, e)| e.is_some())
        .fold(0u32, |m, (i, _)| m | 1 << i);
    let mut idx = SENTINEL;
    // SAFETY: both write at most one usize to a local.
    let found = unsafe {
        match (s, kind) {
            (Side::Oracle, SlotKind::Video) => {
                oracle_obs_output_slots_first_video(mask, &raw mut idx)
            }
            (Side::Oracle, SlotKind::Audio) => {
                oracle_obs_output_slots_first_audio(mask, &raw mut idx)
            }
            (Side::Shim, SlotKind::Video) => obs_output_slots_first_video(mask, &raw mut idx),
            (Side::Shim, SlotKind::Audio) => obs_output_slots_first_audio(mask, &raw mut idx),
        }
    };
    found.then_some(idx)
}

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

fn log_flag(o: &mut StandIn, flag: u32, inverse_log: bool) -> bool {
    let ret = o.flags & flag != 0;
    if ret == inverse_log {
        o.push("warn-flag".into());
    }
    ret
}

fn load_active(o: &mut StandIn) -> bool {
    o.push("active?".into());
    o.active
}

fn set_encoder(s: Side, o: &mut StandIn, kind: SlotKind, encoder: Option<Encoder>, idx: usize) {
    let flag = if kind == SlotKind::Video {
        F_VIDEO
    } else {
        F_AUDIO
    };
    if !log_flag(o, F_ENCODED, false) || !log_flag(o, flag, false) {
        return;
    }
    if let Some(e) = encoder
        && !type_ok(s, kind, e.kind)
    {
        o.push("warn-type".into());
        return;
    }
    if load_active(o) {
        o.push("warn-active".into());
        return;
    }
    if !index_ok(s, kind, o.flags, idx) {
        return;
    }
    let cur = match kind {
        SlotKind::Video => o.video[idx],
        SlotKind::Audio => o.audio[idx],
    };
    if cur == encoder {
        return;
    }
    o.push(format!(
        "unhook{} release{} ref{}",
        id(cur),
        id(cur),
        id(encoder)
    ));
    let new = encoder.filter(|e| !e.dying);
    o.push(format!("hook{}", id(new)));
    match kind {
        SlotKind::Video => {
            o.video[idx] = new;
            if o.captions[idx] != 0 {
                o.push(format!("uncaption{idx}"));
            }
            o.captions[idx] = 0;
            // On the stored slot, as obs_output_set_video_encoder2 decides
            // since #2495.
            if new.is_some() {
                o.next_caption += 1;
                o.captions[idx] = o.next_caption;
                o.push(format!("caption{idx}"));
            }
            if default_index(s, idx) && new.is_some() && axis(s, o.scaled.0) && axis(s, o.scaled.1)
            {
                o.push(format!("size{}={:?}", id(new), o.scaled));
            }
        }
        SlotKind::Audio => o.audio[idx] = new,
    }
}

fn apply(s: Side, o: &mut StandIn, c: &Call) {
    let r = match *c {
        Call::SetVideo(e, i) => {
            set_encoder(s, o, SlotKind::Video, e, i);
            None
        }
        Call::SetAudio(e, i) => {
            set_encoder(s, o, SlotKind::Audio, e, i);
            None
        }
        Call::GetVideo2(i) => Some(if slot(s, SlotKind::Video, i) {
            id(o.video[i])
        } else {
            -1
        }),
        Call::GetAudio(i) => Some(if slot(s, SlotKind::Audio, i) {
            id(o.audio[i])
        } else {
            -1
        }),
        Call::GetVideo => Some(first_of(s, SlotKind::Video, &o.video).map_or(-1, |i| {
            if slot(s, SlotKind::Video, i) {
                id(o.video[i])
            } else {
                -1
            }
        })),
        Call::Audio => Some(if o.flags & F_ENCODED == 0 {
            100
        } else {
            first_of(s, SlotKind::Audio, &o.audio).map_or(-1, |i| id(o.audio[i]))
        }),
        Call::SetMixer(i) => {
            if !log_flag(o, F_ENCODED, true) {
                if !mixer_in_range(s, i) {
                    o.push("warn-mixer".into());
                } else if !load_active(o) {
                    o.mixer_mask = mixer_mask(s, i);
                }
            }
            None
        }
        Call::Remove(e) => {
            if type_ok(s, SlotKind::Video, e.kind) {
                for i in 0..MAXV {
                    if o.video[i] == Some(e) {
                        o.video[i] = None;
                        o.push(format!("release{}", e.id));
                    }
                }
            } else if type_ok(s, SlotKind::Audio, e.kind) {
                for i in 0..MAXA {
                    if o.audio[i] == Some(e) {
                        o.audio[i] = None;
                        o.push(format!("release{}", e.id));
                    }
                }
            }
            None
        }
        Call::PreferredSize(w, h, i) => {
            if log_flag(o, F_VIDEO, false) && slot(s, SlotKind::Video, i) {
                if load_active(o) {
                    o.push("warn-active".into());
                } else {
                    if default_index(s, i) {
                        o.scaled = (w, h);
                    }
                    if o.flags & F_ENCODED != 0 && o.video[i].is_some() {
                        o.push(format!("size{}={w}x{h}", id(o.video[i])));
                    }
                }
            }
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

fn run_both(start: &StandIn, calls: &[Call]) {
    let mut oracle = start.clone();
    let mut shim = start.clone();
    for c in calls {
        apply(Side::Oracle, &mut oracle, c);
        apply(Side::Shim, &mut shim, c);
    }
    assert_eq!(shim, oracle, "{calls:?} from {start:?}");
}

fn calls() -> Vec<Call> {
    let v = Some(ENCODERS[0]);
    let a = Some(ENCODERS[2]);
    vec![
        Call::SetVideo(v, 0),
        Call::SetVideo(Some(ENCODERS[5]), MAXV - 1),
        Call::SetVideo(None, 0),
        Call::SetVideo(a, 1),
        Call::SetAudio(a, 0),
        Call::SetAudio(Some(ENCODERS[3]), MAXA),
        Call::SetAudio(v, MAXA - 1),
        Call::GetVideo2(MAXV),
        Call::GetVideo,
        Call::Audio,
        Call::SetMixer(MAX_AUDIO_MIXES - 1),
        Call::SetMixer(MAX_AUDIO_MIXES),
        Call::Remove(ENCODERS[0]),
        Call::PreferredSize(1280, 720, 0),
    ]
}

fn starts() -> Vec<StandIn> {
    let mut v = Vec::new();
    for bits in 0..64u32 {
        // the flags: video, audio, encoded and the two multitrack ones
        let mut flags = bits & 0x7;
        if bits & 8 != 0 {
            flags |= OBS_OUTPUT_MULTI_TRACK_AUDIO;
        }
        if bits & 16 != 0 {
            flags |= OBS_OUTPUT_MULTI_TRACK_VIDEO;
        }
        let mut o = StandIn::new(flags);
        if bits & 32 != 0 {
            o.video[0] = Some(ENCODERS[1]);
            o.video[MAXV - 1] = Some(ENCODERS[0]);
            o.audio[1] = Some(ENCODERS[3]);
            o.captions[0] = 7;
            o.scaled = (1920, 1080);
        }
        v.push(o);
    }
    v
}

#[test]
fn callers_on_the_stand_in() {
    let calls = calls();
    let starts = starts();
    let mut n = 0;
    for start in &starts {
        for a in &calls {
            run_both(start, std::slice::from_ref(a));
            n += 1;
            for b in &calls {
                run_both(start, &[a.clone(), b.clone()]);
                n += 1;
            }
        }
        let mut active = start.clone();
        active.active = true;
        for a in &calls {
            run_both(&active, std::slice::from_ref(a));
        }
    }
    assert_eq!(n, 64 * (14 + 14 * 14));
}

fn arb_encoder() -> impl Strategy<Value = Option<Encoder>> {
    prop_oneof![
        Just(None),
        (0..ENCODERS.len()).prop_map(|i| Some(ENCODERS[i]))
    ]
}

fn arb_index() -> impl Strategy<Value = usize> {
    prop_oneof![4 => 0usize..12, 1 => Just(usize::MAX), 1 => any::<usize>()]
}

fn arb_size() -> impl Strategy<Value = u32> {
    prop_oneof![Just(0u32), Just(1280u32), any::<u32>()]
}

fn arb_call() -> impl Strategy<Value = Call> {
    prop_oneof![
        (arb_encoder(), arb_index()).prop_map(|(e, i)| Call::SetVideo(e, i)),
        (arb_encoder(), arb_index()).prop_map(|(e, i)| Call::SetAudio(e, i)),
        arb_index().prop_map(Call::GetVideo2),
        arb_index().prop_map(Call::GetAudio),
        Just(Call::GetVideo),
        Just(Call::Audio),
        arb_index().prop_map(Call::SetMixer),
        (0..ENCODERS.len()).prop_map(|i| Call::Remove(ENCODERS[i])),
        (arb_size(), arb_size(), arb_index()).prop_map(|(w, h, i)| Call::PreferredSize(w, h, i)),
        any::<bool>().prop_map(Call::SetActive),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn proptest_decisions(mask: u32, flags: u32, idx: usize, small in 0usize..80, t: i32, size: u32) {
        first(SlotKind::Video, mask);
        first(SlotKind::Audio, mask);
        for i in [idx, small] {
            same!(core::index_ok(SlotKind::Video, flags, i), obs_output_slots_video_index_ok(flags, i));
            same!(core::index_ok(SlotKind::Audio, flags, i), obs_output_slots_audio_index_ok(flags, i));
            same!(core::slot_exists(SlotKind::Video, i), obs_output_slots_video_slot(i));
            same!(core::slot_exists(SlotKind::Audio, i), obs_output_slots_audio_slot(i));
            same!(core::default_index(i), obs_output_slots_default_index(i));
            same!(core::mixer_in_range(i), obs_output_slots_mixer_in_range(i));
            if i < usize::BITS as usize {
                same!(core::mixer_mask(i), obs_output_slots_mixer_mask(i));
            }
        }
        same!(core::type_ok(SlotKind::Video, t), obs_output_slots_video_type_ok(t));
        same!(core::type_ok(SlotKind::Audio, t), obs_output_slots_audio_type_ok(t));
        same!(core::scaled_axis_set(size), obs_output_slots_scaled_axis_set(size));
    }

    #[test]
    fn proptest_callers(
        flags in prop_oneof![0u32..0x80, any::<u32>()],
        active: bool,
        video in proptest::array::uniform10(arb_encoder()),
        audio in proptest::array::uniform6(arb_encoder()),
        scaled in (arb_size(), arb_size()),
        calls in proptest::collection::vec(arb_call(), 1..17),
    ) {
        let mut start = StandIn::new(flags);
        start.active = active;
        start.video = video;
        start.audio = audio;
        start.scaled = scaled;
        run_both(&start, &calls);
    }
}
