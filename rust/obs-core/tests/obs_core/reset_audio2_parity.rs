//! Tier 3: the original `obs_reset_audio2()` (oracle `reset_audio2.c`) and
//! the Rust shim on one recording fixture core, which the shim reaches
//! through the oracle's copies of the real `obs-reset-audio2-fields.c.h`
//! adapters, with the oracle's `blog` stand-in. For each setup (core or none,
//! an output or none, active or not, an info or NULL, its four fields, the
//! open's answer and the fields stored before) both must return the same
//! result, make the same steps in the same order (the handle asked about,
//! the free, the info passed to `obs_init_audio()` and the fields stored
//! when it is called, each `blog` level, format, `%d` and `%s`) and leave
//! the same output and buffering fields. Six mutants of the safe core's
//! steps must fail.
use core::cell::Cell;
use core::ffi::{c_char, c_int};

use obs_core::audio_info_queries::AudioInfo2;
use obs_core::ffi::audio_info_queries::obs_audio_info2;
use obs_core::ffi::reset_audio2::{
    Blog, Calls, LOG_INFO, RESET_FORMAT, SEPARATOR_FORMAT, obs_reset_audio2_with,
};
use obs_core::reset_audio2::{Line, Steps, reset_audio2};
use proptest::prelude::*;

type ResetFn = unsafe extern "C" fn(*const obs_audio_info2) -> bool;

unsafe extern "C" {
    fn oracle_reset_audio2_probe(
        candidate: ResetFn,
        core: bool,
        output: bool,
        active: bool,
        info: bool,
        samples_per_sec: u32,
        speakers: i32,
        max_buffering_ms: u32,
        fixed_buffering: bool,
        init_answer: bool,
        ticks: i32,
        fixed_buffer: bool,
        original: *mut bool,
        steps: *mut u32,
    ) -> u32;
    fn oracle_reset_audio2_blog(level: c_int, format: *const c_char, ...);

    fn obs_reset_audio2_core() -> bool;
    fn obs_reset_audio2_has_output() -> bool;
    fn obs_reset_audio2_output_active() -> bool;
    fn obs_reset_audio2_free();
    fn obs_reset_audio2_set_buffering(max_buffering_ticks: c_int, fixed_buffer: bool);
    fn obs_reset_audio2_init(samples_per_sec: u32, speakers: c_int) -> bool;
}

/// The oracle's `blog` stand-in.
enum Oracle {}

impl Calls for Oracle {
    fn blog() -> Blog {
        oracle_reset_audio2_blog
    }
}

/// The shim with the oracle's `blog`.
unsafe extern "C" fn shim(oai: *const obs_audio_info2) -> bool {
    // SAFETY: the probe passes NULL or a stack info and has set its fixture.
    unsafe { obs_reset_audio2_with::<Oracle>(oai) }
}

#[derive(Clone, Copy, Debug)]
struct Setup {
    core: bool,
    output: bool,
    active: bool,
    info: bool,
    samples_per_sec: u32,
    speakers: i32,
    max_buffering_ms: u32,
    fixed_buffering: bool,
    init_answer: bool,
    ticks: i32,
    fixed_buffer: bool,
}

/// The probe's difference bits (0 is a match), the original's result and
/// its step count.
fn probe(s: Setup, candidate: ResetFn) -> (u32, bool, u32) {
    let mut original = false;
    let mut steps = 0;
    // SAFETY: the probe runs both on its thread-local fixture and a stack
    // info; the out pointers are live.
    let diff = unsafe {
        oracle_reset_audio2_probe(
            candidate,
            s.core,
            s.output,
            s.active,
            s.info,
            s.samples_per_sec,
            s.speakers,
            s.max_buffering_ms,
            s.fixed_buffering,
            s.init_answer,
            s.ticks,
            s.fixed_buffer,
            &mut original,
            &mut steps,
        )
    };
    (diff, original, steps)
}

const MUTANTS: [&str; 6] = [
    "fixed flag dropped",
    "default ticks always",
    "no free",
    "buffering type swapped",
    "active asked without an output",
    "always true",
];

thread_local! {
    /// The mutant `mutant_shim` runs.
    static MUTANT: Cell<usize> = const { Cell::new(0) };
}

/// The shim's steps with one mutation (`.0`, an index of [`MUTANTS`]),
/// through the oracle's adapters, on the info `.1`.
struct Mutated(usize, *const obs_audio_info2);

impl Steps for Mutated {
    fn core(&mut self) -> bool {
        // SAFETY: the oracle's fixture.
        unsafe { obs_reset_audio2_core() }
    }
    fn has_output(&mut self) -> bool {
        // SAFETY: the oracle's fixture core.
        self.0 == 4 || unsafe { obs_reset_audio2_has_output() }
    }
    fn output_active(&mut self) -> bool {
        // SAFETY: the oracle's stand-in, which records the handle.
        unsafe { obs_reset_audio2_output_active() }
    }
    fn free_audio(&mut self) {
        if self.0 != 2 {
            // SAFETY: the oracle's stand-in.
            unsafe { obs_reset_audio2_free() }
        }
    }
    fn info(&mut self) -> Option<AudioInfo2> {
        // SAFETY: the probe passes NULL or a stack info.
        let oai = unsafe { self.1.as_ref() }?;
        Some(AudioInfo2 {
            samples_per_sec: oai.samples_per_sec,
            speakers: oai.speakers,
            max_buffering_ms: oai.max_buffering_ms,
            fixed_buffering: oai.fixed_buffering,
        })
    }
    fn set_buffering(&mut self, max_buffering_ticks: c_int, fixed_buffer: bool) {
        let ticks = if self.0 == 1 { 45 } else { max_buffering_ticks };
        // SAFETY: the oracle's fixture core.
        unsafe { obs_reset_audio2_set_buffering(ticks, fixed_buffer && self.0 != 0) }
    }
    fn log(&mut self, line: Line) {
        // SAFETY: the oracle's stand-in, with the shim's formats and
        // arguments.
        unsafe {
            match line {
                Line::Separator => oracle_reset_audio2_blog(LOG_INFO, SEPARATOR_FORMAT.as_ptr()),
                Line::Reset {
                    samples_per_sec,
                    speakers,
                    max_buffering_ms,
                    fixed,
                } => {
                    let fixed = fixed != (self.0 == 3);
                    let kind = if fixed {
                        c"fixed"
                    } else {
                        c"dynamically increasing"
                    };
                    oracle_reset_audio2_blog(
                        LOG_INFO,
                        RESET_FORMAT.as_ptr(),
                        samples_per_sec,
                        speakers,
                        max_buffering_ms,
                        kind.as_ptr(),
                    );
                }
            }
        }
    }
    fn init_audio(&mut self, samples_per_sec: u32, speakers: c_int) -> bool {
        // SAFETY: the oracle's stand-in.
        unsafe { obs_reset_audio2_init(samples_per_sec, speakers) }
    }
}

unsafe extern "C" fn mutant_shim(oai: *const obs_audio_info2) -> bool {
    let mutant = MUTANT.with(Cell::get);
    let result = reset_audio2(&mut Mutated(mutant, oai));
    result || mutant == 5
}

fn setup(core: bool, output: bool, active: bool, info: bool) -> Setup {
    Setup {
        core,
        output,
        active,
        info,
        samples_per_sec: 48000,
        speakers: 2,
        max_buffering_ms: 200,
        fixed_buffering: true,
        init_answer: true,
        ticks: 7,
        fixed_buffer: false,
    }
}

/// Every gate path, both answers of the open, and the test_reset_audio2
/// cases and edges.
fn fixed_setups() -> Vec<Setup> {
    let mut setups = Vec::new();
    for core in [false, true] {
        for output in [false, true] {
            for active in [false, true] {
                for info in [false, true] {
                    for init_answer in [false, true] {
                        setups.push(Setup {
                            init_answer,
                            ..setup(core, output, active, info)
                        });
                    }
                }
            }
        }
    }
    for (samples_per_sec, speakers, max_buffering_ms, fixed_buffering) in [
        (44100, 6, 500, false),
        (96000, 8, 1000, true),
        (32000, 1, 0, false),
        (22050, 5, 0, true),
        (192_000, 2, 22370, false),
        (0, 2, 200, false),
        (u32::MAX, -1, u32::MAX, true),
        (1, c_int::MIN, 1, false),
    ] {
        setups.push(Setup {
            samples_per_sec,
            speakers,
            max_buffering_ms,
            fixed_buffering,
            ..setup(true, true, false, true)
        });
    }
    setups
}

#[test]
fn shim_matches_the_original() {
    for s in fixed_setups() {
        assert_eq!(probe(s, shim).0, 0, "{s:?}");
    }
    // The reset path makes every step: the free, the separator, the
    // message and the open, after the active check with an output.
    assert_eq!(probe(setup(true, true, false, true), shim), (0, true, 5));
    assert_eq!(probe(setup(true, false, false, true), shim), (0, true, 4));
    assert_eq!(probe(setup(true, true, true, true), shim), (0, false, 1));
    assert_eq!(probe(setup(false, true, true, true), shim), (0, false, 0));
}

#[test]
fn mutants_are_caught() {
    // The unmutated wrapper matches, so each failure is its mutation's.
    MUTANT.with(|m| m.set(usize::MAX));
    for s in fixed_setups() {
        assert_eq!(probe(s, mutant_shim).0, 0, "{s:?}");
    }
    for (index, name) in MUTANTS.iter().enumerate() {
        MUTANT.with(|m| m.set(index));
        let caught = fixed_setups()
            .into_iter()
            .any(|s| probe(s, mutant_shim).0 != 0);
        assert!(caught, "mutant not caught: {name}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(512), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn shim_matches_the_original_on_any_setup(
        core in any::<bool>(),
        output in any::<bool>(),
        active in any::<bool>(),
        info in any::<bool>(),
        samples_per_sec in any::<u32>(),
        speakers in any::<i32>(),
        max_buffering_ms in any::<u32>(),
        fixed_buffering in any::<bool>(),
        init_answer in any::<bool>(),
        ticks in any::<i32>(),
        fixed_buffer in any::<bool>(),
    ) {
        let s = Setup {
            core,
            output,
            active,
            info,
            samples_per_sec,
            speakers,
            max_buffering_ms,
            fixed_buffering,
            init_answer,
            ticks,
            fixed_buffer,
        };
        prop_assert_eq!(probe(s, shim).0, 0, "{:?}", s);
    }
}
