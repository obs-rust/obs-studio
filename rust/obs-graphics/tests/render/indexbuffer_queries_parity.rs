//! Tier 3 compares original C with the real Rust ABI and shared field adapters.
//! No intentional differences or exclusions; all backends are GPU-free fakes.
use obs_c_oracle as _;
use obs_graphics::ffi::indexbuffer_queries as rust;
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_int, c_uint, c_void};
use std::ptr;

unsafe extern "C" {
    fn oracle_gs_indexbuffer_get_data(buffer: *const c_void) -> *mut c_void;
    fn oracle_gs_indexbuffer_get_type(buffer: *const c_void) -> c_uint;
    fn oracle_indexbuffer_queries_reset(context: bool, data: *mut c_void, long_indices: bool);
    fn oracle_indexbuffer_queries_type(long_indices: bool) -> c_uint;
    fn oracle_indexbuffer_maintenance_calls() -> c_uint;
    fn oracle_indexbuffer_maintenance_logs() -> c_uint;
    fn oracle_indexbuffer_maintenance_kind() -> c_int;
    fn oracle_indexbuffer_maintenance_level() -> c_int;
    fn oracle_indexbuffer_maintenance_buffer() -> *const c_void;
    fn oracle_indexbuffer_maintenance_log() -> *const c_char;
}

static TOKENS: [u8; 2] = [0; 2];

#[derive(Clone, Copy, Debug)]
struct Step {
    context: bool,
    buffer: Option<usize>,
    data: Option<usize>,
    long_indices: bool,
    query_type: bool,
}

#[derive(Debug, PartialEq)]
struct Observed {
    data: *mut c_void,
    index_type: c_uint,
    calls: c_uint,
    kind: c_int,
    buffer: *const c_void,
    logs: c_uint,
    level: c_int,
    log: Vec<u8>,
}

fn token(value: Option<usize>) -> *mut c_void {
    value.map_or(ptr::null_mut(), |i| {
        ptr::from_ref(&TOKENS[i]).cast_mut().cast()
    })
}

fn run(use_rust: bool, step: Step) -> Observed {
    // SAFETY: the recorder and real C-layout context are thread-local; live
    // static tokens are compared only by identity by the recording callbacks.
    unsafe {
        oracle_indexbuffer_queries_reset(step.context, token(step.data), step.long_indices);
        let buffer = token(step.buffer);
        let mut data = ptr::null_mut();
        let mut index_type = 0;
        match (use_rust, step.query_type) {
            (true, false) => data = rust::gs_indexbuffer_get_data(buffer),
            (false, false) => data = oracle_gs_indexbuffer_get_data(buffer),
            (true, true) => index_type = rust::gs_indexbuffer_get_type(buffer),
            (false, true) => index_type = oracle_gs_indexbuffer_get_type(buffer),
        }
        Observed {
            data,
            index_type,
            calls: oracle_indexbuffer_maintenance_calls(),
            kind: oracle_indexbuffer_maintenance_kind(),
            buffer: oracle_indexbuffer_maintenance_buffer(),
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
    if step.context && step.buffer.is_some() {
        assert_eq!((ported.calls, ported.logs), (1, 0));
        assert_eq!(ported.kind, if step.query_type { 5 } else { 4 });
        assert_eq!(ported.buffer, token(step.buffer).cast_const());
        assert!(ported.log.is_empty());
        if step.query_type {
            // SAFETY: this pure helper measures the real public C enum values.
            assert_eq!(ported.index_type, unsafe {
                oracle_indexbuffer_queries_type(step.long_indices)
            });
        } else {
            assert_eq!(ported.data, token(step.data));
        }
    } else {
        assert_eq!((ported.calls, ported.index_type), (0, 0));
        assert!(ported.data.is_null());
        assert!(ported.buffer.is_null());
        assert_eq!((ported.logs, ported.level), (1, 400));
        let name = if step.query_type {
            "gs_indexbuffer_get_type"
        } else {
            "gs_indexbuffer_get_data"
        };
        let reason = if step.context {
            "Null 'indexbuffer' parameter"
        } else {
            "called while not in a graphics context"
        };
        assert_eq!(ported.log, format!("{name}: {reason}").as_bytes());
    }
}

#[test]
fn characterized_matrix_matches_c() {
    for context in [false, true] {
        for buffer in [None, Some(0), Some(1)] {
            for data in [None, Some(0), Some(1)] {
                for long_indices in [false, true] {
                    for query_type in [false, true] {
                        compare(Step {
                            context,
                            buffer,
                            data,
                            long_indices,
                            query_type,
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
        (any::<bool>(), prop::option::of(0usize..2), prop::option::of(0usize..2), any::<bool>(), any::<bool>()),
        1..=16,
    )) {
        for (context, buffer, data, long_indices, query_type) in steps {
            compare(Step { context, buffer, data, long_indices, query_type });
        }
    }
}
