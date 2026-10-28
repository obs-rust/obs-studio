//! Tier 1: `obs_source::monitoring`, the monitoring, monitor-only mute,
//! balance, mixer, flag, push-to and audio active rules of
//! `obs_source_set_flags()`, `obs_source_set_audio_mixers()` /
//! `_get_audio_mixers()`, the push-to enables,
//! `obs_source_set_monitoring_enabled()`, `_set_monitoring_type()` /
//! `_get_monitoring_type()`, `clamp_balance()` and
//! `obs_source_set_audio_active()`.
//!
//! The `steps_*` cases port `test/cmocka/test_source_monitoring_steps.c`
//! 1:1: its 16 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones and the
//! copies of the callers on a stand-in source included. The C calls the
//! hidden `obs_source_monitoring_*()` functions; here the safe core is
//! called directly.
//!
//! The `edge_*` cases are new: every decision over every combination of its
//! flags against models of origin/master's expressions, the route over
//! -100,000..100,000 and the ends of `i32`, the balance over every float
//! from -2.0 to 2.0 in steps and every special value, the mixers at every
//! power of two with its neighbours, and proptests: full-range values, and
//! operation sequences through the stand-in against a copy of the callers
//! with origin/master's expressions inline.

use obs_source::monitoring::{
    AUDIO, Active, Monitor, Route, TYPE_MONITOR_AND_OUTPUT, TYPE_MONITOR_ONLY, TYPE_NONE,
    audio_active, balance, changes, has_audio, mixers_store, monitor, off_reads_mute, off_unmutes,
    on_type, only_mutes, push_to_logs, route, stored_type,
};
use proptest::prelude::*;

const SOURCE_VIDEO: u32 = 1; // OBS_SOURCE_VIDEO
const SOURCE_ASYNC: u32 = 1 << 2; // OBS_SOURCE_ASYNC
const FLAG_FORCE_MONO: u32 = 1 << 1; // OBS_SOURCE_FLAG_FORCE_MONO

const U32S: [u32; 12] = [
    0,
    1,
    2,
    3,
    0x7f,
    0x80,
    0xff,
    0x100,
    0x7fff_ffff,
    0x8000_0000,
    0xffff_fffe,
    0xffff_ffff,
];

fn bits(f: f32) -> u32 {
    f.to_bits()
}

/// origin/master's `clamp_balance()`, its compares as they are.
#[allow(clippy::manual_clamp)]
fn master_balance(v: f32) -> f32 {
    if v.is_nan() {
        return 0.5;
    }
    if v < 0.0 {
        0.0
    } else if v > 1.0 {
        1.0
    } else {
        v
    }
}

/// origin/master's `(uint32_t)` of the handler's `int64_t`.
fn master_mixers(v: i64) -> u32 {
    u32::from_le_bytes(v.to_le_bytes()[..4].try_into().unwrap())
}

/// origin/master's `switch (type)` in `obs_source_set_monitoring_type()`.
fn master_route(t: i32) -> Route {
    match t {
        0 => Route::Off,
        1 => Route::MonitorOnly,
        2 => Route::MonitorAndOutput,
        _ => Route::On,
    }
}

#[test]
fn steps_constants() {
    // the header's mirror of libobs
    assert_eq!(AUDIO, 1 << 1);
    assert_eq!(TYPE_NONE, 0);
    assert_eq!(TYPE_MONITOR_ONLY, 1);
    assert_eq!(TYPE_MONITOR_AND_OUTPUT, 2);
    assert_eq!(Monitor::Keep as i32, 0);
    assert_eq!(Monitor::Create as i32, 1);
    assert_eq!(Monitor::Destroy as i32, 2);
    assert_eq!(Route::Off as i32, 0);
    assert_eq!(Route::MonitorOnly as i32, 1);
    assert_eq!(Route::MonitorAndOutput as i32, 2);
    assert_eq!(Route::On as i32, 3);
    assert_eq!(Active::None as i32, 0);
    assert_eq!(Active::Activate as i32, 1);
    assert_eq!(Active::Deactivate as i32, 2);
}

#[test]
fn steps_changes() {
    for (i, &a) in U32S.iter().enumerate() {
        for (j, &b) in U32S.iter().enumerate() {
            assert_eq!(changes(a, b), i != j);
        }
    }

    // the bools of the push-to enables and monitoring_enabled
    assert!(!changes(0, 0));
    assert!(changes(0, 1));
    assert!(changes(1, 0));
    assert!(!changes(1, 1));
}

#[test]
fn steps_has_audio() {
    assert!(!has_audio(0));
    assert!(has_audio(AUDIO));
    assert!(has_audio(0xffff_ffff));
    assert!(!has_audio(!AUDIO));
    for bit in 0..32 {
        assert_eq!(has_audio(1 << bit), bit == 1);
    }
    assert!(!has_audio(SOURCE_VIDEO | SOURCE_ASYNC));
    assert!(has_audio(SOURCE_VIDEO | AUDIO));
}

#[test]
fn steps_mixers_store() {
    assert_eq!(mixers_store(0), 0);
    assert_eq!(mixers_store(1), 1);
    assert_eq!(mixers_store(0x3f), 0x3f);
    assert_eq!(mixers_store(0xffff_ffff), 0xffff_ffff);

    // characterized, not endorsed: the low 32 bits
    assert_eq!(mixers_store(0x1_0000_0000), 0);
    assert_eq!(mixers_store(0x1_0000_0005), 5);
    assert_eq!(mixers_store(-1), 0xffff_ffff);
    assert_eq!(mixers_store(-2), 0xffff_fffe);
    assert_eq!(mixers_store(i64::MIN), 0);
    assert_eq!(mixers_store(i64::MAX), 0xffff_ffff);
    assert_eq!(mixers_store(i64::from(i32::MIN)), 0x8000_0000);
}

#[test]
fn steps_push_to_logs() {
    for &flags in &U32S {
        let audio = flags & AUDIO != 0;

        assert!(!push_to_logs(flags, false));
        assert_eq!(push_to_logs(flags, true), audio);
    }
    assert!(push_to_logs(AUDIO, true));
    assert!(!push_to_logs(SOURCE_VIDEO, true));
}

#[test]
fn steps_monitoring_types() {
    // on and muted is Monitor Only (#2447)
    assert_eq!(on_type(true), TYPE_MONITOR_ONLY);
    assert_eq!(on_type(false), TYPE_MONITOR_AND_OUTPUT);

    // the stored type: on is Monitor and Output
    assert_eq!(stored_type(true), TYPE_MONITOR_AND_OUTPUT);
    assert_eq!(stored_type(false), TYPE_NONE);

    // the monitor
    assert_eq!(monitor(false, false), Monitor::Keep);
    assert_eq!(monitor(false, true), Monitor::Create);
    assert_eq!(monitor(true, false), Monitor::Destroy);
    assert_eq!(monitor(true, true), Monitor::Keep);
}

#[test]
fn steps_route() {
    assert_eq!(route(TYPE_NONE), Route::Off);
    assert_eq!(route(TYPE_MONITOR_ONLY), Route::MonitorOnly);
    assert_eq!(route(TYPE_MONITOR_AND_OUTPUT), Route::MonitorAndOutput);
    assert_eq!(route(3), Route::On);
    assert_eq!(route(4), Route::On);
    assert_eq!(route(-1), Route::On);
    assert_eq!(route(i32::MIN), Route::On);
    assert_eq!(route(i32::MAX), Route::On);
    for t in -1000..=1000 {
        let want = match t {
            0 => Route::Off,
            1 => Route::MonitorOnly,
            2 => Route::MonitorAndOutput,
            _ => Route::On,
        };
        assert_eq!(route(t), want);
    }

    // the mute steps
    assert!(!off_reads_mute(false));
    assert!(off_reads_mute(true));
    assert!(!off_unmutes(false));
    assert!(off_unmutes(true));
    assert!(only_mutes(false));
    assert!(!only_mutes(true));
}

#[test]
fn steps_balance() {
    // NaN is centred: quiet, negative and signalling
    assert_eq!(bits(balance(f32::NAN)), bits(0.5));
    assert_eq!(bits(balance(-f32::NAN)), bits(0.5));
    assert_eq!(bits(balance(f32::from_bits(0x7f80_0001))), bits(0.5));
    assert_eq!(bits(balance(f32::from_bits(0xffbf_ffff))), bits(0.5));

    // in range as it is
    assert_eq!(bits(balance(0.0)), bits(0.0));
    assert_eq!(bits(balance(0.25)), bits(0.25));
    assert_eq!(bits(balance(0.5)), bits(0.5));
    assert_eq!(bits(balance(1.0)), bits(1.0));
    assert_eq!(bits(balance(f32::from_bits(1))), 1);
    let below_one = f32::from_bits(1.0f32.to_bits() - 1);
    assert_eq!(bits(balance(below_one)), bits(below_one));

    // characterized, not endorsed: -0.0 is not below 0.0f, so it stays
    assert_eq!(bits(balance(-0.0)), 0x8000_0000);

    // out of range clamped
    assert_eq!(
        bits(balance(f32::from_bits(1.0f32.to_bits() + 1))),
        bits(1.0)
    );
    assert_eq!(bits(balance(2.0)), bits(1.0));
    assert_eq!(bits(balance(f32::MAX)), bits(1.0));
    assert_eq!(bits(balance(f32::INFINITY)), bits(1.0));
    assert_eq!(bits(balance(f32::from_bits(0x8000_0001))), bits(0.0));
    assert_eq!(bits(balance(-1.0)), bits(0.0));
    assert_eq!(bits(balance(-f32::MAX)), bits(0.0));
    assert_eq!(bits(balance(f32::NEG_INFINITY)), bits(0.0));
}

#[test]
fn steps_audio_active() {
    assert_eq!(audio_active(false, false), Active::None);
    assert_eq!(audio_active(false, true), Active::Activate);
    assert_eq!(audio_active(true, false), Active::Deactivate);
    assert_eq!(audio_active(true, true), Active::None);
}

// A stand-in source and copies of the callers, as obs-source.c has them, on
// it: every signal, log, mute call and monitor is recorded. `Rules` picks
// the decisions: the core, or origin/master's expressions inline.

trait Rules {
    fn changes(cur: u32, value: u32) -> bool;
    fn has_audio(flags: u32) -> bool;
    fn mixers_store(v: i64) -> u32;
    fn push_to_logs(flags: u32, changed: bool) -> bool;
    fn on_type(muted: bool) -> i32;
    fn stored_type(enabled: bool) -> i32;
    fn monitor(was_on: bool, enabled: bool) -> Monitor;
    fn route(t: i32) -> Route;
    fn off_reads_mute(m: bool) -> bool;
    fn off_unmutes(muted: bool) -> bool;
    fn only_mutes(muted: bool) -> bool;
    fn balance(v: f32) -> f32;
    fn audio_active(prev: bool, active: bool) -> Active;
}

struct Core;
impl Rules for Core {
    fn changes(cur: u32, value: u32) -> bool {
        changes(cur, value)
    }
    fn has_audio(flags: u32) -> bool {
        has_audio(flags)
    }
    fn mixers_store(v: i64) -> u32 {
        mixers_store(v)
    }
    fn push_to_logs(flags: u32, changed: bool) -> bool {
        push_to_logs(flags, changed)
    }
    fn on_type(muted: bool) -> i32 {
        on_type(muted)
    }
    fn stored_type(enabled: bool) -> i32 {
        stored_type(enabled)
    }
    fn monitor(was_on: bool, enabled: bool) -> Monitor {
        monitor(was_on, enabled)
    }
    fn route(t: i32) -> Route {
        route(t)
    }
    fn off_reads_mute(m: bool) -> bool {
        off_reads_mute(m)
    }
    fn off_unmutes(muted: bool) -> bool {
        off_unmutes(muted)
    }
    fn only_mutes(muted: bool) -> bool {
        only_mutes(muted)
    }
    fn balance(v: f32) -> f32 {
        balance(v)
    }
    fn audio_active(prev: bool, active: bool) -> Active {
        audio_active(prev, active)
    }
}

/// origin/master's expressions, inline in its callers.
struct Master;
impl Rules for Master {
    fn changes(cur: u32, value: u32) -> bool {
        cur != value
    }
    fn has_audio(flags: u32) -> bool {
        flags & AUDIO != 0
    }
    fn mixers_store(v: i64) -> u32 {
        master_mixers(v)
    }
    fn push_to_logs(flags: u32, changed: bool) -> bool {
        flags & AUDIO != 0 && changed
    }
    fn on_type(muted: bool) -> i32 {
        if muted { 1 } else { 2 }
    }
    fn stored_type(enabled: bool) -> i32 {
        if enabled { 2 } else { 0 }
    }
    fn monitor(was_on: bool, enabled: bool) -> Monitor {
        if was_on != enabled {
            if !was_on {
                Monitor::Create
            } else {
                Monitor::Destroy
            }
        } else {
            Monitor::Keep
        }
    }
    fn route(t: i32) -> Route {
        master_route(t)
    }
    fn off_reads_mute(m: bool) -> bool {
        m
    }
    fn off_unmutes(muted: bool) -> bool {
        muted
    }
    fn only_mutes(muted: bool) -> bool {
        !muted
    }
    fn balance(v: f32) -> f32 {
        master_balance(v)
    }
    fn audio_active(prev: bool, active: bool) -> Active {
        if prev == active {
            Active::None
        } else if active {
            Active::Activate
        } else {
            Active::Deactivate
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct StandIn {
    valid: bool,
    flags: u32,
    audio_mixers: u32,
    owns_info_id: bool,
    output_flags: u32,
    push_to_mute_enabled: bool,
    push_to_talk_enabled: bool,
    monitoring_enabled: bool,
    monitoring_type: i32,
    user_muted: bool,
    monitor: bool,
    monitor_only_muted: bool,
    balance_bits: u32,
    audio_active: bool,

    // what a handler leaves
    handler_sets: bool,
    handler_mixers: i64,
    handler_balance_bits: u64,

    // recorded
    trace: String,
    signals: i32,
    logs: i32,
    compat_type: i64,
    sent_mixers: i64,
    sent_balance_bits: u64,
}

impl StandIn {
    fn source() -> Self {
        Self {
            valid: true,
            flags: 0,
            audio_mixers: 0,
            owns_info_id: false,
            output_flags: AUDIO,
            push_to_mute_enabled: false,
            push_to_talk_enabled: false,
            monitoring_enabled: false,
            monitoring_type: 0,
            user_muted: false,
            monitor: false,
            monitor_only_muted: false,
            balance_bits: 0.5f32.to_bits(),
            audio_active: false,
            handler_sets: false,
            handler_mixers: 0,
            handler_balance_bits: 0,
            trace: String::new(),
            signals: 0,
            logs: 0,
            compat_type: -1,
            sent_mixers: 0,
            sent_balance_bits: 0,
        }
    }

    fn balance(&self) -> f32 {
        f32::from_bits(self.balance_bits)
    }

    fn record(&mut self, what: &str) {
        self.trace.push_str(what);
        self.trace.push(';');
    }

    fn muted(&self) -> bool {
        if self.valid { self.user_muted } else { false }
    }

    fn set_muted(&mut self, muted: bool) {
        if !self.valid {
            return;
        }
        self.record(if muted { "mute" } else { "unmute" });
        self.user_muted = muted;
        // a mute set by anything else is not Monitor Only's to undo (#2447)
        self.monitor_only_muted = false;
        self.signals += 1;
    }

    fn set_flags<R: Rules>(&mut self, flags: u32) {
        if !self.valid {
            return;
        }
        if R::changes(self.flags, flags) {
            self.flags = flags;
            self.record("update_flags");
            self.signals += 1;
        }
    }

    fn set_audio_mixers<R: Rules>(&mut self, mixers: u32) {
        if !self.valid {
            return;
        }
        if !self.owns_info_id && !R::has_audio(self.output_flags) {
            return;
        }
        if !R::changes(self.audio_mixers, mixers) {
            return;
        }
        let mut value = i64::from(mixers);
        self.sent_mixers = value;
        self.record("audio_mixers");
        self.signals += 1;
        if self.handler_sets {
            value = self.handler_mixers;
        }
        self.audio_mixers = R::mixers_store(value);
    }

    fn get_audio_mixers<R: Rules>(&self) -> u32 {
        if !self.valid {
            return 0;
        }
        if !self.owns_info_id && !R::has_audio(self.output_flags) {
            return 0;
        }
        self.audio_mixers
    }

    fn enable_push_to<R: Rules>(&mut self, talk: bool, enabled: bool) {
        if !self.valid {
            return;
        }
        let cur = if talk {
            self.push_to_talk_enabled
        } else {
            self.push_to_mute_enabled
        };
        let changed = R::changes(u32::from(cur), u32::from(enabled));
        if R::push_to_logs(self.output_flags, changed) {
            self.record("log");
            self.logs += 1;
        }
        if talk {
            self.push_to_talk_enabled = enabled;
        } else {
            self.push_to_mute_enabled = enabled;
        }
        if changed {
            self.record(if talk {
                "push_to_talk_changed"
            } else {
                "push_to_mute_changed"
            });
            self.signals += 1;
        }
    }

    fn set_monitoring_enabled<R: Rules>(&mut self, enabled: bool) {
        if !self.valid {
            return;
        }
        if !R::changes(u32::from(self.monitoring_enabled), u32::from(enabled)) {
            return;
        }
        let mut compat_type = TYPE_NONE;
        if enabled {
            compat_type = R::on_type(self.user_muted);
        }
        self.record(if enabled { "monitor_on" } else { "monitor_off" });
        self.signals += 1;
        self.compat_type = i64::from(compat_type);
        self.signals += 1;

        let was_on = self.monitoring_enabled;
        self.monitoring_enabled = enabled;
        self.monitoring_type = R::stored_type(enabled);

        match R::monitor(was_on, enabled) {
            Monitor::Create => {
                self.record("create");
                self.monitor = true;
            }
            Monitor::Destroy => {
                self.record("destroy");
                self.monitor = false;
            }
            Monitor::Keep => {}
        }
    }

    fn set_monitoring_type<R: Rules>(&mut self, t: i32) {
        if !self.valid {
            return;
        }
        self.record("deprecated");
        self.logs += 1;

        let mut muted_for_monitor_only = self.monitor_only_muted;

        match R::route(t) {
            Route::Off => {
                self.set_monitoring_enabled::<R>(false);
                if R::off_reads_mute(muted_for_monitor_only) && R::off_unmutes(self.muted()) {
                    self.set_muted(false);
                }
                self.monitor_only_muted = false;
            }
            Route::MonitorOnly => {
                if R::only_mutes(self.muted()) {
                    self.set_muted(true);
                    muted_for_monitor_only = true;
                }
                self.set_monitoring_enabled::<R>(true);
                self.monitor_only_muted = muted_for_monitor_only;
            }
            Route::MonitorAndOutput => {
                self.set_muted(false);
                self.set_monitoring_enabled::<R>(true);
            }
            Route::On => self.set_monitoring_enabled::<R>(true),
        }
    }

    fn get_monitoring_type<R: Rules>(&self) -> i32 {
        if !self.valid || !self.monitoring_enabled {
            return TYPE_NONE;
        }
        R::on_type(self.user_muted)
    }

    fn set_balance_value<R: Rules>(&mut self, v: f32) {
        if self.valid {
            let mut value = f64::from(R::balance(v));
            self.sent_balance_bits = value.to_bits();
            self.record("audio_balance");
            self.signals += 1;
            if self.handler_sets {
                value = f64::from_bits(self.handler_balance_bits);
            }
            #[allow(clippy::cast_possible_truncation)]
            let narrowed = value as f32;
            self.balance_bits = R::balance(narrowed).to_bits();
        }
    }

    fn set_audio_active<R: Rules>(&mut self, active: bool) {
        if !self.valid {
            return;
        }
        let prev = self.audio_active;
        self.audio_active = active;
        match R::audio_active(prev, active) {
            Active::Activate => {
                self.record("audio_activate");
                self.signals += 1;
            }
            Active::Deactivate => {
                self.record("audio_deactivate");
                self.signals += 1;
            }
            Active::None => {}
        }
    }

    fn sent_balance(&self) -> f32 {
        #[allow(clippy::cast_possible_truncation)]
        let narrowed = f64::from_bits(self.sent_balance_bits) as f32;
        narrowed
    }
}

/// `obs_source_set_monitoring_type()` from every start state: monitoring on
/// or off, muted or not, Monitor Only's mark set or not.
#[test]
#[expect(
    clippy::cognitive_complexity,
    reason = "scenario table: one block per ported step"
)]
#[expect(
    clippy::too_many_lines,
    reason = "scenario table: one block per ported step"
)]
fn steps_monitoring_type_steps() {
    let types = [TYPE_NONE, TYPE_MONITOR_ONLY, TYPE_MONITOR_AND_OUTPUT, 3, -1];

    for &t in &types {
        for start in 0..8 {
            let mut s = StandIn::source();
            let on = start & 1 != 0;
            let muted = start & 2 != 0;
            let mark = start & 4 != 0;
            let mut want = String::from("deprecated;");

            s.monitoring_enabled = on;
            s.monitor = on;
            s.monitoring_type = if on {
                TYPE_MONITOR_AND_OUTPUT
            } else {
                TYPE_NONE
            };
            s.user_muted = muted;
            s.monitor_only_muted = mark;

            s.set_monitoring_type::<Core>(t);

            if t == TYPE_NONE {
                // off, then the mute Monitor Only set goes, any other
                // stays; the mark is cleared
                if on {
                    want.push_str("monitor_off;destroy;");
                }
                if mark && muted {
                    want.push_str("unmute;");
                }
                assert_eq!(s.trace, want);
                assert!(!s.monitoring_enabled);
                assert!(!s.monitor);
                assert_eq!(s.monitoring_type, TYPE_NONE);
                assert_eq!(s.user_muted, muted && !mark);
                assert!(!s.monitor_only_muted);
                if on {
                    assert_eq!(s.compat_type, i64::from(TYPE_NONE));
                }
            } else if t == TYPE_MONITOR_ONLY {
                // the mute first, then on; the mark says whether this
                // call muted it (an earlier mark stays)
                if !muted {
                    want.push_str("mute;");
                }
                if !on {
                    want.push_str("monitor_on;create;");
                }
                assert_eq!(s.trace, want);
                assert!(s.monitoring_enabled);
                assert!(s.monitor);
                assert_eq!(s.monitoring_type, TYPE_MONITOR_AND_OUTPUT);
                assert!(s.user_muted);
                assert_eq!(s.monitor_only_muted, !muted || mark);
                if !on {
                    assert_eq!(s.compat_type, i64::from(TYPE_MONITOR_ONLY));
                }
            } else if t == TYPE_MONITOR_AND_OUTPUT {
                // characterized, not endorsed: the unmute takes a mute
                // the user set too
                want.push_str("unmute;");
                if !on {
                    want.push_str("monitor_on;create;");
                }
                assert_eq!(s.trace, want);
                assert!(s.monitoring_enabled);
                assert!(s.monitor);
                assert_eq!(s.monitoring_type, TYPE_MONITOR_AND_OUTPUT);
                assert!(!s.user_muted);
                assert!(!s.monitor_only_muted);
                if !on {
                    assert_eq!(s.compat_type, i64::from(TYPE_MONITOR_AND_OUTPUT));
                }
            } else {
                // not a type: on, the mute and the mark as they were
                if !on {
                    want.push_str("monitor_on;create;");
                }
                assert_eq!(s.trace, want);
                assert!(s.monitoring_enabled);
                assert!(s.monitor);
                assert_eq!(s.user_muted, muted);
                assert_eq!(s.monitor_only_muted, mark);
                if !on {
                    let compat = if muted {
                        TYPE_MONITOR_ONLY
                    } else {
                        TYPE_MONITOR_AND_OUTPUT
                    };
                    assert_eq!(s.compat_type, i64::from(compat));
                }
            }

            // the getter: off is none, on and muted Monitor Only
            let getter = if !s.monitoring_enabled {
                TYPE_NONE
            } else if s.user_muted {
                TYPE_MONITOR_ONLY
            } else {
                TYPE_MONITOR_AND_OUTPUT
            };
            assert_eq!(s.get_monitoring_type::<Core>(), getter);
        }
    }

    // Monitor Only, then none: the mute it set goes
    let mut s = StandIn::source();
    s.set_monitoring_type::<Core>(TYPE_MONITOR_ONLY);
    assert!(s.user_muted);
    assert!(s.monitor_only_muted);
    s.set_monitoring_type::<Core>(TYPE_NONE);
    assert!(!s.user_muted);
    assert!(!s.monitoring_enabled);

    // Monitor Only, the user mutes again, then none: the mute stays
    let mut s = StandIn::source();
    s.set_monitoring_type::<Core>(TYPE_MONITOR_ONLY);
    s.set_muted(true);
    s.set_monitoring_type::<Core>(TYPE_NONE);
    assert!(s.user_muted);

    // an invalid source: nothing
    let mut s = StandIn::source();
    s.valid = false;
    s.set_monitoring_type::<Core>(TYPE_MONITOR_ONLY);
    assert_eq!(s.trace, "");
    assert_eq!(s.get_monitoring_type::<Core>(), TYPE_NONE);
}

/// `obs_source_set_monitoring_enabled()`: the same value does nothing, and
/// the compatibility signal's type.
#[test]
fn steps_monitoring_enabled_steps() {
    for start in 0..4 {
        let on = start & 1 != 0;
        let muted = start & 2 != 0;

        for enabled in [false, true] {
            let mut s = StandIn::source();
            s.monitoring_enabled = on;
            s.monitor = on;
            s.user_muted = muted;
            s.set_monitoring_enabled::<Core>(enabled);

            if on == enabled {
                assert_eq!(s.trace, "");
                assert_eq!(s.signals, 0);
                assert_eq!(s.compat_type, -1);
                continue;
            }
            assert_eq!(s.signals, 2);
            let compat = if !enabled {
                TYPE_NONE
            } else if muted {
                TYPE_MONITOR_ONLY
            } else {
                TYPE_MONITOR_AND_OUTPUT
            };
            assert_eq!(s.compat_type, i64::from(compat));
            let stored = if enabled {
                TYPE_MONITOR_AND_OUTPUT
            } else {
                TYPE_NONE
            };
            assert_eq!(s.monitoring_type, stored);
            assert_eq!(s.monitor, enabled);
            assert_eq!(
                s.trace,
                if enabled {
                    "monitor_on;create;"
                } else {
                    "monitor_off;destroy;"
                }
            );
            // the mute is not touched
            assert_eq!(s.user_muted, muted);
        }
    }
}

#[test]
fn steps_balance_steps() {
    let handler = [f32::NAN, 2.0, -1.0, 0.75, -0.0];
    let want = [0.5f32, 1.0, 0.0, 0.75, -0.0];

    // the value sent is clamped
    let mut s = StandIn::source();
    s.set_balance_value::<Core>(f32::NAN);
    assert_eq!(bits(s.sent_balance()), bits(0.5));
    assert_eq!(bits(s.balance()), bits(0.5));
    s.set_balance_value::<Core>(3.0);
    assert_eq!(bits(s.sent_balance()), bits(1.0));
    assert_eq!(bits(s.balance()), bits(1.0));
    s.set_balance_value::<Core>(f32::NEG_INFINITY);
    assert_eq!(bits(s.sent_balance()), bits(0.0));
    assert_eq!(bits(s.balance()), bits(0.0));

    // and again the value a handler leaves (#2051)
    for (h, w) in handler.iter().zip(want) {
        let mut s = StandIn::source();
        s.handler_sets = true;
        s.handler_balance_bits = f64::from(*h).to_bits();
        s.set_balance_value::<Core>(0.25);
        assert_eq!(bits(s.sent_balance()), bits(0.25));
        assert_eq!(bits(s.balance()), bits(w));
        assert_eq!(s.signals, 1);
    }

    // an invalid source: nothing
    let mut s = StandIn::source();
    s.valid = false;
    s.set_balance_value::<Core>(0.0);
    assert_eq!(s.signals, 0);
    assert_eq!(bits(s.balance()), bits(0.5));
}

#[test]
fn steps_mixers_steps() {
    // an audio source: the same value does nothing
    let mut s = StandIn::source();
    s.audio_mixers = 0x3f;
    s.set_audio_mixers::<Core>(0x3f);
    assert_eq!(s.signals, 0);
    s.set_audio_mixers::<Core>(0x5);
    assert_eq!(s.signals, 1);
    assert_eq!(s.sent_mixers, 0x5);
    assert_eq!(s.audio_mixers, 0x5);
    assert_eq!(s.get_audio_mixers::<Core>(), 0x5);

    // a handler's value, truncated: characterized, not endorsed
    let mut s = StandIn::source();
    s.handler_sets = true;
    s.handler_mixers = 0x1_0000_0003;
    s.set_audio_mixers::<Core>(1);
    assert_eq!(s.audio_mixers, 3);
    s.handler_mixers = -1;
    s.set_audio_mixers::<Core>(1);
    assert_eq!(s.audio_mixers, 0xffff_ffff);
    assert_eq!(s.signals, 2);

    // a video-only source: no mixers
    let mut s = StandIn::source();
    s.output_flags = SOURCE_VIDEO;
    s.audio_mixers = 0x3f;
    s.set_audio_mixers::<Core>(1);
    assert_eq!(s.signals, 0);
    assert_eq!(s.audio_mixers, 0x3f);
    assert_eq!(s.get_audio_mixers::<Core>(), 0);

    // a video-only source that owns its info id: mixers
    s.owns_info_id = true;
    s.set_audio_mixers::<Core>(1);
    assert_eq!(s.signals, 1);
    assert_eq!(s.get_audio_mixers::<Core>(), 1);

    // an invalid source
    let mut s = StandIn::source();
    s.valid = false;
    s.audio_mixers = 7;
    s.set_audio_mixers::<Core>(1);
    assert_eq!(s.audio_mixers, 7);
    assert_eq!(s.get_audio_mixers::<Core>(), 0);
}

#[test]
fn steps_flags_steps() {
    let mut s = StandIn::source();
    s.set_flags::<Core>(0);
    assert_eq!(s.signals, 0);
    s.set_flags::<Core>(FLAG_FORCE_MONO);
    assert_eq!(s.signals, 1);
    assert_eq!(s.flags, FLAG_FORCE_MONO);
    s.set_flags::<Core>(FLAG_FORCE_MONO);
    assert_eq!(s.signals, 1);
    s.set_flags::<Core>(0xffff_ffff);
    assert_eq!(s.signals, 2);
}

#[test]
fn steps_push_to_steps() {
    // the same value: stored, no signal, no log
    let mut s = StandIn::source();
    s.enable_push_to::<Core>(false, false);
    s.enable_push_to::<Core>(true, false);
    assert_eq!(s.trace, "");

    // a change on an audio source: log, then signal
    s.enable_push_to::<Core>(false, true);
    assert_eq!(s.trace, "log;push_to_mute_changed;");
    assert!(s.push_to_mute_enabled);
    s.enable_push_to::<Core>(true, true);
    assert_eq!(
        s.trace,
        "log;push_to_mute_changed;log;push_to_talk_changed;"
    );
    assert!(s.push_to_talk_enabled);
    s.enable_push_to::<Core>(false, true);
    assert_eq!(s.signals, 2);
    assert_eq!(s.logs, 2);

    // a video-only source: the signal, no log
    let mut s = StandIn::source();
    s.output_flags = SOURCE_VIDEO;
    s.enable_push_to::<Core>(false, true);
    s.enable_push_to::<Core>(true, true);
    assert_eq!(s.trace, "push_to_mute_changed;push_to_talk_changed;");
    assert_eq!(s.logs, 0);
}

#[test]
fn steps_audio_active_steps() {
    let mut s = StandIn::source();
    s.set_audio_active::<Core>(true);
    s.set_audio_active::<Core>(true);
    assert_eq!(s.trace, "audio_activate;");
    s.set_audio_active::<Core>(false);
    s.set_audio_active::<Core>(false);
    assert_eq!(s.trace, "audio_activate;audio_deactivate;");
    assert_eq!(s.signals, 2);
}

// New edge cases.

#[test]
fn edge_flag_decisions_every_combination() {
    for a in [false, true] {
        for b in [false, true] {
            assert_eq!(monitor(a, b), Master::monitor(a, b));
            assert_eq!(audio_active(a, b), Master::audio_active(a, b));
            assert_eq!(changes(u32::from(a), u32::from(b)), a != b);
            for &flags in &U32S {
                assert_eq!(push_to_logs(flags, b), Master::push_to_logs(flags, b));
            }
        }
        assert_eq!(on_type(a), Master::on_type(a));
        assert_eq!(stored_type(a), Master::stored_type(a));
        assert_eq!(off_reads_mute(a), a);
        assert_eq!(off_unmutes(a), a);
        assert_eq!(only_mutes(a), !a);
    }
}

#[test]
fn edge_route_range() {
    for t in -100_000..=100_000 {
        assert_eq!(route(t), master_route(t));
    }
    for t in [i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX] {
        assert_eq!(route(t), Route::On);
    }
    for bit in 0..31 {
        let v = 1i32 << bit;
        for t in [v, v | 1, v | 2, -v, v - 1] {
            assert_eq!(route(t), master_route(t));
        }
    }
}

#[test]
fn edge_mixers_powers_of_two() {
    for bit in 0..63 {
        let v = 1i64 << bit;
        for x in [v - 1, v, v + 1, -v, -v - 1, -v + 1] {
            assert_eq!(mixers_store(x), master_mixers(x));
        }
    }
    for x in [i64::MIN, i64::MIN + 1, i64::MAX - 1, i64::MAX, 0] {
        assert_eq!(mixers_store(x), master_mixers(x));
    }
    for bit in 0..32 {
        let f = 1u32 << bit;
        assert_eq!(has_audio(f), Master::has_audio(f));
        assert_eq!(has_audio(!f), Master::has_audio(!f));
    }
}

#[test]
fn edge_balance_specials_and_steps() {
    let specials = [
        0u32,
        0x8000_0000,
        1,
        0x8000_0001,
        0x007f_ffff,
        0x0080_0000,
        0x3f00_0000,
        0x3f7f_ffff,
        0x3f80_0000,
        0x3f80_0001,
        0xbf80_0000,
        0x7f7f_ffff,
        0xff7f_ffff,
        0x7f80_0000,
        0xff80_0000,
        0x7f80_0001,
        0x7fbf_ffff,
        0x7fc0_0000,
        0x7fff_ffff,
        0xff80_0001,
        0xffc0_0000,
        0xffff_ffff,
    ];
    for b in specials {
        let v = f32::from_bits(b);
        assert_eq!(bits(balance(v)), bits(master_balance(v)), "{b:#x}");
    }
    // every float from -2.0 to 2.0 in steps of 1/4096 of its exponent
    let mut v = -2.0f32;
    while v <= 2.0 {
        assert_eq!(bits(balance(v)), bits(master_balance(v)), "{v}");
        v += 1.0 / 4096.0;
    }
    // every bit pattern in strides
    let mut b: u64 = 0;
    while b <= u64::from(u32::MAX) {
        let v = f32::from_bits(u32::try_from(b).unwrap());
        assert_eq!(bits(balance(v)), bits(master_balance(v)));
        b += 65_521;
    }
}

#[derive(Debug, Clone)]
enum Op {
    SetFlags(u32),
    SetMixers(u32),
    GetMixers,
    PushTo(bool, bool),
    SetEnabled(bool),
    SetType(i32),
    GetType,
    SetBalance(u32),
    SetActive(bool),
    UserMute(bool),
    Handler(bool, i64, u32),
    OutputFlags(u32),
    OwnsInfoId(bool),
}

fn any_type() -> impl Strategy<Value = i32> {
    prop_oneof![
        Just(0),
        Just(1),
        Just(2),
        Just(3),
        Just(-1),
        Just(4),
        Just(i32::MIN),
        Just(i32::MAX),
        any::<i32>(),
    ]
}

fn any_float_bits() -> impl Strategy<Value = u32> {
    prop_oneof![
        any::<u32>(),
        Just(0x7fc0_0000u32),
        Just(0x7f80_0001),
        Just(0xffc0_0000),
        Just(0x8000_0000),
        Just(0),
        Just(1),
        Just(0x3f80_0000),
        Just(0x3f80_0001),
        Just(0x3f7f_ffff),
        Just(0x7f80_0000),
        Just(0xff80_0000),
        (0u32..=0x3f80_0000),
    ]
}

fn any_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        any::<u32>().prop_map(Op::SetFlags),
        (0u32..8).prop_map(Op::SetFlags),
        any::<u32>().prop_map(Op::SetMixers),
        (0u32..8).prop_map(Op::SetMixers),
        Just(Op::GetMixers),
        (any::<bool>(), any::<bool>()).prop_map(|(t, e)| Op::PushTo(t, e)),
        any::<bool>().prop_map(Op::SetEnabled),
        any_type().prop_map(Op::SetType),
        any_type().prop_map(Op::SetType),
        Just(Op::GetType),
        any_float_bits().prop_map(Op::SetBalance),
        any::<bool>().prop_map(Op::SetActive),
        any::<bool>().prop_map(Op::UserMute),
        (any::<bool>(), any::<i64>(), any_float_bits()).prop_map(|(s, m, b)| Op::Handler(s, m, b)),
        prop_oneof![any::<u32>(), Just(AUDIO), Just(SOURCE_VIDEO), Just(0)]
            .prop_map(Op::OutputFlags),
        any::<bool>().prop_map(Op::OwnsInfoId),
    ]
}

fn apply<R: Rules>(s: &mut StandIn, op: &Op, out: &mut Vec<i64>) {
    match *op {
        Op::SetFlags(f) => s.set_flags::<R>(f),
        Op::SetMixers(m) => s.set_audio_mixers::<R>(m),
        Op::GetMixers => out.push(i64::from(s.get_audio_mixers::<R>())),
        Op::PushTo(talk, e) => s.enable_push_to::<R>(talk, e),
        Op::SetEnabled(e) => s.set_monitoring_enabled::<R>(e),
        Op::SetType(t) => s.set_monitoring_type::<R>(t),
        Op::GetType => out.push(i64::from(s.get_monitoring_type::<R>())),
        Op::SetBalance(b) => s.set_balance_value::<R>(f32::from_bits(b)),
        Op::SetActive(a) => s.set_audio_active::<R>(a),
        Op::UserMute(m) => s.set_muted(m),
        Op::Handler(sets, m, b) => {
            s.handler_sets = sets;
            s.handler_mixers = m;
            s.handler_balance_bits = f64::from(f32::from_bits(b)).to_bits();
        }
        Op::OutputFlags(f) => s.output_flags = f,
        Op::OwnsInfoId(o) => s.owns_info_id = o,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn edge_full_range_decisions(
        a in any::<u32>(),
        b in any::<u32>(),
        m in any::<i64>(),
        t in any::<i32>(),
        f in any::<u32>(),
        x in any::<bool>(),
        y in any::<bool>(),
    ) {
        prop_assert_eq!(changes(a, b), a != b);
        prop_assert!(!changes(a, a));
        prop_assert_eq!(has_audio(a), Master::has_audio(a));
        prop_assert_eq!(mixers_store(m), master_mixers(m));
        prop_assert_eq!(push_to_logs(a, x), Master::push_to_logs(a, x));
        prop_assert_eq!(route(t), master_route(t));
        let v = f32::from_bits(f);
        prop_assert_eq!(bits(balance(v)), bits(master_balance(v)));
        prop_assert_eq!(monitor(x, y), Master::monitor(x, y));
        prop_assert_eq!(audio_active(x, y), Master::audio_active(x, y));
    }

    #[test]
    fn edge_sequences_match_master(
        valid in prop::bool::weighted(0.9),
        on in any::<bool>(),
        muted in any::<bool>(),
        mark in any::<bool>(),
        active in any::<bool>(),
        ops in prop::collection::vec(any_op(), 1..12),
    ) {
        let mut a = StandIn::source();
        a.valid = valid;
        a.monitoring_enabled = on;
        a.monitor = on;
        a.monitoring_type = if on { TYPE_MONITOR_AND_OUTPUT } else { TYPE_NONE };
        a.user_muted = muted;
        a.monitor_only_muted = mark;
        a.audio_active = active;
        let mut b = a.clone();
        let (mut out_a, mut out_b) = (Vec::new(), Vec::new());
        for op in &ops {
            apply::<Core>(&mut a, op, &mut out_a);
            apply::<Master>(&mut b, op, &mut out_b);
            prop_assert_eq!(&a, &b);
            prop_assert_eq!(&out_a, &out_b);
        }
    }
}
