//! Tier 3: actual C layouts, original C bodies and real private adapters.
//! No intentional differences or input exclusions; tokens are never dereferenced.
use obs_c_oracle as _;
use obs_graphics::ffi::indexbuffer_maintenance as rust;
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr;

unsafe extern "C" {
    fn oracle_gs_indexbuffer_destroy(buffer: *mut c_void);
    fn oracle_gs_indexbuffer_flush(buffer: *mut c_void);
    fn oracle_gs_indexbuffer_flush_direct(buffer: *mut c_void, data: *const c_void);
    fn oracle_gs_indexbuffer_get_num_indices(buffer: *const c_void) -> usize;
    fn oracle_indexbuffer_maintenance_reset(context: bool, count: usize);
    fn oracle_indexbuffer_maintenance_calls() -> u32;
    fn oracle_indexbuffer_maintenance_logs() -> u32;
    fn oracle_indexbuffer_maintenance_kind() -> c_int;
    fn oracle_indexbuffer_maintenance_level() -> c_int;
    fn oracle_indexbuffer_maintenance_buffer() -> *const c_void;
    fn oracle_indexbuffer_maintenance_data() -> *const c_void;
    fn oracle_indexbuffer_maintenance_log() -> *const c_char;
}

static TOKENS: [u8; 2] = [0; 2];

#[derive(Clone, Copy, Debug)]
struct Step {
    context: bool,
    buffer: Option<usize>,
    data: Option<usize>,
    kind: u8,
    count: usize,
}

#[derive(Debug, PartialEq)]
struct Observed {
    result: usize,
    calls: u32,
    kind: i32,
    buffer: *const c_void,
    data: *const c_void,
    logs: u32,
    level: i32,
    log: Vec<u8>,
}

fn token(value: Option<usize>) -> *mut c_void {
    value.map_or(ptr::null_mut(), |i| {
        ptr::from_ref(&TOKENS[i]).cast_mut().cast()
    })
}

fn run(use_rust: bool, step: Step) -> Observed {
    // SAFETY: recorder and context are thread-local; live static tokens are
    // compared only by identity. The real vtable points to recording callbacks.
    unsafe {
        oracle_indexbuffer_maintenance_reset(step.context, step.count);
        let buffer = token(step.buffer);
        let data = token(step.data);
        let mut result = 0;
        match (use_rust, step.kind) {
            (true, 0) => rust::gs_indexbuffer_destroy(buffer),
            (false, 0) => oracle_gs_indexbuffer_destroy(buffer),
            (true, 1) => rust::gs_indexbuffer_flush(buffer),
            (false, 1) => oracle_gs_indexbuffer_flush(buffer),
            (true, 2) => rust::gs_indexbuffer_flush_direct(buffer, data),
            (false, 2) => oracle_gs_indexbuffer_flush_direct(buffer, data),
            (true, 3) => result = rust::gs_indexbuffer_get_num_indices(buffer),
            (false, 3) => result = oracle_gs_indexbuffer_get_num_indices(buffer),
            _ => unreachable!(),
        }
        Observed {
            result,
            calls: oracle_indexbuffer_maintenance_calls(),
            kind: oracle_indexbuffer_maintenance_kind(),
            buffer: oracle_indexbuffer_maintenance_buffer(),
            data: oracle_indexbuffer_maintenance_data(),
            logs: oracle_indexbuffer_maintenance_logs(),
            level: oracle_indexbuffer_maintenance_level(),
            log: CStr::from_ptr(oracle_indexbuffer_maintenance_log())
                .to_bytes()
                .to_vec(),
        }
    }
}

fn compare(step: Step) {
    let original = run(false, step);
    let ported = run(true, step);
    assert_eq!(ported, original, "{step:?}");
    if step.context && step.buffer.is_some() && (step.kind != 2 || step.data.is_some()) {
        assert_eq!((ported.calls, ported.logs), (1, 0));
        assert_eq!(ported.kind, i32::from(step.kind));
        assert_eq!(ported.buffer, token(step.buffer).cast_const());
        assert_eq!(
            ported.data,
            if step.kind == 2 {
                token(step.data).cast_const()
            } else {
                ptr::null()
            }
        );
        assert_eq!(ported.result, if step.kind == 3 { step.count } else { 0 });
    } else {
        assert_eq!((ported.calls, ported.result), (0, 0));
        let reason = if !step.context {
            Some("called while not in a graphics context")
        } else if step.kind == 0 {
            None
        } else if step.buffer.is_none() {
            Some("Null 'indexbuffer' parameter")
        } else {
            Some("Null 'data' parameter")
        };
        if let Some(reason) = reason {
            assert_eq!((ported.logs, ported.level), (1, 400));
            let name = [
                "gs_indexbuffer_destroy",
                "gs_indexbuffer_flush",
                "gs_indexbuffer_flush_direct",
                "gs_indexbuffer_get_num_indices",
            ][usize::from(step.kind)];
            assert_eq!(ported.log, format!("{name}: {reason}").as_bytes());
        } else {
            assert_eq!(ported.logs, 0);
            assert!(ported.log.is_empty());
        }
    }
}

#[test]
fn characterized_matrix_matches_c() {
    for kind in 0..4 {
        for context in [false, true] {
            for buffer in [None, Some(0), Some(1)] {
                for data in [None, Some(0), Some(1)] {
                    for count in [0, 1, u32::MAX as usize, usize::MAX / 2 + 1, usize::MAX] {
                        compare(Step {
                            context,
                            buffer,
                            data,
                            kind,
                            count,
                        });
                    }
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..ProptestConfig::default()
    })]
    #[test]
    fn random_sequences_match_c(steps in prop::collection::vec(
        (any::<bool>(), prop::option::of(0usize..2), prop::option::of(0usize..2), 0u8..4, any::<usize>()),
        1..=16,
    )) {
        for (context, buffer, data, kind, count) in steps {
            compare(Step { context, buffer, data, kind, count });
        }
    }
}
