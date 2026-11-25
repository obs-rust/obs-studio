//! Tier 3: the original C `obs_source_set_flags()` and
//! `obs_source_set_audio_mixers()` against the Rust exports, on noise-filled
//! real-header sources with a real signal handler: every signal (its source,
//! value, calldata shape and the field as the handler sees it), handlers
//! that rewrite or resize `mixers` or set again from inside, the stored
//! fields, every other byte, and the NULL log lines. No intentional
//! differences or excluded inputs. Each run allocates one fixture and at
//! most 32 operations.
use core::ffi::c_void;
use obs_source::ffi::source_flags_mixers::{
    Live, obs_source_set_audio_mixers, obs_source_set_flags,
};
use obs_source::flags_mixers::Source;
use obs_source::monitoring::{AUDIO, has_audio, mixers_store};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Setter = unsafe extern "C" fn(*mut c_void, u32);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

const OPS: usize = 32;
const EVENTS: usize = 4;
const TRACE: usize = 1 + 3 * EVENTS + 2;
const MODES: i32 = 4;

unsafe extern "C" {
    fn oracle_source_flags_mixers_run(
        set_flags: Setter,
        set_mixers: Setter,
        noise: u8,
        output_flags: u32,
        owns_info_id: bool,
        mode: i32,
        rewrite: i64,
        kinds: *const u8,
        values: *const u32,
        count: usize,
        trace: *mut u64,
    ) -> bool;
    fn oracle_source_flags_mixers_originals(
        set_flags: *mut Option<Setter>,
        set_mixers: *mut Option<Setter>,
    );
    fn oracle_source_flags_mixers_null_logs(
        set_flags: Setter,
        set_mixers: Setter,
        get: GetLog,
        set: SetLog,
    ) -> bool;
}

fn originals() -> (Setter, Setter) {
    let (mut set_flags, mut set_mixers) = (None, None);
    // SAFETY: the oracle writes both function pointers.
    unsafe { oracle_source_flags_mixers_originals(&raw mut set_flags, &raw mut set_mixers) };
    (set_flags.unwrap(), set_mixers.unwrap())
}

#[derive(Clone, Copy, Debug)]
struct Run<'a> {
    noise: u8,
    output_flags: u32,
    owns_info_id: bool,
    mode: i32,
    rewrite: i64,
    ops: &'a [(u8, u32)],
}

/// The trace of one run with the given setters; `None` when the oracle
/// found a byte other than the two fields written (or could not build it).
fn trace(setters: (Setter, Setter), run: Run<'_>) -> Option<Vec<u64>> {
    assert!(run.ops.len() <= OPS);
    let kinds: Vec<u8> = run.ops.iter().map(|op| op.0).collect();
    let values: Vec<u32> = run.ops.iter().map(|op| op.1).collect();
    let mut trace = vec![0; run.ops.len() * TRACE];
    // SAFETY: the C fixture owns a real-layout source and a live signal
    // handler; kinds and values hold count entries and trace TRACE per
    // operation; both setters have the original ABI.
    let valid = unsafe {
        oracle_source_flags_mixers_run(
            setters.0,
            setters.1,
            run.noise,
            run.output_flags,
            run.owns_info_id,
            run.mode,
            run.rewrite,
            kinds.as_ptr(),
            values.as_ptr(),
            run.ops.len(),
            trace.as_mut_ptr(),
        )
    };
    valid.then_some(trace)
}

const RUST: (Setter, Setter) = (obs_source_set_flags, obs_source_set_audio_mixers);

fn compare(run: Run<'_>) {
    let original = trace(originals(), run).unwrap_or_else(|| panic!("C: {run:?}"));
    let rust = trace(RUST, run).unwrap_or_else(|| panic!("Rust: {run:?}"));
    assert_eq!(rust, original, "{run:?}");
}

/// Operations that repeat, change, and wrap both fields.
const SEQUENCE: [(u8, u32); 12] = [
    (0, 0),
    (1, 0x3f),
    (1, 0x3f),
    (0, 2),
    (0, 2),
    (1, 0),
    (0, u32::MAX),
    (1, u32::MAX),
    (1, 5),
    (0, 0x5a5a_5a5a),
    (1, 0x5a5a_5a5a),
    (0, 0),
];

#[test]
fn setters_match_c_over_the_grid() {
    for noise in [0, 0x5a, 0xff] {
        for output_flags in [0, AUDIO, !AUDIO, u32::MAX, 1] {
            for owns_info_id in [false, true] {
                for mode in 0..MODES {
                    for rewrite in [0, 0x3f, -1, 0x1_0000_0005, i64::MIN, i64::MAX] {
                        compare(Run {
                            noise,
                            output_flags,
                            owns_info_id,
                            mode,
                            rewrite,
                            ops: &SEQUENCE,
                        });
                    }
                }
            }
        }
    }
}

#[test]
fn pinned_answers() {
    let run = Run {
        noise: 0,
        output_flags: AUDIO,
        owns_info_id: false,
        mode: 1,
        rewrite: 0x1_0000_0105,
        ops: &[(1, 3), (0, 2)],
    };
    let t = trace(RUST, run).unwrap();
    // One audio_mixers event from the fixture's source on the fixed stack,
    // carrying 3 while the field still holds 0; the store keeps the
    // handler's low 32 bits.
    assert_eq!(t[0], 1);
    assert_eq!(t[1] & 0xff, 1 | 2 | 4);
    assert_eq!(t[2], 3);
    assert_eq!(t[3], 0);
    assert_eq!(t[1 + 3 * EVENTS], 0x105);
    // update_flags carries the new flags, stored before the signal.
    let flags = &t[TRACE..];
    assert_eq!(flags[0], 1);
    assert_eq!(flags[1] & 0xff, 2 | 4);
    assert_eq!(flags[2], 2);
    assert_eq!(flags[3], 2);
    assert_eq!(flags[1 + 3 * EVENTS], 2 << 32 | 0x105);
}

/// Mutants over the real adapters and signals; each must disagree with the
/// original C somewhere. They run only on the oracle's live fixture source.
fn fixture(source: *mut c_void) -> Live {
    assert!(!source.is_null());
    // SAFETY: the oracle passes its live fixture source, which outlives the
    // setter call.
    unsafe { Live::new(source) }
}
unsafe extern "C" fn flags_signal_first(source: *mut c_void, flags: u32) {
    let s = fixture(source);
    if s.flags() != flags {
        s.signal_update_flags();
        s.store_flags(flags);
    }
}
unsafe extern "C" fn flags_always(source: *mut c_void, flags: u32) {
    let s = fixture(source);
    s.store_flags(flags);
    s.signal_update_flags();
}
unsafe extern "C" fn mixers_store_first(source: *mut c_void, mixers: u32) {
    let s = fixture(source);
    if (s.owns_info_id() || has_audio(s.output_flags())) && s.audio_mixers() != mixers {
        s.store_audio_mixers(mixers);
        let left = s.signal_audio_mixers(i64::from(mixers));
        s.store_audio_mixers(mixers_store(left));
    }
}
unsafe extern "C" fn mixers_ignore_handlers(source: *mut c_void, mixers: u32) {
    let s = fixture(source);
    if (s.owns_info_id() || has_audio(s.output_flags())) && s.audio_mixers() != mixers {
        s.signal_audio_mixers(i64::from(mixers));
        s.store_audio_mixers(mixers);
    }
}
unsafe extern "C" fn mixers_audio_only(source: *mut c_void, mixers: u32) {
    let s = fixture(source);
    if has_audio(s.output_flags()) && s.audio_mixers() != mixers {
        let left = s.signal_audio_mixers(i64::from(mixers));
        s.store_audio_mixers(mixers_store(left));
    }
}

#[test]
fn rejects_mutants() {
    let mutants: [(Setter, Setter); 5] = [
        (flags_signal_first, obs_source_set_audio_mixers),
        (flags_always, obs_source_set_audio_mixers),
        (obs_source_set_flags, mixers_store_first),
        (obs_source_set_flags, mixers_ignore_handlers),
        (obs_source_set_flags, mixers_audio_only),
    ];
    for mutant in mutants {
        let caught = (0..MODES).any(|mode| {
            [(AUDIO, false), (0, true)]
                .into_iter()
                .any(|(output_flags, owns_info_id)| {
                    let run = Run {
                        noise: 0x5a,
                        output_flags,
                        owns_info_id,
                        mode,
                        rewrite: 0x1_0000_0005,
                        ops: &SEQUENCE,
                    };
                    trace(mutant, run) != trace(originals(), run)
                })
        });
        assert!(caught, "a mutant agreed with the original C");
    }
}

#[test]
fn null_diagnostics_match_c() {
    if !crate::output::isolated(concat!(module_path!(), "::null_diagnostics_match_c")) {
        return;
    }
    // SAFETY: the isolated child owns the process log handler, which the C
    // probe restores; the callbacks use the base shim's opaque pointer ABI.
    unsafe {
        assert!(oracle_source_flags_mixers_null_logs(
            obs_source_set_flags,
            obs_source_set_audio_mixers,
            base_get_log_handler,
            base_set_log_handler,
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn arbitrary_runs_match_c(
        noise in any::<u8>(),
        output_flags in prop_oneof![Just(0_u32), Just(AUDIO), any::<u32>()],
        owns_info_id in any::<bool>(),
        mode in 0..MODES,
        rewrite in any::<i64>(),
        ops in prop::collection::vec((0..2_u8, prop_oneof![0..4_u32, any::<u32>()]), 0..=OPS),
    ) {
        compare(Run { noise, output_flags, owns_info_id, mode, rewrite, ops: &ops });
    }
}
