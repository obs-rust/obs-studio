//! Tier 1: `obs_audio_monitoring::pulse_monitor`, the PulseAudio monitor's
//! open and write steps.
//!
//! The `steps_*` cases port `test/cmocka/test_pulse_monitor_steps.c` 1:1: its
//! 20 cases under the same names, in the same order, with the same values and
//! expectations, the "characterized, not endorsed" ones included. The C calls
//! the hidden `pulseaudio_monitor_*` functions; here the safe core is called
//! directly. The C's stand-ins (the write loop of `do_stream_write()`, the
//! routes of `audio_monitor_init()` and `audio_monitor_reset()`) are
//! [`StandInWrite`], [`stand_in_open`] and [`stand_in_reset`].
//!
//! The `edge_*` cases are new: every decision against a model of the
//! origin/master C (its expressions inline) over edge values, every flag
//! bit, event word and context state, and proptests of every decision over
//! full-range inputs and of the write loop over queues and server asks.

use obs_audio_monitoring::pulse_format::{
    SAMPLE_FLOAT32LE, SAMPLE_INVALID, SAMPLE_MAX, SAMPLE_S16LE, SAMPLE_U8,
};
use obs_audio_monitoring::pulse_monitor::{
    CONTEXT_AUTHORIZING, CONTEXT_CONNECTING, CONTEXT_FAILED, CONTEXT_READY, CONTEXT_SETTING_NAME,
    CONTEXT_TERMINATED, CONTEXT_UNCONNECTED, DO_NOT_SELF_MONITOR, EPSILON, SinkResult,
    TLENGTH_USEC, context_good, event_queued, fallback, format_invalid, grow, grown_tlength,
    has_device, is_default, last, needs_volume, open_flags, playback_bytes, playback_idle,
    self_monitor_check, sink_named, sink_result, tlength_usec, uncork, write_more, write_short,
};
use obs_audio_monitoring::pulse_stream_flags::{
    STREAM_DONT_MOVE, SUBSCRIPTION_EVENT_CHANGE, SUBSCRIPTION_EVENT_FACILITY_MASK,
    SUBSCRIPTION_EVENT_NEW, SUBSCRIPTION_EVENT_REMOVE, SUBSCRIPTION_EVENT_SINK,
    SUBSCRIPTION_EVENT_SINK_INPUT, SUBSCRIPTION_EVENT_SOURCE, SUBSCRIPTION_EVENT_TYPE_MASK,
};
use obs_audio_monitoring::stream_flags;
use proptest::prelude::*;

/// `PA_SUBSCRIPTION_EVENT_SERVER`.
const SUBSCRIPTION_EVENT_SERVER: u32 = 0x0007;
/// `PA_SUBSCRIPTION_EVENT_CARD`.
const SUBSCRIPTION_EVENT_CARD: u32 = 0x0009;

/// `uint_fast32_t` is 64 bits wide on this host (glibc and Bionic on 64-bit).
const FAST32_WIDE: bool = cfg!(any(
    all(
        target_os = "linux",
        target_env = "gnu",
        target_pointer_width = "64"
    ),
    all(target_os = "android", target_pointer_width = "64")
));

const U32_MAX: usize = 0xffff_ffff;

// ---- models of the origin/master C, expressions inline ----

fn c_grow(queued: usize, tlength: u32) -> bool {
    // data->new_data.size > data->attr.tlength * 2
    queued > tlength.wrapping_mul(2) as usize
}
fn c_grown_tlength(queued: usize) -> u32 {
    // data->attr.tlength = data->new_data.size
    queued as u32
}
fn c_uncork(queued: usize, tlength: u32) -> bool {
    queued >= tlength as usize
}
fn c_write_short(asked: usize, queued: usize) -> bool {
    asked > queued
}
fn c_playback_bytes(bytes_per_frame: u64, frames: u32, wide: bool) -> usize {
    // monitor->bytes_per_frame * resample_frames, assigned to size_t
    if wide {
        bytes_per_frame.wrapping_mul(u64::from(frames)) as usize
    } else {
        (bytes_per_frame as u32).wrapping_mul(frames) as usize
    }
}
// The C's `!(a <= b)`, which is true for NaN.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
fn c_needs_volume(vol: f32) -> bool {
    // !close_float(vol, 1.0f, EPSILON): !(fabsf(vol - 1.0f) <= EPSILON)
    !((vol - 1.0f32).abs() <= 1e-4f32)
}
fn c_sink_result(eol: i32) -> SinkResult {
    if eol < 0 {
        SinkResult::Error
    } else if eol > 0 {
        SinkResult::End
    } else {
        SinkResult::Info
    }
}
fn c_event_queued(t: u32) -> bool {
    let ty = t & SUBSCRIPTION_EVENT_TYPE_MASK;
    !(ty != SUBSCRIPTION_EVENT_NEW && ty != SUBSCRIPTION_EVENT_REMOVE)
}
fn c_context_good(state: u32) -> bool {
    // PA_CONTEXT_IS_GOOD()
    state == CONTEXT_CONNECTING
        || state == CONTEXT_AUTHORIZING
        || state == CONTEXT_SETTING_NAME
        || state == CONTEXT_READY
}

// ---- the C test's stand-ins ----

/// `struct stand_in_write`: `do_stream_write()` put together, with the
/// stream's calls replaced by what the server asks.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct StandInWrite {
    queued: usize,
    tlength: u32,
    corked: bool,
    grew: u32,
    uncorked: bool,
    written: usize,
    writes: u32,
    cancelled: bool,
}

impl StandInWrite {
    fn new(queued: usize, tlength: u32, corked: bool) -> Self {
        Self {
            queued,
            tlength,
            corked,
            ..Self::default()
        }
    }

    fn run(&mut self, asks: &[usize]) {
        if grow(self.queued, self.tlength) {
            self.tlength = grown_tlength(self.queued);
            self.grew += 1;
        }
        if self.corked {
            if !uncork(self.queued, self.tlength) {
                return;
            }
            self.corked = false;
            self.uncorked = true;
        }
        let mut k = 0;
        while write_more(self.queued) {
            let Some(&ask) = asks.get(k) else {
                return; // pa_stream_begin_write() failed
            };
            k += 1;
            if write_short(ask, self.queued) {
                self.cancelled = true;
                return;
            }
            self.queued -= ask;
            self.written += ask;
            self.writes += 1;
        }
    }

    /// The same loop with origin/master's expressions inline.
    fn run_c(&mut self, asks: &[usize]) {
        if c_grow(self.queued, self.tlength) {
            self.tlength = c_grown_tlength(self.queued);
            self.grew += 1;
        }
        if self.corked {
            if !c_uncork(self.queued, self.tlength) {
                return;
            }
            self.corked = false;
            self.uncorked = true;
        }
        let mut k = 0;
        while self.queued > 0 {
            let Some(&ask) = asks.get(k) else {
                return;
            };
            k += 1;
            if c_write_short(ask, self.queued) {
                self.cancelled = true;
                return;
            }
            self.queued -= ask;
            self.written += ask;
            self.writes += 1;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    Fail,
    Ignore,
    Default,
    Named,
    Fallback,
}

struct OpenIn<'a> {
    id: Option<&'a [u8]>,
    output_flags: u32,
    matched: bool,
    fmt: i32,
    default_id: Option<&'a [u8]>,
    fallback_fmt: i32,
}

/// `stand_in_open()`: `audio_monitor_init()`'s decisions in its order.
fn stand_in_open(input: &OpenIn<'_>) -> (Route, u32) {
    let Some(id) = input.id.filter(|_| has_device(input.id)) else {
        return (Route::Fail, 0);
    };
    if self_monitor_check(input.output_flags) && input.matched {
        return (Route::Ignore, 0);
    }
    let default = is_default(id);
    let mut fell_back = false;
    let mut fmt = input.fmt;
    if fallback(fmt, id) {
        fell_back = true;
        if !sink_named(input.default_id) {
            return (Route::Fail, 0);
        }
        fmt = input.fallback_fmt;
    }
    if format_invalid(fmt) {
        return (Route::Fail, 0);
    }
    let flags = open_flags(fell_back, Some(id));
    let route = if fell_back {
        Route::Fallback
    } else if default {
        Route::Default
    } else {
        Route::Named
    };
    (route, flags)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResetRoute {
    Replace,
    Keep,
    Ignore,
}

/// `stand_in_reset()`: `audio_monitor_reset()`'s branches.
fn stand_in_reset(init_ok: bool, has_stream: bool, has_context: bool, state: u32) -> ResetRoute {
    if init_ok {
        return ResetRoute::Replace;
    }
    if !has_stream {
        return ResetRoute::Keep;
    }
    let good = has_context && context_good(state);
    if good {
        ResetRoute::Keep
    } else {
        ResetRoute::Ignore
    }
}

// ---- steps_*: test_pulse_monitor_steps.c, 1:1 ----

#[test]
fn steps_copied_constants() {
    assert_eq!(DO_NOT_SELF_MONITOR, 1 << 9);
    assert_eq!(SinkResult::Info as u32, 0);
    assert_eq!(SinkResult::End as u32, 1);
    assert_eq!(SinkResult::Error as u32, 2);
}

#[test]
fn steps_grow_is_strictly_past_twice_tlength() {
    for tl in [1u32, 2, 4800, 9600, 1_000_000, 0x7fff_ffff] {
        let twice = tl as usize * 2;
        assert!(!grow(twice - 1, tl), "tlength {tl}: 2 * tlength - 1 grew");
        assert!(
            !grow(twice, tl),
            "tlength {tl}: 2 * tlength grew (the test is >)"
        );
        assert!(
            grow(twice + 1, tl),
            "tlength {tl}: 2 * tlength + 1 did not grow"
        );
    }
    assert!(!grow(0, 0));
    assert!(grow(1, 0));
    assert!(!grow(0, 4800));
    assert!(grow(usize::MAX, 4800));
}

#[test]
fn steps_grow_product_wraps_at_2_31() {
    // Characterized, not endorsed: tlength * 2 wraps in uint32_t.
    assert!(grow(1, 0x8000_0000));
    assert!(!grow(0, 0x8000_0000));
    assert!(!grow(2, 0x8000_0001));
    assert!(grow(3, 0x8000_0001));
    assert!(!grow(0xffff_fffe, 0xffff_ffff));
    assert!(grow(0xffff_ffff, 0xffff_ffff));
    if usize::MAX > U32_MAX {
        // Not wrapped, it would need more than 2^32 bytes.
        assert!(grow(U32_MAX + 1, 0x8000_0000));
    }
}

#[test]
fn steps_grown_tlength_truncates() {
    for q in [0usize, 1, 9601, 0x7fff_ffff, U32_MAX] {
        assert_eq!(grown_tlength(q), q as u32);
    }
    if usize::MAX > U32_MAX {
        // Characterized, not endorsed: past UINT32_MAX the buffer shrinks.
        assert_eq!(grown_tlength(U32_MAX + 6), 5);
        assert_eq!(grown_tlength(U32_MAX + 1), 0);
        assert_eq!(grown_tlength(usize::MAX), 0xffff_ffff);
    }
}

#[test]
fn steps_uncork_at_a_whole_tlength() {
    for tl in [1u32, 4800, 9600, 0x7fff_ffff, 0xffff_ffff] {
        let t = tl as usize;
        assert!(!uncork(t - 1, tl), "tlength {tl}: uncorked one byte short");
        assert!(
            uncork(t, tl),
            "tlength {tl}: stayed corked at a whole tlength"
        );
        assert!(uncork(t + 1, tl), "tlength {tl}: stayed corked past it");
    }
    // A tlength of 0 uncorks at once, even with nothing queued.
    assert!(uncork(0, 0));
}

#[test]
fn steps_write_loop_and_short_write() {
    assert!(!write_more(0));
    assert!(write_more(1));
    assert!(write_more(usize::MAX));

    for q in [1usize, 4800, 9600, usize::MAX - 1] {
        assert!(write_short(q + 1, q), "queue {q}: asked for more and wrote");
        assert!(!write_short(q, q), "queue {q}: asked for all and cancelled");
        assert!(
            !write_short(q - 1, q),
            "queue {q}: asked for less and cancelled"
        );
    }
    assert!(!write_short(0, 0));
    assert!(write_short(usize::MAX, 0));
}

#[test]
fn steps_write_end_to_end() {
    // 25 ms at 48 kHz stereo float: 9600 bytes.
    let tl: u32 = 9600;
    let t = tl as usize;

    // Corked, one byte short: nothing is written, the buffer stays.
    let mut a = StandInWrite::new(t - 1, tl, true);
    a.run(&[t - 1]);
    assert!(!a.uncorked);
    assert_eq!(a.written, 0);
    assert_eq!(a.tlength, tl);

    // Corked, a whole tlength: uncorked and written in one go.
    let mut b = StandInWrite::new(t, tl, true);
    b.run(&[t]);
    assert!(b.uncorked);
    assert_eq!(b.written, t);
    assert_eq!(b.queued, 0);

    // The server asks for more than is queued: cancel and wait, nothing
    // popped.
    let mut c = StandInWrite::new(4000, tl, false);
    c.run(&[4001]);
    assert!(c.cancelled);
    assert_eq!(c.written, 0);
    assert_eq!(c.queued, 4000);

    // Asked in pieces: written until the queue is empty.
    let mut d = StandInWrite::new(5000, tl, false);
    d.run(&[2000, 2000, 1000]);
    assert_eq!(d.writes, 3);
    assert_eq!(d.written, 5000);
    assert!(!d.cancelled);

    // A piece, then more than is left: one write is kept, the rest waits.
    let mut e = StandInWrite::new(5000, tl, false);
    e.run(&[3000, 2001]);
    assert_eq!(e.writes, 1);
    assert_eq!(e.queued, 2000);
    assert!(e.cancelled);

    // Past twice tlength: the buffer grows to the queue, which then
    // uncorks a corked stream.
    let mut f = StandInWrite::new(2 * t + 1, tl, true);
    f.run(&[2 * t + 1]);
    assert_eq!(f.grew, 1);
    assert_eq!(f.tlength, 2 * tl + 1);
    assert!(f.uncorked);
    assert_eq!(f.written, 2 * t + 1);

    // At exactly twice tlength it does not grow.
    let mut g = StandInWrite::new(2 * t, tl, true);
    g.run(&[]);
    assert_eq!(g.grew, 0);
    assert_eq!(g.tlength, tl);
    assert!(g.uncorked);
}

#[test]
fn steps_playback_skips_inactive_sources() {
    assert!(playback_idle(0));
    assert!(!playback_idle(1));
    assert!(!playback_idle(2));
    assert!(!playback_idle(i64::MAX));
    // Characterized, not endorsed: a negative count is not idle.
    assert!(!playback_idle(-1));
    assert!(!playback_idle(i64::MIN));
}

#[test]
fn steps_playback_bytes() {
    assert_eq!(playback_bytes(8, 480, FAST32_WIDE), 3840);
    assert_eq!(playback_bytes(8, 0, FAST32_WIDE), 0);
    assert_eq!(playback_bytes(0, 480, FAST32_WIDE), 0);
    assert_eq!(playback_bytes(1, 0xffff_ffff, FAST32_WIDE), U32_MAX);
    // A product in uint_fast32_t: 64 bits on glibc, 32 on FreeBSD.
    let big = playback_bytes(32, 0x1000_0000, FAST32_WIDE);
    if FAST32_WIDE && usize::MAX > U32_MAX {
        assert_eq!(big, 32 * 0x1000_0000);
    } else {
        assert_eq!(big, 0);
    }
}

#[test]
fn steps_volume_step_unless_close_to_one() {
    assert!(!needs_volume(1.0));
    // The last floats within EPSILON (1e-4f) of 1, and the next ones.
    assert!(!needs_volume(f32::from_bits(0x3f80_0346)));
    assert!(needs_volume(f32::from_bits(0x3f80_0347)));
    assert!(!needs_volume(f32::from_bits(0x3f7f_f973)));
    assert!(needs_volume(f32::from_bits(0x3f7f_f972)));
    // 1 + EPSILON and 1 - EPSILON round past it in float.
    assert!(needs_volume(1.0 + 1e-4));
    assert!(needs_volume(1.0 - 1e-4));
    assert!(!needs_volume(1.00005));
    assert!(!needs_volume(0.99995));
    assert!(needs_volume(0.0));
    assert!(needs_volume(-0.0));
    assert!(needs_volume(0.5));
    assert!(needs_volume(2.0));
    assert!(needs_volume(-1.0));
    assert!(needs_volume(f32::INFINITY));
    assert!(needs_volume(f32::NEG_INFINITY));
    // Characterized, not endorsed: NaN is processed.
    assert!(needs_volume(f32::NAN));
}

#[test]
fn steps_sink_info_results() {
    assert_eq!(sink_result(0), SinkResult::Info);
    for eol in [1, 2, 1000, i32::MAX] {
        assert_eq!(sink_result(eol), SinkResult::End, "eol {eol}");
    }
    for eol in [-1, -2, -1000, i32::MIN] {
        assert_eq!(sink_result(eol), SinkResult::Error, "eol {eol}");
    }
}

#[test]
fn steps_device_and_default() {
    assert!(!has_device(None));
    assert!(has_device(Some(b"")));
    assert!(has_device(Some(b"default")));

    assert!(is_default(b"default"));
    for id in [
        &b""[..],
        b"Default",
        b"default ",
        b"defaults",
        b"default.monitor",
        b"sink",
    ] {
        assert!(
            !is_default(id),
            "'{}' taken as default",
            String::from_utf8_lossy(id)
        );
    }
}

#[test]
fn steps_self_monitor_flag() {
    for bit in 0..32u32 {
        let flags = 1u32 << bit;
        assert_eq!(self_monitor_check(flags), bit == 9, "flag bit {bit}");
        assert_eq!(
            self_monitor_check(!flags),
            bit != 9,
            "flags without bit {bit}"
        );
    }
    assert!(!self_monitor_check(0));
    assert!(self_monitor_check(u32::MAX));
}

#[test]
fn steps_fallback_rule() {
    for id in [
        &b"sink.a"[..],
        b"alsa_output.usb",
        b"defaults",
        b"Default",
        b"",
    ] {
        let name = String::from_utf8_lossy(id);
        assert!(fallback(SAMPLE_INVALID, id), "'{name}' gone: no fallback");
        assert!(
            !fallback(SAMPLE_FLOAT32LE, id),
            "'{name}' present: fell back"
        );
        assert!(!fallback(SAMPLE_S16LE, id), "'{name}' present: fell back");
    }
    assert!(!fallback(SAMPLE_INVALID, b"default"));
    assert!(!fallback(SAMPLE_U8, b"default"));
    assert!(!fallback(SAMPLE_MAX, b"sink.a"));

    assert!(!sink_named(None));
    assert!(!sink_named(Some(b"")));
    assert!(sink_named(Some(b"x")));
    assert!(sink_named(Some(b"default")));

    assert!(format_invalid(SAMPLE_INVALID));
    for f in 0..=SAMPLE_MAX {
        assert!(!format_invalid(f), "format {f} taken as invalid");
    }
    assert!(!format_invalid(-2));
}

#[test]
fn steps_buffer_and_flags() {
    assert_eq!(tlength_usec(), 25_000);

    for id in [Some(&b"default"[..]), Some(b"sink.a"), Some(b""), None] {
        assert_eq!(
            open_flags(true, id),
            stream_flags(Some(b"default")),
            "fallback stream for {id:?} does not have the flags of default"
        );
        assert_eq!(
            open_flags(false, id),
            stream_flags(id),
            "stream for {id:?} does not have its own flags"
        );
    }
    assert_eq!(open_flags(true, Some(b"sink.a")) & STREAM_DONT_MOVE, 0);
    assert_ne!(open_flags(false, Some(b"sink.a")) & STREAM_DONT_MOVE, 0);
    assert_eq!(open_flags(false, Some(b"default")) & STREAM_DONT_MOVE, 0);
}

#[test]
fn steps_open_routes() {
    let s = DO_NOT_SELF_MONITOR;
    let f = SAMPLE_FLOAT32LE;
    let i = SAMPLE_INVALID;
    let some = |x: &'static [u8]| Some(x);
    #[rustfmt::skip]
    let cases: [(OpenIn<'_>, Route, bool); 14] = [
        (OpenIn { id: None, output_flags: 0, matched: false, fmt: f, default_id: some(b"s"), fallback_fmt: f }, Route::Fail, false),
        (OpenIn { id: None, output_flags: s, matched: true, fmt: f, default_id: some(b"s"), fallback_fmt: f }, Route::Fail, false),
        (OpenIn { id: some(b"default"), output_flags: s, matched: true, fmt: f, default_id: some(b"s"), fallback_fmt: f }, Route::Ignore, false),
        (OpenIn { id: some(b"sink.a"), output_flags: s, matched: true, fmt: f, default_id: some(b"s"), fallback_fmt: f }, Route::Ignore, false),
        (OpenIn { id: some(b"sink.a"), output_flags: 0, matched: true, fmt: f, default_id: some(b"s"), fallback_fmt: f }, Route::Named, true),
        (OpenIn { id: some(b"sink.a"), output_flags: s, matched: false, fmt: f, default_id: some(b"s"), fallback_fmt: f }, Route::Named, true),
        (OpenIn { id: some(b"default"), output_flags: 0, matched: false, fmt: f, default_id: some(b"s"), fallback_fmt: f }, Route::Default, false),
        (OpenIn { id: some(b"default"), output_flags: 0, matched: false, fmt: i, default_id: some(b"s"), fallback_fmt: f }, Route::Fail, false),
        (OpenIn { id: some(b"sink.a"), output_flags: 0, matched: false, fmt: i, default_id: some(b"s"), fallback_fmt: f }, Route::Fallback, false),
        (OpenIn { id: some(b"sink.a"), output_flags: 0, matched: false, fmt: i, default_id: some(b"s"), fallback_fmt: i }, Route::Fail, false),
        (OpenIn { id: some(b"sink.a"), output_flags: 0, matched: false, fmt: i, default_id: some(b""), fallback_fmt: f }, Route::Fail, false),
        (OpenIn { id: some(b"sink.a"), output_flags: 0, matched: false, fmt: i, default_id: None, fallback_fmt: f }, Route::Fail, false),
        (OpenIn { id: some(b""), output_flags: 0, matched: false, fmt: i, default_id: some(b"s"), fallback_fmt: f }, Route::Fallback, false),
        (OpenIn { id: some(b""), output_flags: 0, matched: false, fmt: f, default_id: some(b"s"), fallback_fmt: f }, Route::Named, true),
    ];
    for (n, (input, route, dont_move)) in cases.iter().enumerate() {
        let (r, flags) = stand_in_open(input);
        assert_eq!(r, *route, "case {n}");
        if matches!(r, Route::Default | Route::Named | Route::Fallback) {
            assert_eq!(
                flags & STREAM_DONT_MOVE != 0,
                *dont_move,
                "case {n}: flags {flags:#x}"
            );
        }
    }
}

#[test]
fn steps_only_new_and_remove_are_queued() {
    for f in [
        SUBSCRIPTION_EVENT_SINK,
        SUBSCRIPTION_EVENT_SOURCE,
        SUBSCRIPTION_EVENT_SINK_INPUT,
        SUBSCRIPTION_EVENT_SERVER,
        SUBSCRIPTION_EVENT_CARD,
        SUBSCRIPTION_EVENT_FACILITY_MASK,
    ] {
        assert!(
            event_queued(f | SUBSCRIPTION_EVENT_NEW),
            "facility {f}: new"
        );
        assert!(
            event_queued(f | SUBSCRIPTION_EVENT_REMOVE),
            "facility {f}: remove"
        );
        assert!(
            !event_queued(f | SUBSCRIPTION_EVENT_CHANGE),
            "facility {f}: change"
        );
        assert!(
            !event_queued(f | SUBSCRIPTION_EVENT_TYPE_MASK),
            "facility {f}: type 0x30"
        );
    }
    // Bits above the type mask are ignored.
    assert!(event_queued(0x40 | SUBSCRIPTION_EVENT_NEW));
    assert!(event_queued(0xffff_ff00 | SUBSCRIPTION_EVENT_REMOVE));
    assert!(!event_queued(0xffff_ff00 | SUBSCRIPTION_EVENT_CHANGE));
}

#[test]
fn steps_context_states() {
    assert!(!context_good(CONTEXT_UNCONNECTED));
    assert!(context_good(CONTEXT_CONNECTING));
    assert!(context_good(CONTEXT_AUTHORIZING));
    assert!(context_good(CONTEXT_SETTING_NAME));
    assert!(context_good(CONTEXT_READY));
    assert!(!context_good(CONTEXT_FAILED));
    assert!(!context_good(CONTEXT_TERMINATED));
    assert!(!context_good(u32::MAX)); // (pa_context_state_t)-1
    assert!(!context_good(7));
}

#[test]
fn steps_reset_routes() {
    for s in [u32::MAX, 0, 1, 2, 3, 4, 5, 6, 7] {
        let good = (CONTEXT_CONNECTING..=CONTEXT_READY).contains(&s);
        assert_eq!(
            stand_in_reset(true, true, true, s),
            ResetRoute::Replace,
            "state {s}: init ok"
        );
        assert_eq!(
            stand_in_reset(true, false, false, s),
            ResetRoute::Replace,
            "state {s}: init ok, no stream"
        );
        assert_eq!(
            stand_in_reset(false, false, true, s),
            ResetRoute::Keep,
            "state {s}: no stream"
        );
        assert_eq!(
            stand_in_reset(false, true, false, s),
            ResetRoute::Ignore,
            "state {s}: no context"
        );
        let want = if good {
            ResetRoute::Keep
        } else {
            ResetRoute::Ignore
        };
        assert_eq!(stand_in_reset(false, true, true, s), want, "state {s}");
    }
}

#[test]
fn steps_last_monitor() {
    assert!(last(0));
    assert!(!last(1));
    assert!(!last(2));
    assert!(!last(usize::MAX));
}

// ---- edge_*: against the models ----

const EDGE_USIZE: [usize; 16] = [
    0,
    1,
    2,
    4799,
    4800,
    4801,
    9599,
    9600,
    9601,
    0x7fff_ffff,
    0x8000_0000,
    0xffff_fffe,
    U32_MAX,
    usize::MAX / 2,
    usize::MAX - 1,
    usize::MAX,
];

const EDGE_U32: [u32; 12] = [
    0,
    1,
    2,
    4799,
    4800,
    4801,
    9600,
    0x7fff_ffff,
    0x8000_0000,
    0x8000_0001,
    0xffff_fffe,
    0xffff_ffff,
];

fn edge_sizes() -> Vec<usize> {
    let mut v: Vec<usize> = EDGE_USIZE.to_vec();
    if usize::MAX > U32_MAX {
        v.extend([
            U32_MAX + 1,
            U32_MAX + 2,
            2 * U32_MAX,
            2 * U32_MAX + 1,
            2 * U32_MAX + 2,
        ]);
    }
    for t in EDGE_U32 {
        let t = t as usize;
        for d in [
            t.wrapping_mul(2).wrapping_sub(1),
            t.wrapping_mul(2),
            t.wrapping_mul(2).wrapping_add(1),
        ] {
            v.push(d);
        }
    }
    v
}

#[test]
fn edge_write_rules_match_the_c() {
    let sizes = edge_sizes();
    for &q in &sizes {
        assert_eq!(grown_tlength(q), c_grown_tlength(q), "grown_tlength({q})");
        assert_eq!(write_more(q), q > 0, "write_more({q})");
        assert_eq!(last(q), q == 0, "last({q})");
        for &t in &EDGE_U32 {
            assert_eq!(grow(q, t), c_grow(q, t), "grow({q}, {t})");
            assert_eq!(uncork(q, t), c_uncork(q, t), "uncork({q}, {t})");
        }
        for &a in &sizes {
            assert_eq!(
                write_short(a, q),
                c_write_short(a, q),
                "write_short({a}, {q})"
            );
        }
    }
}

#[test]
fn edge_playback_bytes_match_the_c() {
    let bpf = [
        0u64,
        1,
        2,
        4,
        8,
        16,
        32,
        0x7fff_ffff,
        0xffff_ffff,
        0x1_0000_0000,
        u64::MAX,
    ];
    for b in bpf {
        for f in EDGE_U32 {
            for wide in [false, true] {
                assert_eq!(
                    playback_bytes(b, f, wide),
                    c_playback_bytes(b, f, wide),
                    "playback_bytes({b}, {f}, {wide})"
                );
            }
        }
    }
}

#[test]
fn edge_playback_idle() {
    for r in [i64::MIN, -2, -1, 0, 1, 2, i64::from(i32::MAX), i64::MAX] {
        assert_eq!(playback_idle(r), r == 0, "playback_idle({r})");
    }
}

#[test]
fn edge_volume_matches_the_c_near_one() {
    // Every float within 2^12 ulps either side of the boundaries, and of 1.
    for centre in [0x3f80_0346u32, 0x3f7f_f973, 0x3f80_0000] {
        for bits in centre - 4096..=centre + 4096 {
            let v = f32::from_bits(bits);
            assert_eq!(needs_volume(v), c_needs_volume(v), "needs_volume({v:e})");
        }
    }
    for v in [
        0.0f32,
        -0.0,
        f32::MIN_POSITIVE,
        f32::MAX,
        f32::MIN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        -f32::NAN,
        EPSILON,
    ] {
        assert_eq!(needs_volume(v), c_needs_volume(v), "needs_volume({v:e})");
    }
}

#[test]
fn edge_sink_result_and_formats() {
    for eol in [i32::MIN, -2, -1, 0, 1, 2, i32::MAX] {
        assert_eq!(sink_result(eol), c_sink_result(eol), "sink_result({eol})");
    }
    for f in [i32::MIN, -2, -1, 0, 1, 5, 12, 13, 14, i32::MAX] {
        assert_eq!(format_invalid(f), f == -1, "format_invalid({f})");
        for id in [&b"default"[..], b"sink.a", b""] {
            assert_eq!(
                fallback(f, id),
                f == -1 && id != b"default",
                "fallback({f}, {id:?})"
            );
        }
    }
}

#[test]
fn edge_events_every_low_word() {
    for e in 0..=0xffffu32 {
        for hi in [0u32, 0x0001_0000, 0x8000_0000, 0xffff_0000] {
            let t = e | hi;
            assert_eq!(event_queued(t), c_event_queued(t), "event_queued({t:#x})");
        }
    }
    for t in [u32::MAX, u32::MAX - 0x30, 0x40, 0x7f] {
        assert_eq!(event_queued(t), c_event_queued(t), "event_queued({t:#x})");
    }
}

#[test]
fn edge_flags_every_low_word() {
    for f in 0..=0xffffu32 {
        for hi in [0u32, 0x0001_0000, 0x8000_0000, 0xffff_0000] {
            let flags = f | hi;
            assert_eq!(
                self_monitor_check(flags),
                flags & (1 << 9) != 0,
                "flags {flags:#x}"
            );
        }
    }
}

#[test]
fn edge_context_every_small_state() {
    for s in (0..=64u32).chain([u32::MAX, u32::MAX - 1, 0x8000_0000]) {
        assert_eq!(context_good(s), c_context_good(s), "context_good({s})");
    }
}

#[test]
fn edge_ids() {
    for id in [
        &b"default"[..],
        b"defaul",
        b"defaultt",
        b"DEFAULT",
        b"default\x01",
        b" default",
        b"",
    ] {
        assert_eq!(is_default(id), id == b"default", "is_default({id:?})");
        assert!(has_device(Some(id)));
        assert_eq!(sink_named(Some(id)), !id.is_empty());
        assert_eq!(open_flags(false, Some(id)), stream_flags(Some(id)));
        assert_eq!(open_flags(true, Some(id)), stream_flags(Some(b"default")));
    }
    assert_eq!(tlength_usec(), TLENGTH_USEC);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn prop_decisions_match_the_c(
        q in any::<usize>(),
        a in any::<usize>(),
        t in any::<u32>(),
        bpf in any::<u64>(),
        frames in any::<u32>(),
        wide in any::<bool>(),
        bits in any::<u32>(),
        eol in any::<i32>(),
        refs in any::<i64>(),
        event in any::<u32>(),
        state in any::<u32>(),
        fmt in any::<i32>(),
        id in proptest::collection::vec(any::<u8>(), 0..10),
    ) {
        prop_assert_eq!(grow(q, t), c_grow(q, t));
        prop_assert_eq!(grown_tlength(q), c_grown_tlength(q));
        prop_assert_eq!(uncork(q, t), c_uncork(q, t));
        prop_assert_eq!(write_more(q), q > 0);
        prop_assert_eq!(write_short(a, q), c_write_short(a, q));
        prop_assert_eq!(playback_idle(refs), refs == 0);
        prop_assert_eq!(playback_bytes(bpf, frames, wide), c_playback_bytes(bpf, frames, wide));
        let v = f32::from_bits(bits);
        prop_assert_eq!(needs_volume(v), c_needs_volume(v));
        prop_assert_eq!(sink_result(eol), c_sink_result(eol));
        prop_assert_eq!(self_monitor_check(bits), bits & (1 << 9) != 0);
        prop_assert_eq!(event_queued(event), c_event_queued(event));
        prop_assert_eq!(context_good(state), c_context_good(state));
        prop_assert_eq!(format_invalid(fmt), fmt == -1);
        let id = id.as_slice();
        prop_assert_eq!(is_default(id), id == b"default");
        prop_assert_eq!(fallback(fmt, id), fmt == -1 && id != b"default");
        prop_assert_eq!(sink_named(Some(id)), !id.is_empty());
        prop_assert_eq!(last(q), q == 0);
    }

    #[test]
    fn prop_write_loop_matches_the_c(
        queued in prop_oneof![0usize..40_000, any::<usize>()],
        tlength in prop_oneof![0u32..20_000, any::<u32>()],
        corked in any::<bool>(),
        asks in proptest::collection::vec(prop_oneof![0usize..20_000, any::<usize>()], 0..6),
    ) {
        // The asks the server makes are at most what is queued, or more
        // (the cancel); a pop of more than is queued never happens.
        let mut rust = StandInWrite::new(queued, tlength, corked);
        let mut c = rust.clone();
        rust.run(&asks);
        c.run_c(&asks);
        prop_assert_eq!(rust, c);
    }
}
