//! Exact dispatch, device identity, call counts and diagnostics vs original C.
use obs_c_oracle as _;
use obs_graphics::ffi::stencil as rust;
use obs_graphics::stencil::Command;
use proptest::prelude::*;
use std::ffi::{CStr, c_char};

unsafe extern "C" {
    fn oracle_stencil_reset(present: bool);
    fn oracle_stencil_value(index: u32) -> i32;
    fn oracle_stencil_log() -> *const c_char;
    fn oracle_gs_enable_stencil_test(enable: bool);
    fn oracle_gs_enable_stencil_write(enable: bool);
    fn oracle_gs_stencil_function(side: i32, test: i32);
    fn oracle_gs_stencil_op(side: i32, fail: i32, zfail: i32, zpass: i32);
}

fn invoke(command: Command, original: bool) {
    // SAFETY: the thread-local oracle owns a real C context and recording device;
    // every callback and log accessor is synchronous and confined to this thread.
    unsafe {
        match (command, original) {
            (Command::Test(v), true) => oracle_gs_enable_stencil_test(v),
            (Command::Test(v), false) => rust::gs_enable_stencil_test(v),
            (Command::Write(v), true) => oracle_gs_enable_stencil_write(v),
            (Command::Write(v), false) => rust::gs_enable_stencil_write(v),
            (Command::Function { side, test }, true) => oracle_gs_stencil_function(side, test),
            (Command::Function { side, test }, false) => rust::gs_stencil_function(side, test),
            (
                Command::Op {
                    side,
                    fail,
                    zfail,
                    zpass,
                },
                true,
            ) => oracle_gs_stencil_op(side, fail, zfail, zpass),
            (
                Command::Op {
                    side,
                    fail,
                    zfail,
                    zpass,
                },
                false,
            ) => rust::gs_stencil_op(side, fail, zfail, zpass),
        }
    }
}

fn snapshot() -> ([i32; 11], Vec<u8>) {
    // SAFETY: oracle returns this thread's initialized counters and NUL-terminated log.
    unsafe {
        (
            std::array::from_fn(|i| oracle_stencil_value(i as u32)),
            CStr::from_ptr(oracle_stencil_log()).to_bytes().to_vec(),
        )
    }
}

fn compare(present: bool, commands: &[Command]) {
    // SAFETY: reset changes only the calling thread's fixture, never global logging.
    unsafe { oracle_stencil_reset(present) };
    let mut expected = Vec::new();
    for &command in commands {
        invoke(command, true);
        expected.push(snapshot());
    }
    // SAFETY: all preceding synchronous calls have completed before resetting.
    unsafe { oracle_stencil_reset(present) };
    for (i, (&command, expected)) in commands.iter().zip(expected).enumerate() {
        invoke(command, false);
        assert_eq!(snapshot(), expected, "command {command:?}");
        let (values, log) = snapshot();
        if present {
            assert_eq!(values[0], (i + 1) as i32);
            assert_eq!(values[8], 1); // exact device identity
            assert_eq!(values[9], 0);
            assert!(log.is_empty());
        } else {
            assert_eq!(values[0], 0);
            assert_eq!(values[9], (i + 1) as i32);
            assert_eq!(values[10], 400); // LOG_DEBUG
            let name = match command {
                Command::Test(_) => "gs_enable_stencil_test",
                Command::Write(_) => "gs_enable_stencil_write",
                Command::Function { .. } => "gs_stencil_function",
                Command::Op { .. } => "gs_stencil_op",
            };
            assert_eq!(
                log,
                format!("{name}: called while not in a graphics context").as_bytes()
            );
        }
    }
}

#[test]
fn test_graphics_stencil_exhaustive_matches_c() {
    let mut commands = Vec::new();
    for v in [false, true] {
        for _ in 0..2 {
            commands.extend([Command::Test(v), Command::Write(v)]);
        }
    }
    for side in 0..3 {
        for test in 0..8 {
            commands.push(Command::Function { side, test });
        }
        for fail in 0..6 {
            for zfail in 0..6 {
                for zpass in 0..6 {
                    commands.push(Command::Op {
                        side,
                        fail,
                        zfail,
                        zpass,
                    });
                }
            }
        }
    }
    assert_eq!(commands.len(), 680);
    compare(false, &commands[..4]);
    compare(true, &commands);
    compare(false, &commands[..4]);
}

fn command() -> impl Strategy<Value = Command> {
    prop_oneof![
        any::<bool>().prop_map(Command::Test),
        any::<bool>().prop_map(Command::Write),
        (0..3i32, 0..8i32).prop_map(|(side, test)| Command::Function { side, test }),
        (0..3i32, 0..6i32, 0..6i32, 0..6i32).prop_map(|(side, fail, zfail, zpass)| Command::Op {
            side,
            fail,
            zfail,
            zpass
        }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256), failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn stencil_sequences_match_c(sequence in prop::collection::vec((any::<bool>(), command()), 0..64)) {
        for (present, command) in sequence { compare(present, &[command, command]); }
    }
}
