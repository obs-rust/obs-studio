//! Tier 3: the original `obs_set_cmdline_args()` (oracle
//! `set_cmdline_args.c`) and the Rust shim on the one process command line
//! behind the real `obs_core_cmdline_args_field()` adapter. Each run starts
//! from a stored count and table, makes a script of calls and records, after
//! every call, the stored count, whether a new block was stored, and the
//! table as offsets into its block with every string's bytes. The original
//! and the shim must record the same; three oracle mutants must not.
use super::core_cmdline_getter_parity::LOCK;
use core::ffi::{CStr, c_char, c_int, c_void};
use obs_core::ffi::core_cmdline_getter::obs_get_cmdline_args;
use obs_core::ffi::set_cmdline_args::obs_set_cmdline_args;
use proptest::prelude::*;
use std::ffi::CString;

/// One stored slot: its address offset from the table and its string.
type Slot = Option<(isize, Vec<u8>)>;
/// The arguments of each call of a script.
type Script = Vec<Vec<Vec<u8>>>;

type SetFn = unsafe extern "C" fn(c_int, *const *const c_char);

unsafe extern "C" {
    fn oracle_core_cmdline_set(argc: c_int, argv: *mut *mut c_char);
    fn oracle_obs_set_cmdline_args(argc: c_int, argv: *const *const c_char);
    fn oracle_mutant_set_cmdline_args_table_is_set(argc: c_int, argv: *const *const c_char);
    fn oracle_mutant_set_cmdline_args_reversed(argc: c_int, argv: *const *const c_char);
    fn oracle_mutant_set_cmdline_args_no_copy(argc: c_int, argv: *const *const c_char);
    fn bfree(ptr: *mut c_void);
}

/// The stored command line after one call.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stored {
    argc: c_int,
    /// A new table was stored by this call.
    new_block: bool,
    /// The table: `argv[i]` as an address offset from the table (a string
    /// outside the block too), `None` for NULL,
    /// with its string, up to and including `argv[argc]`.
    table: Option<Vec<Slot>>,
}

/// The start state: a count and, when it is not 0 or a table was set, a
/// table of `argc` strings stored by a first call.
#[derive(Debug, Clone)]
struct Start {
    argc: c_int,
    preset: Option<Vec<Vec<u8>>>,
}

fn snapshot(previous: *mut *mut c_char) -> Stored {
    // SAFETY: LOCK is held; the table, when there is one, was stored by the
    // function under test for a non-negative count, so it holds argc + 1
    // slots, each NULL or a NUL-terminated string.
    unsafe {
        let args = obs_get_cmdline_args();
        let table = (!args.argv.is_null() && args.argc >= 0).then(|| {
            let base = args.argv;
            (0..=usize::try_from(args.argc).unwrap())
                .map(|index| {
                    let slot = *args.argv.add(index);
                    (!slot.is_null()).then(|| {
                        (
                            slot.addr().wrapping_sub(base.addr()).cast_signed(),
                            CStr::from_ptr(slot).to_bytes().to_vec(),
                        )
                    })
                })
                .collect()
        });
        Stored {
            argc: args.argc,
            new_block: args.argv != previous,
            table,
        }
    }
}

fn run(set: SetFn, start: &Start, calls: &[Vec<Vec<u8>>]) -> Vec<Stored> {
    let mut blocks = Vec::new();
    // SAFETY: LOCK is held by the caller. Every call passes `argc` live
    // NUL-terminated strings; every block stored is freed once at the end,
    // and the stored command line is reset to 0/NULL.
    unsafe {
        oracle_core_cmdline_set(0, std::ptr::null_mut());
        if let Some(preset) = &start.preset {
            call(oracle_obs_set_cmdline_args, preset);
            blocks.push(obs_get_cmdline_args().argv);
        }
        let stored = obs_get_cmdline_args();
        oracle_core_cmdline_set(start.argc, stored.argv);

        let mut out = Vec::new();
        for args in calls {
            let previous = obs_get_cmdline_args().argv;
            call(set, args);
            let now = obs_get_cmdline_args().argv;
            if now != previous && !now.is_null() {
                blocks.push(now);
            }
            out.push(snapshot(previous));
        }

        oracle_core_cmdline_set(0, std::ptr::null_mut());
        for block in blocks {
            bfree(block.cast());
        }
        out
    }
}

unsafe fn call(set: SetFn, args: &[Vec<u8>]) {
    let strings: Vec<CString> = args
        .iter()
        .map(|arg| CString::new(arg.clone()).unwrap())
        .collect();
    let mut argv: Vec<*const c_char> = strings.iter().map(|arg| arg.as_ptr()).collect();
    argv.push(std::ptr::null());
    // SAFETY: argv holds `args.len()` live strings and a NULL.
    unsafe { set(c_int::try_from(args.len()).unwrap(), argv.as_ptr()) };
}

unsafe extern "C" fn rust_shim(argc: c_int, argv: *const *const c_char) {
    // SAFETY: forwarded unchanged from `call`.
    unsafe { obs_set_cmdline_args(argc, argv) }
}

fn compare(start: &Start, calls: &[Vec<Vec<u8>>]) -> Vec<Stored> {
    let expected = run(oracle_obs_set_cmdline_args, start, calls);
    let actual = run(rust_shim, start, calls);
    assert_eq!(actual, expected, "start {start:?}, calls {calls:?}");
    expected
}

fn bytes(args: &[&str]) -> Vec<Vec<u8>> {
    args.iter().map(|arg| arg.as_bytes().to_vec()).collect()
}

fn unset() -> Start {
    Start {
        argc: 0,
        preset: None,
    }
}

#[test]
fn characterized_scripts() {
    let _guard = LOCK.lock().unwrap();
    let four = bytes(&["obs", "--startreplaybuffer", "", "--scene=\u{e9} x"]);
    let scripts: &[(Start, Script)] = &[
        (unset(), vec![vec![]]),
        (unset(), vec![vec![], four.clone()]),
        (unset(), vec![four.clone(), bytes(&["later"])]),
        (unset(), vec![bytes(&["obs", "--verbose"]), vec![]]),
        (unset(), vec![vec![], vec![], bytes(&[""])]),
        (unset(), vec![vec![b"\xff\x80".to_vec(), vec![b'x'; 300]]]),
        (
            Start {
                argc: 2,
                preset: Some(bytes(&["obs", "--verbose"])),
            },
            vec![bytes(&["later"]), vec![]],
        ),
        (
            Start {
                argc: 0,
                preset: Some(vec![]),
            },
            vec![bytes(&["a", "bc"])],
        ),
        (
            Start {
                argc: -1,
                preset: None,
            },
            vec![bytes(&["a"])],
        ),
    ];
    for (start, calls) in scripts {
        compare(start, calls);
    }

    let first = compare(&unset(), std::slice::from_ref(&four));
    assert_eq!(first[0].argc, 4);
    assert!(first[0].new_block);
    let table = first[0].table.as_ref().unwrap();
    let ptr = isize::try_from(size_of::<*mut c_char>()).unwrap();
    assert_eq!(table[0].as_ref().unwrap().0, 5 * ptr);
    assert_eq!(table[4], None);
}

#[test]
fn mutants_are_observable() {
    let _guard = LOCK.lock().unwrap();
    let mutants: [(SetFn, Start, Script); 3] = [
        (
            oracle_mutant_set_cmdline_args_table_is_set,
            unset(),
            vec![vec![], bytes(&["obs"])],
        ),
        (
            oracle_mutant_set_cmdline_args_reversed,
            unset(),
            vec![bytes(&["a", "bc"])],
        ),
        (
            oracle_mutant_set_cmdline_args_no_copy,
            unset(),
            vec![bytes(&["a"])],
        ),
    ];
    for (mutant, start, calls) in &mutants {
        let expected = run(oracle_obs_set_cmdline_args, start, calls);
        assert_ne!(run(*mutant, start, calls), expected, "{calls:?}");
        assert_eq!(run(rust_shim, start, calls), expected, "{calls:?}");
    }
}

fn arg() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(1u8..=255, 0..12)
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn scripts_match_c(
        preset in prop::option::of(prop::collection::vec(arg(), 0..4)),
        keep_count in any::<bool>(),
        calls in prop::collection::vec(prop::collection::vec(arg(), 0..6), 1..4),
    ) {
        let _guard = LOCK.lock().unwrap();
        let argc = match (&preset, keep_count) {
            (Some(preset), true) => c_int::try_from(preset.len()).unwrap(),
            _ => 0,
        };
        compare(&Start { argc, preset }, &calls);
    }
}
