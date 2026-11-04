//! Tier 1 for `obs_source::audio_settings`, the source audio settings of
//! `libobs/obs-source-audio-settings.c.h`.
//!
//! The cases run the functions on a fake source that records every call in
//! order, and panics on a field touched without the lock the C holds there, a
//! lock taken twice, or a signal sent with a lock held.
//! `test_source_audio_settings.c` pins the same signals on the real libobs.

use std::cell::{Cell, RefCell};
use std::ffi::CStr;

use obs_source::audio_settings::{
    BALANCE_INVALID, GET_TYPE_DEPRECATED, SET_TYPE_DEPRECATED, Source, VOLUME_INVALID,
    clamp_volume, get_balance_value, get_monitoring_enabled, get_monitoring_type, get_sync_offset,
    get_volume, muted, set_balance_value, set_monitoring_enabled, set_monitoring_type, set_muted,
    set_sync_offset, set_volume,
};
use obs_source::monitoring::{TYPE_MONITOR_AND_OUTPUT, TYPE_MONITOR_ONLY, TYPE_NONE};
use proptest::prelude::*;

const NOW: u64 = 1234;

thread_local! {
    /// The calls with no source: `valid`, `now` and `warn`.
    static STATIC_LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn static_log(entry: String) {
    STATIC_LOG.with(|log| log.borrow_mut().push(entry));
}

fn take_static_log() -> Vec<String> {
    STATIC_LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

#[derive(Default)]
struct Fake {
    log: RefCell<Vec<String>>,
    audio_buf_locked: Cell<bool>,
    actions_locked: Cell<bool>,
    private: Cell<bool>,
    user_volume: Cell<f32>,
    sync_offset: Cell<i64>,
    balance: Cell<f32>,
    monitoring_type: Cell<i32>,
    user_muted: Cell<bool>,
    monitor_only_muted: Cell<bool>,
    monitoring_enabled: Cell<bool>,
    /// What a handler puts in the calldata.
    volume_handler: Cell<Option<f64>>,
    sync_handler: Cell<Option<i64>>,
    balance_handler: Cell<Option<f64>>,
}

impl Fake {
    fn record(&self, entry: impl Into<String>) {
        self.log.borrow_mut().push(entry.into());
    }

    fn take_log(&self) -> Vec<String> {
        std::mem::take(&mut *self.log.borrow_mut())
    }

    fn need_audio_buf(&self, field: &str) {
        assert!(
            self.audio_buf_locked.get(),
            "{field} without audio_buf_mutex"
        );
    }

    fn need_actions(&self, field: &str) {
        assert!(
            self.actions_locked.get(),
            "{field} without audio_actions_mutex"
        );
    }

    fn no_lock_for_signal(&self, signal: &str) {
        assert!(
            !self.audio_buf_locked.get(),
            "{signal} with audio_buf_mutex"
        );
        assert!(
            !self.actions_locked.get(),
            "{signal} with audio_actions_mutex"
        );
    }
}

impl Source for Fake {
    fn valid(source: Option<&Self>, func: &'static CStr) -> bool {
        static_log(format!("valid {}", func.to_str().unwrap()));
        source.is_some()
    }

    fn now() -> u64 {
        static_log("now".to_owned());
        NOW
    }

    fn warn(message: &'static CStr) {
        static_log(format!("warn {}", message.to_str().unwrap()));
    }

    fn is_private(&self) -> bool {
        self.private.get()
    }

    fn lock_audio_buf(&self) {
        assert!(
            !self.audio_buf_locked.replace(true),
            "audio_buf_mutex twice"
        );
        self.record("lock audio_buf");
    }

    fn unlock_audio_buf(&self) {
        assert!(
            self.audio_buf_locked.replace(false),
            "audio_buf_mutex not held"
        );
        self.record("unlock audio_buf");
    }

    fn lock_actions(&self) {
        assert!(
            !self.actions_locked.replace(true),
            "audio_actions_mutex twice"
        );
        self.record("lock actions");
    }

    fn unlock_actions(&self) {
        assert!(
            self.actions_locked.replace(false),
            "audio_actions_mutex not held"
        );
        self.record("unlock actions");
    }

    fn user_volume(&self) -> f32 {
        self.need_audio_buf("user_volume");
        self.user_volume.get()
    }

    fn set_user_volume(&self, value: f32) {
        self.need_audio_buf("user_volume");
        self.record(format!("user_volume {value}"));
        self.user_volume.set(value);
    }

    fn sync_offset(&self) -> i64 {
        self.need_audio_buf("sync_offset");
        self.sync_offset.get()
    }

    fn set_sync_offset(&self, value: i64) {
        self.need_audio_buf("sync_offset");
        self.record(format!("sync_offset {value}"));
        self.sync_offset.set(value);
    }

    fn balance(&self) -> f32 {
        self.need_audio_buf("balance");
        self.balance.get()
    }

    fn set_balance(&self, value: f32) {
        self.need_audio_buf("balance");
        self.record(format!("balance {value}"));
        self.balance.set(value);
    }

    fn set_monitoring_type(&self, value: i32) {
        self.need_audio_buf("monitoring_type");
        self.record(format!("monitoring_type {value}"));
        self.monitoring_type.set(value);
    }

    fn user_muted(&self) -> bool {
        self.user_muted.get()
    }

    fn set_user_muted(&self, value: bool) {
        self.record(format!("user_muted {value}"));
        self.user_muted.set(value);
    }

    fn monitor_only_muted(&self) -> bool {
        self.monitor_only_muted.get()
    }

    fn set_monitor_only_muted(&self, value: bool) {
        self.record(format!("monitor_only_muted {value}"));
        self.monitor_only_muted.set(value);
    }

    fn monitoring_enabled(&self) -> bool {
        self.monitoring_enabled.get()
    }

    fn set_monitoring_enabled(&self, value: bool) {
        self.record(format!("monitoring_enabled {value}"));
        self.monitoring_enabled.set(value);
    }

    fn push_volume_action(&self, timestamp: u64, volume: f32) {
        self.need_actions("volume action");
        self.record(format!("action volume {timestamp} {volume}"));
    }

    fn push_mute_action(&self, timestamp: u64, muted: bool) {
        self.need_actions("mute action");
        self.record(format!("action mute {timestamp} {muted}"));
    }

    fn signal_volume(&self, volume: f64, also_core: bool) -> f64 {
        self.no_lock_for_signal("volume");
        self.record(format!("signal volume {volume} core={also_core}"));
        self.volume_handler.get().unwrap_or(volume)
    }

    fn signal_sync_offset(&self, offset: i64) -> i64 {
        self.no_lock_for_signal("audio_sync");
        self.record(format!("signal audio_sync {offset}"));
        self.sync_handler.get().unwrap_or(offset)
    }

    fn signal_balance(&self, balance: f64) -> f64 {
        self.no_lock_for_signal("audio_balance");
        self.record(format!("signal audio_balance {balance}"));
        self.balance_handler.get().unwrap_or(balance)
    }

    fn signal_muted(&self, muted: bool) {
        self.no_lock_for_signal("mute");
        self.record(format!("signal mute {muted}"));
    }

    fn signal_monitor(&self, enabled: bool) {
        self.no_lock_for_signal("monitor");
        self.record(format!("signal monitor {enabled}"));
    }

    fn signal_audio_monitoring(&self, monitoring_type: i32) {
        self.no_lock_for_signal("audio_monitoring");
        self.record(format!("signal audio_monitoring {monitoring_type}"));
    }

    fn monitor_create(&self) {
        self.record("monitor create");
    }

    fn monitor_destroy(&self) {
        self.record("monitor destroy");
    }
}

fn log(entries: &[&str]) -> Vec<String> {
    entries.iter().map(|e| (*e).to_owned()).collect()
}

#[test]
fn clamp_volume_keeps_zero_and_above() {
    assert_eq!(clamp_volume(f32::NAN), 1.0);
    assert_eq!(clamp_volume(f32::INFINITY), 20.0);
    assert_eq!(clamp_volume(f32::NEG_INFINITY), 0.0);
    assert_eq!(clamp_volume(-0.5), 0.0);
    assert_eq!(clamp_volume(0.0), 0.0);
    assert_eq!(clamp_volume(0.5), 0.5);
    assert_eq!(clamp_volume(25.0), 25.0);
    assert_eq!(clamp_volume(-0.0).to_bits(), (-0.0f32).to_bits());
}

#[test]
fn set_volume_signals_then_queues_then_stores() {
    let s = Fake::default();
    take_static_log();
    set_volume(Some(&s), 0.5);
    assert_eq!(
        take_static_log(),
        log(&["valid obs_source_set_volume", "now"])
    );
    assert_eq!(
        s.take_log(),
        log(&[
            "signal volume 0.5 core=true",
            "lock actions",
            "action volume 1234 0.5",
            "unlock actions",
            "lock audio_buf",
            "user_volume 0.5",
            "unlock audio_buf",
        ])
    );
}

#[test]
fn set_volume_of_a_private_source_skips_the_core_signal() {
    let s = Fake::default();
    s.private.set(true);
    set_volume(Some(&s), 0.5);
    assert_eq!(s.take_log()[0], "signal volume 0.5 core=false");
}

#[test]
fn set_volume_clamps_before_the_signal() {
    let s = Fake::default();
    set_volume(Some(&s), -1.0);
    assert_eq!(s.take_log()[0], "signal volume 0 core=true");
}

#[test]
fn a_volume_handler_changes_the_store_and_not_the_action() {
    let s = Fake::default();
    s.volume_handler.set(Some(-3.0));
    set_volume(Some(&s), 0.5);
    let log = s.take_log();
    assert!(log.contains(&"action volume 1234 0.5".to_owned()));
    assert!(log.contains(&"user_volume 0".to_owned()));
}

#[test]
fn set_volume_to_the_same_value_signals_and_queues() {
    let s = Fake::default();
    set_volume(Some(&s), 0.5);
    s.take_log();
    set_volume(Some(&s), 0.5);
    assert_eq!(s.take_log().len(), 7);
}

#[test]
fn null_sources_do_nothing_and_answer_the_invalid_value() {
    let none: Option<&Fake> = None;
    take_static_log();

    set_volume(none, 1.0);
    assert_eq!(get_volume(none).to_bits(), VOLUME_INVALID.to_bits());
    set_sync_offset(none, 1);
    assert_eq!(get_sync_offset(none), 0);
    assert!(!muted(none));
    set_muted(none, true);
    set_monitoring_enabled(none, true);
    assert!(!get_monitoring_enabled(none));
    set_monitoring_type(none, TYPE_MONITOR_ONLY);
    assert_eq!(get_monitoring_type(none), TYPE_NONE);
    set_balance_value(none, 1.0);
    assert_eq!(get_balance_value(none).to_bits(), BALANCE_INVALID.to_bits());

    let get_warn = format!("warn {}", GET_TYPE_DEPRECATED.to_str().unwrap());
    assert_eq!(
        take_static_log(),
        log(&[
            "valid obs_source_set_volume",
            "valid obs_source_get_volume",
            "valid obs_source_set_sync_offset",
            "valid obs_source_get_sync_offset",
            "valid obs_source_muted",
            // the C reads the clock before it checks the source
            "now",
            "valid obs_source_set_muted",
            "valid obs_source_set_monitoring_enabled",
            "valid obs_source_get_monitoring_enabled",
            "valid obs_source_set_monitoring_type",
            // the C warns before it checks the source
            &get_warn,
            "valid obs_source_get_monitoring_type",
            "valid obs_source_set_balance_value",
            "valid obs_source_get_balance_value",
        ])
    );
}

#[test]
fn getters_read_under_the_lock() {
    let s = Fake::default();
    s.user_volume.set(0.25);
    s.sync_offset.set(-7);
    s.balance.set(0.75);
    assert_eq!(get_volume(Some(&s)), 0.25);
    assert_eq!(get_sync_offset(Some(&s)), -7);
    assert_eq!(get_balance_value(Some(&s)), 0.75);
    assert_eq!(
        s.take_log(),
        log(&[
            "lock audio_buf",
            "unlock audio_buf",
            "lock audio_buf",
            "unlock audio_buf",
            "lock audio_buf",
            "unlock audio_buf",
        ])
    );
}

#[test]
fn set_sync_offset_stores_what_a_handler_left() {
    let s = Fake::default();
    set_sync_offset(Some(&s), 1000);
    assert_eq!(
        s.take_log(),
        log(&[
            "signal audio_sync 1000",
            "lock audio_buf",
            "sync_offset 1000",
            "unlock audio_buf",
        ])
    );
    s.sync_handler.set(Some(-2000));
    set_sync_offset(Some(&s), 3000);
    assert_eq!(s.sync_offset.get(), -2000);
}

#[test]
fn set_balance_clamps_for_the_signal_and_the_store() {
    let s = Fake::default();
    set_balance_value(Some(&s), 2.0);
    assert_eq!(
        s.take_log(),
        log(&[
            "signal audio_balance 1",
            "lock audio_buf",
            "balance 1",
            "unlock audio_buf",
        ])
    );
    s.balance_handler.set(Some(-3.0));
    set_balance_value(Some(&s), 0.25);
    assert_eq!(s.balance.get(), 0.0);
}

#[test]
fn set_muted_stores_clears_the_mark_signals_then_queues() {
    let s = Fake::default();
    s.monitor_only_muted.set(true);
    take_static_log();
    set_muted(Some(&s), true);
    assert_eq!(
        take_static_log(),
        log(&["now", "valid obs_source_set_muted"])
    );
    assert_eq!(
        s.take_log(),
        log(&[
            "user_muted true",
            "monitor_only_muted false",
            "signal mute true",
            "lock actions",
            "action mute 1234 true",
            "unlock actions",
        ])
    );
}

#[test]
fn set_monitoring_enabled_signals_stores_then_creates() {
    let s = Fake::default();
    set_monitoring_enabled(Some(&s), true);
    assert_eq!(
        s.take_log(),
        log(&[
            "signal monitor true",
            &format!("signal audio_monitoring {TYPE_MONITOR_AND_OUTPUT}"),
            "monitoring_enabled true",
            "lock audio_buf",
            &format!("monitoring_type {TYPE_MONITOR_AND_OUTPUT}"),
            "unlock audio_buf",
            "monitor create",
        ])
    );
}

#[test]
fn set_monitoring_enabled_off_destroys_the_monitor() {
    let s = Fake::default();
    s.monitoring_enabled.set(true);
    set_monitoring_enabled(Some(&s), false);
    assert_eq!(
        s.take_log(),
        log(&[
            "signal monitor false",
            &format!("signal audio_monitoring {TYPE_NONE}"),
            "monitoring_enabled false",
            "lock audio_buf",
            &format!("monitoring_type {TYPE_NONE}"),
            "unlock audio_buf",
            "monitor destroy",
        ])
    );
}

#[test]
fn set_monitoring_enabled_to_the_same_value_does_nothing() {
    let s = Fake::default();
    set_monitoring_enabled(Some(&s), false);
    assert!(s.take_log().is_empty());
}

#[test]
fn a_muted_monitored_source_is_monitor_only_for_the_signal_only() {
    let s = Fake::default();
    s.user_muted.set(true);
    set_monitoring_enabled(Some(&s), true);
    let log = s.take_log();
    assert!(log.contains(&format!("signal audio_monitoring {TYPE_MONITOR_ONLY}")));
    assert!(log.contains(&format!("monitoring_type {TYPE_MONITOR_AND_OUTPUT}")));
    assert_eq!(get_monitoring_type(Some(&s)), TYPE_MONITOR_ONLY);
}

#[test]
fn type_monitor_only_mutes_first_and_marks_it() {
    let s = Fake::default();
    take_static_log();
    set_monitoring_type(Some(&s), TYPE_MONITOR_ONLY);
    assert!(
        take_static_log().contains(&format!("warn {}", SET_TYPE_DEPRECATED.to_str().unwrap()))
    );
    let log = s.take_log();
    let mute = log.iter().position(|e| e == "user_muted true").unwrap();
    let on = log
        .iter()
        .position(|e| e == "monitoring_enabled true")
        .unwrap();
    assert!(mute < on);
    assert_eq!(log.last().unwrap(), "monitor_only_muted true");
}

#[test]
fn type_monitor_only_on_a_muted_source_does_not_mark_it() {
    let s = Fake::default();
    s.user_muted.set(true);
    set_monitoring_type(Some(&s), TYPE_MONITOR_ONLY);
    assert!(!s.monitor_only_muted.get());
    assert!(s.user_muted.get());
}

#[test]
fn type_none_undoes_only_the_mute_monitor_only_set() {
    let s = Fake::default();
    set_monitoring_type(Some(&s), TYPE_MONITOR_ONLY);
    set_monitoring_type(Some(&s), TYPE_NONE);
    assert!(!s.user_muted.get());
    assert!(!s.monitoring_enabled.get());
    assert!(!s.monitor_only_muted.get());

    s.user_muted.set(true);
    set_monitoring_type(Some(&s), TYPE_NONE);
    assert!(s.user_muted.get());
}

#[test]
fn a_mute_after_monitor_only_is_not_undone() {
    let s = Fake::default();
    set_monitoring_type(Some(&s), TYPE_MONITOR_ONLY);
    set_muted(Some(&s), true);
    set_monitoring_type(Some(&s), TYPE_NONE);
    assert!(s.user_muted.get());
}

#[test]
fn type_monitor_and_output_unmutes_then_turns_on() {
    let s = Fake::default();
    s.user_muted.set(true);
    set_monitoring_type(Some(&s), TYPE_MONITOR_AND_OUTPUT);
    let log = s.take_log();
    let mute = log.iter().position(|e| e == "user_muted false").unwrap();
    let on = log
        .iter()
        .position(|e| e == "monitoring_enabled true")
        .unwrap();
    assert!(mute < on);
}

#[test]
fn an_unknown_type_only_turns_monitoring_on() {
    let s = Fake::default();
    s.user_muted.set(true);
    set_monitoring_type(Some(&s), 77);
    assert!(s.monitoring_enabled.get());
    assert!(s.user_muted.get());
}

#[test]
fn get_type_warns_then_reads() {
    let s = Fake::default();
    take_static_log();
    assert_eq!(get_monitoring_type(Some(&s)), TYPE_NONE);
    assert_eq!(
        take_static_log(),
        log(&[
            &format!("warn {}", GET_TYPE_DEPRECATED.to_str().unwrap()),
            "valid obs_source_get_monitoring_type",
        ])
    );
    s.monitoring_enabled.set(true);
    assert_eq!(get_monitoring_type(Some(&s)), TYPE_MONITOR_AND_OUTPUT);
}

proptest! {
    #[test]
    fn clamp_volume_matches_the_c_expression(bits in any::<u32>()) {
        let volume = f32::from_bits(bits);
        let expected = if volume.is_nan() {
            1.0
        } else if volume < 0.0 {
            0.0
        } else if volume.is_infinite() {
            20.0
        } else {
            volume
        };
        prop_assert_eq!(clamp_volume(volume).to_bits(), expected.to_bits());
    }

    /// Any set of any value keeps the lock discipline the fake panics on.
    #[test]
    fn any_sequence_keeps_the_locks(ops in prop::collection::vec((0u8..6, any::<i32>()), 0..32)) {
        let s = Fake::default();
        for (op, value) in ops {
            match op {
                0 => set_volume(Some(&s), f32::from_bits(value.cast_unsigned())),
                1 => set_sync_offset(Some(&s), i64::from(value)),
                2 => set_balance_value(Some(&s), f32::from_bits(value.cast_unsigned())),
                3 => set_muted(Some(&s), value % 2 == 0),
                4 => set_monitoring_enabled(Some(&s), value % 2 == 0),
                _ => set_monitoring_type(Some(&s), value % 4),
            }
            prop_assert!(!s.audio_buf_locked.get());
            prop_assert!(!s.actions_locked.get());
        }
    }
}
