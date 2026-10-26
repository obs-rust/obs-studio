//! Tier 1: `obs_output::encoder_lifecycle`, the encoder callback and
//! connection lifecycle decisions.
//!
//! The `steps_*` cases port `test/cmocka/test_encoder_lifecycle_steps.c`
//! 1:1: its 12 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones
//! included. The C calls the hidden `obs_encoder_lifecycle_*` and
//! `obs_encoder_audio_start_not_video` functions; here the safe core is
//! called directly. The C's stand-in encoder and its copies of
//! `obs_encoder_start_internal()`, `obs_encoder_stop()`, `add_connection()`,
//! `remove_connection()`, `gpu_encode_available()`, `full_stop()`,
//! `send_packet()` and `send_first_video_packet()` are [`Stand`], with the
//! group as [`Group`].
//!
//! The `edge_*` cases are new: every decision against the origin/master
//! expressions inline ([`ORIGIN`]) over every type from -1,000 to 1,000 and
//! the extremes, every cap word of the low 16 bits with and without the high
//! bits, counts and indexes around 0 and the ends of `u32` and `usize`, the
//! SEI truth table, the stand-in through every caller for every type, cap,
//! mix, request, SEI answer and group, and proptests of every decision over
//! full-range values and of the stand-in over call sequences with another
//! thread flipping the outputs, the mix, `initialized`, the SEI answer and
//! the group count between them.

use obs_output::encoder_lifecycle::{
    NOT_FOUND, OBS_ENCODER_CAP_PASS_TEXTURE, first, found, group_idle, group_ready, last,
    lcm_unset, not_audio, not_video, pass_texture, prepend_sei, started_dec, started_inc,
};
use proptest::prelude::*;

pub(crate) const TYPE_AUDIO: i32 = 0;
pub(crate) const TYPE_VIDEO: i32 = 1;
pub(crate) const CAP_PASS_TEXTURE: u32 = 2;

const EDGE_TYPE: [i32; 7] = [i32::MIN, -1, TYPE_AUDIO, TYPE_VIDEO, 2, 3, i32::MAX];
const EDGE_SIZE: [usize; 8] = [0, 1, 2, 3, 255, 0x7fff_ffff, usize::MAX - 1, usize::MAX];
const EDGE_U32: [u32; 9] = [
    0,
    1,
    2,
    3,
    6,
    0x7fff_ffff,
    0x8000_0000,
    u32::MAX - 1,
    u32::MAX,
];

// ------------------------------------------------------------------------
// The gates the callers ask, so the stand-in runs on the core ([`CORE`]) or
// on the origin/master expressions inline ([`ORIGIN`]).

pub(crate) struct Gates {
    pub(crate) first: fn(usize) -> bool,
    pub(crate) found: fn(usize) -> bool,
    pub(crate) last: fn(usize) -> bool,
    pub(crate) not_audio: fn(i32) -> bool,
    pub(crate) not_video: fn(i32) -> bool,
    pub(crate) pass_texture: fn(u32) -> bool,
    pub(crate) lcm_unset: fn(u32) -> bool,
    pub(crate) started_inc: fn(u32) -> u32,
    pub(crate) group_ready: fn(u32, usize) -> bool,
    pub(crate) started_dec: fn(u32) -> u32,
    pub(crate) group_idle: fn(u32) -> bool,
    pub(crate) prepend_sei: fn(bool, bool, usize) -> bool,
}

pub(crate) const CORE: Gates = Gates {
    first,
    found,
    last,
    not_audio,
    not_video,
    pass_texture,
    lcm_unset,
    started_inc,
    group_ready,
    started_dec,
    group_idle,
    prepend_sei,
};

/// origin/master's obs-encoder.c, its expressions inline.
pub(crate) const ORIGIN: Gates = Gates {
    first: |n| n == 0,
    found: |i| i != usize::MAX,
    last: |n| n == 0,
    not_audio: |t| t != TYPE_AUDIO,
    not_video: |t| t != TYPE_VIDEO,
    pass_texture: |c| (c & CAP_PASS_TEXTURE) != 0,
    lcm_unset: |l| l == 0,
    started_inc: |s| s.wrapping_add(1),
    group_ready: |s, n| s as usize >= n,
    started_dec: |s| s.wrapping_sub(1),
    group_idle: |s| s == 0,
    prepend_sei: |got, has, size| got && has && size != 0,
};

// ------------------------------------------------------------------------
// The stand-in encoder and the callers, as the C test's.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Cb {
    pub(crate) func: i32,
    pub(crate) param: i32,
    pub(crate) sent_first_packet: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Group {
    pub(crate) destroy_on_stop: bool,
    pub(crate) num_encoders: usize,
    pub(crate) started: u32,
    pub(crate) lcm: u32,
    pub(crate) resets: u32,
    pub(crate) ready_adds: u32,
    pub(crate) destroyed: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Output {
    pub(crate) active: bool,
    pub(crate) force_stops: u32,
    pub(crate) null_packets: u32,
}

/// The plugin's `get_sei_data()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Sei {
    /// no `get_sei_data`
    None,
    /// returns false, sets nothing
    False,
    /// returns true, data NULL, size 5
    Null,
    /// returns true, data set, size 0
    Empty,
    /// returns true, 3 bytes
    Data,
    /// returns false with 3 bytes set
    FalseSet,
}

pub(crate) const SEIS: [Sei; 6] = [
    Sei::None,
    Sei::False,
    Sei::Null,
    Sei::Empty,
    Sei::Data,
    Sei::FalseSet,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Stand {
    pub(crate) encoder_type: i32,
    pub(crate) caps: u32,
    pub(crate) has_data: bool,
    pub(crate) has_media: bool,
    pub(crate) has_mix: bool,
    pub(crate) nv12: bool,
    pub(crate) p010: bool,
    pub(crate) has_request_keyframe: bool,
    pub(crate) divisor: u32,
    pub(crate) sei: Sei,
    pub(crate) cbs: Vec<Cb>,
    /// in the [`Group`] the callers are given
    pub(crate) in_group: bool,
    pub(crate) outputs: Vec<Output>,
    pub(crate) initialized: bool,
    pub(crate) active: bool,
    pub(crate) gpu_encode_failed: bool,
    pub(crate) packet_times: usize,
    pub(crate) audio_connects: u32,
    pub(crate) audio_disconnects: u32,
    pub(crate) gpu_starts: u32,
    pub(crate) gpu_stops: u32,
    pub(crate) raw_starts: u32,
    pub(crate) raw_stops: u32,
    pub(crate) shutdowns: u32,
    pub(crate) keyframe_requests: u32,
    pub(crate) pause_resets: u32,
    pub(crate) mix_asks: u32,
    pub(crate) sei_asks: u32,
    pub(crate) packets: u32,
    pub(crate) last_size: usize,
    pub(crate) last_with_sei: bool,
}

/// `lcm()` and `gcd()` of obs-encoder.c, which stay C.
fn stand_gcd(a: i32, b: i32) -> i32 {
    if b == 0 { a } else { stand_gcd(b, a % b) }
}

fn stand_lcm(a: i32, b: i32) -> i32 {
    (a / stand_gcd(a, b)).wrapping_mul(b)
}

impl Stand {
    pub(crate) fn new(encoder_type: i32) -> Self {
        Self {
            encoder_type,
            caps: 0,
            has_data: true,
            has_media: true,
            has_mix: true,
            nv12: true,
            p010: false,
            has_request_keyframe: false,
            divisor: 1,
            sei: Sei::None,
            cbs: Vec::new(),
            in_group: false,
            outputs: Vec::new(),
            initialized: false,
            active: false,
            gpu_encode_failed: false,
            packet_times: 0,
            audio_connects: 0,
            audio_disconnects: 0,
            gpu_starts: 0,
            gpu_stops: 0,
            raw_starts: 0,
            raw_stops: 0,
            shutdowns: 0,
            keyframe_requests: 0,
            pause_resets: 0,
            mix_asks: 0,
            sei_asks: 0,
            packets: 0,
            last_size: 0,
            last_with_sei: false,
        }
    }

    /// `gpu_encode_available()`
    pub(crate) fn gpu_available(&mut self, g: &Gates) -> bool {
        self.mix_asks += 1;
        if !self.has_media || !self.has_mix {
            return false;
        }
        (g.pass_texture)(self.caps) && (self.p010 || self.nv12)
    }

    /// `add_connection()`
    pub(crate) fn add_connection(&mut self, g: &Gates, grp: &mut Group) {
        if !(g.not_audio)(self.encoder_type) {
            self.audio_connects += 1;
        } else if self.gpu_available(g) {
            self.gpu_starts += 1;
        } else {
            self.raw_starts += 1;
        }

        if self.in_group {
            if (g.lcm_unset)(grp.lcm) {
                grp.lcm = 1;
            }
            grp.lcm = stand_lcm(grp.lcm as i32, self.divisor as i32) as u32;

            let started = (g.started_inc)(grp.started);
            grp.started = started;
            let ready = (g.group_ready)(started, grp.num_encoders);
            if ready {
                grp.ready_adds += 1;
            }
        }

        self.active = true;
    }

    /// `remove_connection()`
    pub(crate) fn remove_connection(&mut self, g: &Gates, grp: &mut Group, shutdown: bool) {
        if !(g.not_audio)(self.encoder_type) {
            self.audio_disconnects += 1;
        } else if self.gpu_available(g) {
            self.gpu_stops += 1;
        } else {
            self.raw_stops += 1;
        }

        if self.in_group {
            let started = (g.started_dec)(grp.started);
            grp.started = started;
            if (g.group_idle)(started) {
                grp.resets += 1;
                grp.lcm = 1;
            }
        }

        if shutdown {
            self.shutdowns += 1;
        }
        self.initialized = false;
        self.active = false;
    }

    fn callback_idx(&self, func: i32, param: i32) -> usize {
        self.cbs
            .iter()
            .position(|c| c.func == func && c.param == param)
            .unwrap_or(NOT_FOUND)
    }

    /// `obs_encoder_start_internal()`
    pub(crate) fn start(&mut self, g: &Gates, grp: &mut Group, func: i32, param: i32) {
        if !self.has_data || !self.has_media {
            return;
        }

        let first = (g.first)(self.cbs.len());

        let idx = self.callback_idx(func, param);
        let added = !(g.found)(idx);
        if added {
            self.cbs.push(Cb {
                func,
                param,
                sent_first_packet: false,
            });
        }

        if first {
            self.pause_resets += 1;
            self.add_connection(g, grp);
        } else if added && !(g.not_video)(self.encoder_type) && self.has_request_keyframe {
            self.keyframe_requests += 1;
        }
    }

    /// `obs_encoder_stop()` after the checks
    pub(crate) fn stop(&mut self, g: &Gates, grp: &mut Group, func: i32, param: i32) {
        let mut last = false;
        let idx = self.callback_idx(func, param);
        if (g.found)(idx) {
            self.cbs.remove(idx);
            last = (g.last)(self.cbs.len());
        }

        self.packet_times = 0;

        if last {
            self.remove_connection(g, grp, true);
            if self.in_group && grp.destroy_on_stop && (g.group_idle)(grp.started) {
                grp.destroyed += 1;
            }
        }
    }

    /// `full_stop()` with an encoder
    pub(crate) fn full_stop(&mut self, g: &Gates, grp: &mut Group) {
        let gpu_encode = self.gpu_available(g);
        if gpu_encode {
            self.gpu_encode_failed = true;
        }

        for o in &mut self.outputs {
            if !o.active {
                continue;
            }
            o.force_stops += 1;
            o.null_packets += 1;
        }

        if gpu_encode {
            return;
        }

        self.cbs.clear();
        self.remove_connection(g, grp, false);
    }

    /// `get_sei()` and the plugin: (result, data set, size)
    fn get_sei(&mut self) -> (bool, bool, usize) {
        if self.sei == Sei::None {
            return (false, false, 0);
        }
        self.sei_asks += 1;
        match self.sei {
            Sei::None | Sei::False => (false, false, 0),
            Sei::Null => (true, false, 5),
            Sei::Empty => (true, true, 0),
            Sei::Data => (true, true, 3),
            Sei::FalseSet => (false, true, 3),
        }
    }

    fn deliver(&mut self, size: usize, with_sei: bool) {
        self.packets += 1;
        self.last_size = size;
        self.last_with_sei = with_sei;
    }

    /// `send_first_video_packet()`
    fn send_first(&mut self, g: &Gates, cb_idx: usize, keyframe: bool, size: usize) {
        if !keyframe {
            return;
        }

        let (got, has, sei_size) = self.get_sei();
        if !(g.prepend_sei)(got, has, sei_size) {
            self.deliver(size, false);
            self.cbs[cb_idx].sent_first_packet = true;
            return;
        }

        self.deliver(sei_size + size, true);
        self.cbs[cb_idx].sent_first_packet = true;
    }

    /// `send_packet()`
    pub(crate) fn send(&mut self, g: &Gates, cb_idx: usize, keyframe: bool, size: usize) {
        if !(g.not_video)(self.encoder_type) && !self.cbs[cb_idx].sent_first_packet {
            self.send_first(g, cb_idx, keyframe, size);
        } else {
            self.deliver(size, false);
        }
    }
}

// ------------------------------------------------------------------------
// The C test's cases, 1:1.

#[test]
fn steps_constants() {
    assert_eq!(obs_output::encoder_bind::OBS_ENCODER_AUDIO, 0);
    assert_eq!(OBS_ENCODER_CAP_PASS_TEXTURE, 2);
    assert_eq!(NOT_FOUND, usize::MAX);
    assert_eq!(obs_output::encoder_audio_start::OBS_ENCODER_VIDEO, 1);
}

#[test]
fn steps_first_found_last() {
    for n in EDGE_SIZE {
        assert_eq!(first(n), n == 0, "{n}");
        assert_eq!(last(n), n == 0, "{n}");
        assert_eq!(found(n), n != usize::MAX, "{n}");
    }
    assert!(!found(NOT_FOUND));
    assert!(found(0));
    assert!(found(usize::MAX - 1));
}

#[test]
fn steps_routes() {
    for t in EDGE_TYPE {
        assert_eq!(!not_audio(t), t == TYPE_AUDIO, "{t}");
        assert_eq!(!not_video(t), t == TYPE_VIDEO, "{t}");
    }
    assert!(not_audio(2));
    assert!(not_video(2));
}

#[test]
fn steps_pass_texture() {
    for caps in 0..256u32 {
        assert_eq!(pass_texture(caps), (caps & CAP_PASS_TEXTURE) != 0, "{caps}");
        assert_eq!(
            pass_texture(caps | 0xffff_0000),
            (caps & CAP_PASS_TEXTURE) != 0,
            "{caps}"
        );
    }
    assert!(!pass_texture(0));
    assert!(!pass_texture(1));
    assert!(pass_texture(CAP_PASS_TEXTURE));
    assert!(pass_texture(u32::MAX));
    assert!(!pass_texture(!CAP_PASS_TEXTURE));
}

#[test]
fn steps_group_counters() {
    for v in EDGE_U32 {
        assert_eq!(lcm_unset(v), v == 0, "{v}");
        assert_eq!(group_idle(v), v == 0, "{v}");
        assert_eq!(started_inc(v), v.wrapping_add(1), "{v}");
        assert_eq!(started_dec(v), v.wrapping_sub(1), "{v}");
        for n in EDGE_SIZE {
            assert_eq!(group_ready(v, n), v as usize >= n, "{v} {n}");
        }
    }
    // the counters wrap (characterized, not endorsed)
    assert_eq!(started_inc(u32::MAX), 0);
    assert_eq!(started_dec(0), u32::MAX);
    assert!(!group_ready(1, 2));
    assert!(group_ready(2, 2));
    assert!(group_ready(3, 2));
    assert!(group_ready(0, 0));
}

#[test]
fn steps_prepend_sei() {
    for got in [false, true] {
        for has in [false, true] {
            for size in EDGE_SIZE {
                assert_eq!(
                    prepend_sei(got, has, size),
                    got && has && size != 0,
                    "{got} {has} {size}"
                );
            }
        }
    }
    assert!(!prepend_sei(false, true, 3));
    assert!(!prepend_sei(true, true, 0));
    assert!(!prepend_sei(true, false, 5));
    assert!(prepend_sei(true, true, 1));
}

fn steps_start_callbacks_with(g: &Gates) {
    let mut grp = Group::default();
    for encoder_type in [TYPE_AUDIO, TYPE_VIDEO] {
        for kf in [false, true] {
            let mut e = Stand::new(encoder_type);
            e.has_request_keyframe = kf;
            let joins = u32::from(encoder_type == TYPE_VIDEO && kf);

            e.start(g, &mut grp, 1, 1);
            assert_eq!(e.cbs.len(), 1);
            assert!(e.active);
            assert_eq!(e.pause_resets, 1);
            assert_eq!(e.audio_connects + e.gpu_starts + e.raw_starts, 1);
            assert_eq!(e.keyframe_requests, 0);

            e.start(g, &mut grp, 1, 2);
            assert_eq!(e.cbs.len(), 2);
            assert_eq!(e.pause_resets, 1);
            assert_eq!(e.audio_connects + e.gpu_starts + e.raw_starts, 1);
            assert_eq!(e.keyframe_requests, joins);

            e.start(g, &mut grp, 2, 1);
            assert_eq!(e.cbs.len(), 3);
            assert_eq!(e.keyframe_requests, 2 * joins);

            // a duplicate: not added, no keyframe request
            e.start(g, &mut grp, 1, 2);
            assert_eq!(e.cbs.len(), 3);
            assert_eq!(e.keyframe_requests, 2 * joins);
            assert_eq!(e.audio_connects + e.gpu_starts + e.raw_starts, 1);
        }
    }

    // an unknown type: the video route, but no keyframe request
    let mut e = Stand::new(2);
    e.has_request_keyframe = true;
    e.start(g, &mut grp, 1, 1);
    e.start(g, &mut grp, 1, 2);
    assert_eq!(e.raw_starts + e.gpu_starts, 1);
    assert_eq!(e.keyframe_requests, 0);

    // no data or no media: nothing at all
    for i in 0..2 {
        let mut e = Stand::new(TYPE_VIDEO);
        if i == 1 {
            e.has_media = false;
        } else {
            e.has_data = false;
        }
        e.start(g, &mut grp, 1, 1);
        assert!(e.cbs.is_empty());
        assert!(!e.active);
        assert_eq!(e.pause_resets, 0);
        assert_eq!(e.mix_asks, 0);
    }
}

#[test]
fn steps_start_callbacks() {
    steps_start_callbacks_with(&CORE);
}

#[test]
fn steps_stop_callbacks() {
    let g = &CORE;
    let mut grp = Group::default();
    let mut e = Stand::new(TYPE_VIDEO);
    e.start(g, &mut grp, 1, 1);
    e.start(g, &mut grp, 1, 2);
    e.initialized = true;
    e.packet_times = 4;

    // a missing callback: nothing erased; the packet times cleared anyway
    // (characterized, not endorsed)
    e.stop(g, &mut grp, 2, 2);
    assert_eq!(e.cbs.len(), 2);
    assert!(e.active);
    assert_eq!(e.packet_times, 0);
    assert_eq!(e.raw_stops + e.gpu_stops, 0);

    // not the last
    e.packet_times = 2;
    e.stop(g, &mut grp, 1, 1);
    assert_eq!(e.cbs.len(), 1);
    assert_eq!(e.cbs[0].param, 2);
    assert!(e.active);
    assert!(e.initialized);
    assert_eq!(e.packet_times, 0);

    // the last
    e.stop(g, &mut grp, 1, 2);
    assert!(e.cbs.is_empty());
    assert!(!e.active);
    assert!(!e.initialized);
    assert_eq!(e.gpu_stops + e.raw_stops, 1);
    assert_eq!(e.shutdowns, 1);

    // again: missing
    e.stop(g, &mut grp, 1, 2);
    assert_eq!(e.gpu_stops + e.raw_stops, 1);
    assert_eq!(e.shutdowns, 1);

    // an audio encoder
    let mut e = Stand::new(TYPE_AUDIO);
    e.start(g, &mut grp, 3, 0);
    e.stop(g, &mut grp, 3, 0);
    assert_eq!(e.audio_connects, 1);
    assert_eq!(e.audio_disconnects, 1);
    assert_eq!(e.mix_asks, 0);
}

#[test]
fn steps_connect_routes() {
    let g = &CORE;
    let mut grp = Group::default();

    let mut e = Stand::new(TYPE_AUDIO);
    e.caps = CAP_PASS_TEXTURE;
    e.add_connection(g, &mut grp);
    assert_eq!(e.audio_connects, 1);
    assert_eq!(e.mix_asks, 0);

    for cap in [false, true] {
        for mix in [false, true] {
            for nv12 in [false, true] {
                for p010 in [false, true] {
                    let mut e = Stand::new(TYPE_VIDEO);
                    e.caps = if cap { CAP_PASS_TEXTURE | 1 } else { 1 };
                    e.has_mix = mix;
                    e.nv12 = nv12;
                    e.p010 = p010;
                    let gpu = u32::from(cap && mix && (nv12 || p010));
                    e.add_connection(g, &mut grp);
                    assert_eq!(e.gpu_starts, gpu);
                    assert_eq!(e.raw_starts, 1 - gpu);
                    e.remove_connection(g, &mut grp, false);
                    assert_eq!(e.gpu_stops, gpu);
                    assert_eq!(e.raw_stops, 1 - gpu);
                    assert_eq!(e.shutdowns, 0);
                    assert_eq!(e.mix_asks, 2);
                }
            }
        }
    }

    // the route is asked again at the stop (characterized, not endorsed)
    let mut e = Stand::new(TYPE_VIDEO);
    e.caps = CAP_PASS_TEXTURE;
    e.add_connection(g, &mut grp);
    e.nv12 = false;
    e.remove_connection(g, &mut grp, true);
    assert_eq!(e.gpu_starts, 1);
    assert_eq!(e.raw_stops, 1);
    assert_eq!(e.gpu_stops, 0);
    assert_eq!(e.shutdowns, 1);
}

#[test]
fn steps_group_lifecycle() {
    let g = &CORE;
    for destroy in [false, true] {
        let mut grp = Group {
            destroy_on_stop: destroy,
            num_encoders: 2,
            ..Group::default()
        };
        let mut a = Stand::new(TYPE_VIDEO);
        let mut b = Stand::new(TYPE_VIDEO);
        a.in_group = true;
        b.in_group = true;
        a.divisor = 2;
        b.divisor = 3;

        a.start(g, &mut grp, 1, 1);
        assert_eq!(grp.started, 1);
        assert_eq!(grp.lcm, 2);
        assert_eq!(grp.ready_adds, 0);

        b.start(g, &mut grp, 1, 1);
        assert_eq!(grp.started, 2);
        assert_eq!(grp.lcm, 6);
        assert_eq!(grp.ready_adds, 1);

        a.stop(g, &mut grp, 1, 1);
        assert_eq!(grp.started, 1);
        assert_eq!(grp.resets, 0);
        assert_eq!(grp.destroyed, 0);
        assert_eq!(grp.lcm, 6);

        b.stop(g, &mut grp, 1, 1);
        assert_eq!(grp.started, 0);
        assert_eq!(grp.resets, 1);
        assert_eq!(grp.lcm, 1);
        assert_eq!(grp.destroyed, u32::from(destroy));
    }

    // a group of one; an LCM already set
    let mut grp = Group {
        num_encoders: 1,
        lcm: 4,
        ..Group::default()
    };
    let mut a = Stand::new(TYPE_AUDIO);
    a.in_group = true;
    a.divisor = 6;
    a.start(g, &mut grp, 1, 1);
    assert_eq!(grp.ready_adds, 1);
    assert_eq!(grp.lcm, 12);

    // a divisor of 0 (characterized, not endorsed)
    let mut grp = Group {
        num_encoders: 3,
        ..Group::default()
    };
    let mut a = Stand::new(TYPE_VIDEO);
    let mut b = Stand::new(TYPE_VIDEO);
    a.in_group = true;
    b.in_group = true;
    a.divisor = 0;
    b.divisor = 2;
    a.start(g, &mut grp, 1, 1);
    assert_eq!(grp.lcm, 0);
    b.start(g, &mut grp, 1, 1);
    assert_eq!(grp.lcm, 2);
    assert_eq!(grp.ready_adds, 0);

    // a stop with none started wraps (characterized, not endorsed)
    let mut grp = Group {
        destroy_on_stop: true,
        num_encoders: 1,
        ..Group::default()
    };
    let mut a = Stand::new(TYPE_VIDEO);
    a.in_group = true;
    a.remove_connection(g, &mut grp, false);
    assert_eq!(grp.started, u32::MAX);
    assert_eq!(grp.resets, 0);
}

#[test]
fn steps_full_stop_outputs() {
    let g = &CORE;
    let mut grp = Group::default();
    let out = |active| Output {
        active,
        ..Output::default()
    };

    let mut e = Stand::new(TYPE_VIDEO);
    e.caps = 0;
    e.start(g, &mut grp, 1, 1);
    e.start(g, &mut grp, 1, 2);
    e.outputs = vec![out(true), out(false), out(true)];
    e.full_stop(g, &mut grp);
    assert_eq!(e.outputs[0].force_stops, 1);
    assert_eq!(e.outputs[1].force_stops, 0);
    assert_eq!(e.outputs[2].force_stops, 1);
    assert_eq!(e.outputs[0].null_packets, 1);
    assert_eq!(e.outputs[1].null_packets, 0);
    assert!(e.cbs.is_empty());
    assert!(!e.active);
    assert_eq!(e.raw_stops, 1);
    assert_eq!(e.shutdowns, 0);
    assert!(!e.gpu_encode_failed);

    let mut e = Stand::new(TYPE_VIDEO);
    e.caps = CAP_PASS_TEXTURE;
    e.start(g, &mut grp, 1, 1);
    e.outputs = vec![out(true), out(true)];
    e.full_stop(g, &mut grp);
    assert!(e.gpu_encode_failed);
    assert_eq!(e.outputs[0].force_stops, 1);
    assert_eq!(e.outputs[1].force_stops, 1);
    assert_eq!(e.cbs.len(), 1);
    assert!(e.active);
    assert_eq!(e.gpu_stops, 0);

    let mut e = Stand::new(TYPE_AUDIO);
    e.start(g, &mut grp, 1, 1);
    e.full_stop(g, &mut grp);
    assert_eq!(e.audio_disconnects, 1);
    assert!(e.cbs.is_empty());
    assert!(!e.gpu_encode_failed);
}

#[test]
fn steps_first_packets() {
    let g = &CORE;
    let mut grp = Group::default();
    let modes = [
        (Sei::None, false),
        (Sei::False, false),
        (Sei::Null, false),
        (Sei::Empty, false),
        (Sei::Data, true),
        (Sei::FalseSet, false),
    ];
    for (sei, with_sei) in modes {
        let asks = u32::from(sei != Sei::None);
        let mut e = Stand::new(TYPE_VIDEO);
        e.sei = sei;
        e.start(g, &mut grp, 1, 1);

        e.send(g, 0, false, 10);
        assert_eq!(e.packets, 0);
        assert!(!e.cbs[0].sent_first_packet);
        assert_eq!(e.sei_asks, 0);

        e.send(g, 0, true, 10);
        assert_eq!(e.packets, 1);
        assert!(e.cbs[0].sent_first_packet);
        assert_eq!(e.last_with_sei, with_sei);
        assert_eq!(e.last_size, if with_sei { 13 } else { 10 });
        assert_eq!(e.sei_asks, asks);

        e.send(g, 0, false, 7);
        assert_eq!(e.packets, 2);
        assert!(!e.last_with_sei);
        assert_eq!(e.last_size, 7);
        assert_eq!(e.sei_asks, asks);

        e.start(g, &mut grp, 1, 2);
        e.send(g, 1, false, 5);
        assert_eq!(e.packets, 2);
        e.send(g, 1, true, 5);
        assert_eq!(e.packets, 3);
        assert_eq!(e.last_with_sei, with_sei);
    }

    let mut e = Stand::new(TYPE_AUDIO);
    e.sei = Sei::Data;
    e.start(g, &mut grp, 1, 1);
    e.send(g, 0, false, 4);
    assert_eq!(e.packets, 1);
    assert!(!e.last_with_sei);
    assert!(!e.cbs[0].sent_first_packet);
    assert_eq!(e.sei_asks, 0);
}

// ------------------------------------------------------------------------
// Edge cases: the core against the origin/master expressions.

fn every_type() -> impl Iterator<Item = i32> {
    (-1000..=1000).chain([i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX])
}

fn every_size() -> impl Iterator<Item = usize> {
    (0..=10_000).chain(usize::MAX - 10_000..=usize::MAX).chain([
        0x7fff_ffff,
        0x8000_0000,
        0xffff_ffff,
        0x1_0000_0000,
    ])
}

fn every_u32() -> impl Iterator<Item = u32> {
    (0..=10_000)
        .chain(u32::MAX - 10_000..=u32::MAX)
        .chain([0x7fff_ffff, 0x8000_0000])
}

#[test]
fn edge_routes_every_type() {
    for t in every_type() {
        assert_eq!(not_audio(t), (ORIGIN.not_audio)(t), "{t}");
        assert_eq!(not_video(t), (ORIGIN.not_video)(t), "{t}");
        // no type is both audio and video
        assert!(not_audio(t) || not_video(t), "{t}");
    }
}

#[test]
fn edge_pass_texture_every_cap() {
    for low in 0..=0xffffu32 {
        for high in [0, 0x1_0000, 0xffff_0000] {
            let caps = low | high;
            assert_eq!(pass_texture(caps), (ORIGIN.pass_texture)(caps), "{caps}");
        }
    }
}

#[test]
fn edge_counts_and_indexes() {
    for n in every_size() {
        assert_eq!(first(n), (ORIGIN.first)(n), "{n}");
        assert_eq!(found(n), (ORIGIN.found)(n), "{n}");
        assert_eq!(last(n), (ORIGIN.last)(n), "{n}");
    }
    for v in every_u32() {
        assert_eq!(lcm_unset(v), (ORIGIN.lcm_unset)(v), "{v}");
        assert_eq!(group_idle(v), (ORIGIN.group_idle)(v), "{v}");
        assert_eq!(started_inc(v), (ORIGIN.started_inc)(v), "{v}");
        assert_eq!(started_dec(v), (ORIGIN.started_dec)(v), "{v}");
    }
    for s in (0..=64).chain(EDGE_U32) {
        for n in (0..=64).chain(EDGE_SIZE) {
            assert_eq!(group_ready(s, n), (ORIGIN.group_ready)(s, n), "{s} {n}");
        }
    }
}

#[test]
fn edge_prepend_sei_every_combination() {
    for got in [false, true] {
        for has in [false, true] {
            for size in (0..=64).chain(EDGE_SIZE) {
                assert_eq!(
                    prepend_sei(got, has, size),
                    (ORIGIN.prepend_sei)(got, has, size)
                );
            }
        }
    }
}

/// The calls the edge and proptest cases make.
#[derive(Clone, Debug)]
pub(crate) enum Call {
    Start(i32, i32),
    Stop(i32, i32),
    FullStop,
    Send(usize, bool, usize),
    /// another thread: an output's active flag, a mix flag, `initialized`,
    /// the SEI answer or the group count
    Flip(u8, u32),
}

pub(crate) fn apply(e: &mut Stand, g: &Gates, grp: &mut Group, c: &Call) {
    match c {
        Call::Start(f, p) => e.start(g, grp, *f, *p),
        Call::Stop(f, p) => e.stop(g, grp, *f, *p),
        Call::FullStop => e.full_stop(g, grp),
        Call::Send(i, k, s) => {
            if !e.cbs.is_empty() {
                let i = *i % e.cbs.len();
                e.send(g, i, *k, *s);
            }
        }
        Call::Flip(0, v) => {
            if !e.outputs.is_empty() {
                let n = e.outputs.len();
                let o = &mut e.outputs[*v as usize % n];
                o.active = !o.active;
            }
        }
        Call::Flip(1, _) => e.nv12 = !e.nv12,
        Call::Flip(2, _) => e.p010 = !e.p010,
        Call::Flip(3, _) => e.has_mix = !e.has_mix,
        Call::Flip(4, _) => e.initialized = !e.initialized,
        Call::Flip(5, v) => e.sei = SEIS[*v as usize % SEIS.len()],
        Call::Flip(_, v) => grp.started = *v % 4,
    }
}

/// A fixed run through every caller.
pub(crate) fn fixed_run(e: &mut Stand, g: &Gates, grp: &mut Group) {
    e.start(g, grp, 1, 1);
    e.send(g, 0, false, 9);
    e.send(g, 0, true, 9);
    e.start(g, grp, 1, 2);
    e.start(g, grp, 1, 2);
    if e.cbs.len() > 1 {
        e.send(g, 1, true, 4);
    }
    e.stop(g, grp, 2, 2);
    e.stop(g, grp, 1, 1);
    e.full_stop(g, grp);
    e.stop(g, grp, 1, 2);
    e.start(g, grp, 2, 2);
    e.stop(g, grp, 2, 2);
}

#[test]
fn edge_stand_in_every_combination() {
    let mut encoders = 0u32;
    for encoder_type in [-1, TYPE_AUDIO, TYPE_VIDEO, 2] {
        for caps in [0, 1, CAP_PASS_TEXTURE, 0xff] {
            for (has_mix, nv12, p010) in [
                (false, false, false),
                (true, false, false),
                (true, true, false),
                (true, false, true),
            ] {
                for kf in [false, true] {
                    for sei in SEIS {
                        for group in [None, Some((0u32, 1usize)), Some((1, 2)), Some((0, 0))] {
                            for destroy in [false, true] {
                                let mut a = Stand::new(encoder_type);
                                a.caps = caps;
                                a.has_mix = has_mix;
                                a.nv12 = nv12;
                                a.p010 = p010;
                                a.has_request_keyframe = kf;
                                a.sei = sei;
                                a.divisor = 2;
                                a.outputs = vec![
                                    Output {
                                        active: true,
                                        ..Output::default()
                                    },
                                    Output::default(),
                                ];
                                let mut ga = Group::default();
                                if let Some((started, num)) = group {
                                    a.in_group = true;
                                    ga.started = started;
                                    ga.num_encoders = num;
                                    ga.destroy_on_stop = destroy;
                                }
                                let mut b = a.clone();
                                let mut gb = ga.clone();
                                fixed_run(&mut a, &CORE, &mut ga);
                                fixed_run(&mut b, &ORIGIN, &mut gb);
                                assert_eq!(a, b);
                                assert_eq!(ga, gb);
                                encoders += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(encoders, 6144);
}

// ------------------------------------------------------------------------
// Proptests

pub(crate) fn call() -> impl Strategy<Value = Call> {
    prop_oneof![
        (0i32..3, 0i32..3).prop_map(|(f, p)| Call::Start(f, p)),
        (0i32..3, 0i32..3).prop_map(|(f, p)| Call::Stop(f, p)),
        Just(Call::FullStop),
        (0usize..4, any::<bool>(), 0usize..64).prop_map(|(i, k, s)| Call::Send(i, k, s)),
        (0u8..7, any::<u32>()).prop_map(|(w, v)| Call::Flip(w, v)),
    ]
}

pub(crate) fn sei() -> impl Strategy<Value = Sei> {
    (0usize..SEIS.len()).prop_map(|i| SEIS[i])
}

pub(crate) fn stand() -> impl Strategy<Value = (Stand, Group)> {
    (
        prop_oneof![Just(TYPE_AUDIO), Just(TYPE_VIDEO), any::<i32>()],
        prop_oneof![Just(0u32), Just(CAP_PASS_TEXTURE), any::<u32>()],
        any::<[bool; 6]>(),
        0u32..4,
        sei(),
        proptest::collection::vec(any::<bool>(), 0..4),
        proptest::option::of((0u32..4, 0usize..4, 0u32..3, any::<bool>())),
    )
        .prop_map(|(t, caps, flags, divisor, sei, outputs, group)| {
            let mut e = Stand::new(t);
            e.caps = caps;
            e.has_data = flags[0];
            e.has_media = flags[1];
            e.has_mix = flags[2];
            e.nv12 = flags[3];
            e.p010 = flags[4];
            e.has_request_keyframe = flags[5];
            e.divisor = divisor;
            e.sei = sei;
            e.outputs = outputs
                .into_iter()
                .map(|active| Output {
                    active,
                    ..Output::default()
                })
                .collect();
            let mut g = Group::default();
            if let Some((started, num, lcm, destroy)) = group {
                e.in_group = true;
                g.started = started;
                g.num_encoders = num;
                g.lcm = lcm;
                g.destroy_on_stop = destroy;
            }
            (e, g)
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
        n in any::<usize>(),
        t in any::<i32>(),
        caps in any::<u32>(),
        v in any::<u32>(),
        got in any::<bool>(),
        has in any::<bool>(),
    ) {
        prop_assert_eq!(first(n), (ORIGIN.first)(n));
        prop_assert_eq!(found(n), (ORIGIN.found)(n));
        prop_assert_eq!(last(n), (ORIGIN.last)(n));
        prop_assert_eq!(not_audio(t), (ORIGIN.not_audio)(t));
        prop_assert_eq!(not_video(t), (ORIGIN.not_video)(t));
        prop_assert_eq!(pass_texture(caps), (ORIGIN.pass_texture)(caps));
        prop_assert_eq!(lcm_unset(v), (ORIGIN.lcm_unset)(v));
        prop_assert_eq!(started_inc(v), (ORIGIN.started_inc)(v));
        prop_assert_eq!(group_ready(v, n), (ORIGIN.group_ready)(v, n));
        prop_assert_eq!(started_dec(v), (ORIGIN.started_dec)(v));
        prop_assert_eq!(group_idle(v), (ORIGIN.group_idle)(v));
        prop_assert_eq!(prepend_sei(got, has, n), (ORIGIN.prepend_sei)(got, has, n));
    }

    #[test]
    fn edge_proptest_stand_in(
        (e, g) in stand(),
        calls in proptest::collection::vec(call(), 1..12),
    ) {
        let (mut a, mut ga) = (e.clone(), g.clone());
        let (mut b, mut gb) = (e, g);
        for c in &calls {
            apply(&mut a, &CORE, &mut ga, c);
            apply(&mut b, &ORIGIN, &mut gb, c);
            prop_assert_eq!(&a, &b);
            prop_assert_eq!(&ga, &gb);
        }
    }
}
