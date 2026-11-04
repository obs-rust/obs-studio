//! Tier 3 for `obs_source_seam::ffi::source_audio_settings` against the
//! original `libobs/obs-source-audio-settings.c.h`
//! (`obs_c_oracle::source_audio_settings`, renamed `oracle_*`).
//!
//! Both sides run on their own test source (built by the oracle) from the same
//! starting fields, with handlers that can change the calldata. After every
//! call the two must agree on what it returned, on the log (each signal and
//! monitor call in order, its value and every field it saw), on the queued
//! action values, on every field and on both mutexes being free. The clock is
//! the one thing not compared: the two sides read it at different times.
//!
//! There is no intentional difference from the C.

use std::ffi::{c_char, c_int, c_void};
use std::ptr;

use obs_c_oracle::source_audio_settings as c;
use obs_source_seam::ffi::source_audio_settings as rs;
use proptest::prelude::*;

#[derive(Debug, Clone, Copy)]
struct Init {
    private_source: bool,
    replace: bool,
    replace_float: f64,
    replace_int: i64,
    volume: f32,
    balance: f32,
}

impl Init {
    fn plain() -> Self {
        Self {
            private_source: true,
            replace: false,
            replace_float: 0.0,
            replace_int: 0,
            volume: 1.0,
            balance: 0.5,
        }
    }
}

struct Pair {
    rust: *mut c_void,
    c: *mut c_void,
}

impl Pair {
    fn new(init: Init) -> Self {
        let new = || {
            // SAFETY: plain values.
            unsafe {
                c::oracle_audio_settings_new(
                    init.private_source,
                    init.replace,
                    init.replace_float,
                    init.replace_int,
                    init.volume,
                    init.balance,
                )
            }
        };
        Self {
            rust: new(),
            c: new(),
        }
    }

    fn state(source: *const c_void) -> String {
        let mut out = vec![0u8; 32768];
        // SAFETY: a live test source and a buffer of `out.len()` bytes.
        let len = unsafe {
            c::oracle_audio_settings_state(source, out.as_mut_ptr().cast::<c_char>(), out.len())
        };
        assert_ne!(len, 0, "the state does not fit");
        out.truncate(len);
        String::from_utf8(out).unwrap()
    }

    fn assert_same(&self, op: &Op) {
        assert_eq!(Self::state(self.rust), Self::state(self.c), "after {op:?}");
    }
}

impl Drop for Pair {
    fn drop(&mut self) {
        // SAFETY: both are live test sources, freed once.
        unsafe {
            c::oracle_audio_settings_free(self.rust);
            c::oracle_audio_settings_free(self.c);
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Op {
    SetVolume(f32),
    SetSync(i64),
    SetBalance(f32),
    SetMuted(bool),
    SetMonitoring(bool),
    SetType(c_int),
    Getters,
}

fn apply(pair: &Pair, op: Op) {
    // SAFETY: both are live test sources.
    unsafe {
        match op {
            Op::SetVolume(v) => {
                rs::obs_source_set_volume(pair.rust, v);
                c::oracle_obs_source_set_volume(pair.c, v);
            }
            Op::SetSync(v) => {
                rs::obs_source_set_sync_offset(pair.rust, v);
                c::oracle_obs_source_set_sync_offset(pair.c, v);
            }
            Op::SetBalance(v) => {
                rs::obs_source_set_balance_value(pair.rust, v);
                c::oracle_obs_source_set_balance_value(pair.c, v);
            }
            Op::SetMuted(v) => {
                rs::obs_source_set_muted(pair.rust, v);
                c::oracle_obs_source_set_muted(pair.c, v);
            }
            Op::SetMonitoring(v) => {
                rs::obs_source_set_monitoring_enabled(pair.rust, v);
                c::oracle_obs_source_set_monitoring_enabled(pair.c, v);
            }
            Op::SetType(v) => {
                rs::obs_source_set_monitoring_type(pair.rust, v);
                c::oracle_obs_source_set_monitoring_type(pair.c, v);
            }
            Op::Getters => {
                assert_eq!(
                    rs::obs_source_get_volume(pair.rust).to_bits(),
                    c::oracle_obs_source_get_volume(pair.c).to_bits()
                );
                assert_eq!(
                    rs::obs_source_get_sync_offset(pair.rust),
                    c::oracle_obs_source_get_sync_offset(pair.c)
                );
                assert_eq!(
                    rs::obs_source_get_balance_value(pair.rust).to_bits(),
                    c::oracle_obs_source_get_balance_value(pair.c).to_bits()
                );
                assert_eq!(
                    rs::obs_source_muted(pair.rust),
                    c::oracle_obs_source_muted(pair.c)
                );
                assert_eq!(
                    rs::obs_source_get_monitoring_enabled(pair.rust),
                    c::oracle_obs_source_get_monitoring_enabled(pair.c)
                );
                assert_eq!(
                    rs::obs_source_get_monitoring_type(pair.rust),
                    c::oracle_obs_source_get_monitoring_type(pair.c)
                );
            }
        }
    }
    pair.assert_same(&op);
}

#[test]
fn null_sources_answer_the_same() {
    // SAFETY: NULL is what these check for.
    unsafe {
        rs::obs_source_set_volume(ptr::null_mut(), 1.0);
        c::oracle_obs_source_set_volume(ptr::null_mut(), 1.0);
        assert_eq!(
            rs::obs_source_get_volume(ptr::null()).to_bits(),
            c::oracle_obs_source_get_volume(ptr::null()).to_bits()
        );
        assert_eq!(
            rs::obs_source_get_balance_value(ptr::null()).to_bits(),
            c::oracle_obs_source_get_balance_value(ptr::null()).to_bits()
        );
        assert_eq!(
            rs::obs_source_get_sync_offset(ptr::null()),
            c::oracle_obs_source_get_sync_offset(ptr::null())
        );
        assert_eq!(
            rs::obs_source_muted(ptr::null()),
            c::oracle_obs_source_muted(ptr::null())
        );
        assert_eq!(
            rs::obs_source_get_monitoring_enabled(ptr::null()),
            c::oracle_obs_source_get_monitoring_enabled(ptr::null())
        );
        assert_eq!(
            rs::obs_source_get_monitoring_type(ptr::null()),
            c::oracle_obs_source_get_monitoring_type(ptr::null())
        );
        rs::obs_source_set_sync_offset(ptr::null_mut(), 1);
        rs::obs_source_set_balance_value(ptr::null_mut(), 1.0);
        rs::obs_source_set_muted(ptr::null_mut(), true);
        rs::obs_source_set_monitoring_enabled(ptr::null_mut(), true);
        rs::obs_source_set_monitoring_type(ptr::null_mut(), 1);
    }
}

#[test]
fn a_source_that_is_not_private_also_signals_the_core() {
    let pair = Pair::new(Init {
        private_source: false,
        ..Init::plain()
    });
    apply(&pair, Op::SetVolume(0.5));
    let expected = format!("source_volume {:016x}", 0.5f64.to_bits());
    assert!(Pair::state(pair.rust).contains(&expected));
}

#[test]
fn a_private_source_does_not_signal_the_core() {
    let pair = Pair::new(Init::plain());
    apply(&pair, Op::SetVolume(0.5));
    assert!(!Pair::state(pair.rust).contains("source_volume"));
}

#[test]
fn handlers_change_the_stored_values_on_both_sides() {
    let pair = Pair::new(Init {
        replace: true,
        replace_float: 0.25,
        replace_int: -2000,
        ..Init::plain()
    });
    apply(&pair, Op::SetVolume(0.75));
    apply(&pair, Op::SetBalance(0.75));
    apply(&pair, Op::SetSync(3000));
    let state = Pair::state(pair.rust);
    assert!(state.ends_with(" free"), "{state}");
    let action = format!("action vol {:08x};", 0.75f32.to_bits());
    assert!(state.contains(&action), "the action keeps it: {state}");
    let fields = format!(
        "|vol {:08x} sync {:016x} bal {:08x}",
        0.25f32.to_bits(),
        (-2000i64).cast_unsigned(),
        0.25f32.to_bits()
    );
    assert!(state.contains(&fields), "{state}");
}

#[test]
fn the_monitor_is_created_and_destroyed_on_both_sides() {
    let pair = Pair::new(Init::plain());
    apply(&pair, Op::SetMonitoring(true));
    apply(&pair, Op::SetMonitoring(true));
    apply(&pair, Op::SetMonitoring(false));
    let state = Pair::state(pair.rust);
    assert_eq!(state.matches("monitor_create").count(), 1);
    assert_eq!(state.matches("monitor_destroy").count(), 1);
}

fn float_value() -> impl Strategy<Value = f32> {
    prop_oneof![
        any::<u32>().prop_map(f32::from_bits),
        -2.0f32..30.0,
        Just(f32::NAN),
        Just(f32::INFINITY),
        Just(f32::NEG_INFINITY),
        Just(-0.0f32),
    ]
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        float_value().prop_map(Op::SetVolume),
        any::<i64>().prop_map(Op::SetSync),
        float_value().prop_map(Op::SetBalance),
        any::<bool>().prop_map(Op::SetMuted),
        any::<bool>().prop_map(Op::SetMonitoring),
        (-1..5).prop_map(Op::SetType),
        Just(Op::Getters),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(64)))]

    #[test]
    fn any_sequence_matches_the_c(
        private_source in any::<bool>(),
        replace in any::<bool>(),
        replace_float in -2.0f64..30.0,
        replace_int in any::<i64>(),
        ops in prop::collection::vec(op(), 1..24),
    ) {
        let pair = Pair::new(Init {
            private_source,
            replace,
            replace_float,
            replace_int,
            volume: 1.0,
            balance: 0.5,
        });
        for op in ops {
            apply(&pair, op);
        }
    }
}
