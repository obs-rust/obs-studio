//! Tier 3: actual C context/vtable layout, original bodies and real adapters.
//! No intentional differences. A missing callback is installed only where no
//! context or shader lets the query reach it; shader and parameter tokens are
//! identity-only.
use obs_c_oracle as _;
use obs_graphics::ffi::shader_matrix_queries as rust;
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr;

unsafe extern "C" {
    fn oracle_gs_shader_get_viewproj_matrix(shader: *const c_void) -> *mut c_void;
    fn oracle_gs_shader_get_world_matrix(shader: *const c_void) -> *mut c_void;
    fn oracle_shader_matrix_reset(
        context: bool,
        callbacks: u32,
        viewproj: *mut c_void,
        world: *mut c_void,
    );
    fn oracle_shader_matrix_calls(index: u32) -> u32;
    fn oracle_shader_matrix_logs() -> u32;
    fn oracle_shader_matrix_level() -> c_int;
    fn oracle_shader_matrix_shader() -> *const c_void;
    fn oracle_shader_matrix_log() -> *const c_char;
    fn oracle_shader_matrix_events() -> *const c_char;
}

static SHADERS: [u8; 2] = [0; 2];
static PARAMS: [u8; 2] = [0; 2];

#[derive(Clone, Copy, Debug)]
struct Step {
    context: bool,
    callbacks: u32,
    /// 0 = viewproj, 1 = world.
    query: u32,
    shader: Option<usize>,
    results: [Option<usize>; 2],
}
#[derive(Debug, PartialEq, Eq)]
struct Observed {
    result: *mut c_void,
    calls: [u32; 2],
    shader: *const c_void,
    logs: u32,
    level: c_int,
    message: Vec<u8>,
    events: Vec<u8>,
}
fn shader(slot: Option<usize>) -> *const c_void {
    slot.map_or(ptr::null(), |index| (&raw const SHADERS[index]).cast())
}
fn param(slot: Option<usize>) -> *mut c_void {
    slot.map_or(ptr::null_mut(), |index| {
        (&raw const PARAMS[index]).cast_mut().cast()
    })
}
fn run(ported: bool, step: Step) -> Observed {
    let input = shader(step.shader);
    // SAFETY: the recording fixture, context and logging are thread-local.
    // A live query has its callback installed; tokens are identity-only.
    unsafe {
        oracle_shader_matrix_reset(
            step.context,
            step.callbacks,
            param(step.results[0]),
            param(step.results[1]),
        );
        let result = match (ported, step.query) {
            (true, 0) => rust::gs_shader_get_viewproj_matrix(input),
            (false, 0) => oracle_gs_shader_get_viewproj_matrix(input),
            (true, 1) => rust::gs_shader_get_world_matrix(input),
            (false, 1) => oracle_gs_shader_get_world_matrix(input),
            _ => unreachable!(),
        };
        Observed {
            result,
            calls: [oracle_shader_matrix_calls(0), oracle_shader_matrix_calls(1)],
            shader: oracle_shader_matrix_shader(),
            logs: oracle_shader_matrix_logs(),
            level: oracle_shader_matrix_level(),
            message: CStr::from_ptr(oracle_shader_matrix_log())
                .to_bytes()
                .to_vec(),
            events: CStr::from_ptr(oracle_shader_matrix_events())
                .to_bytes()
                .to_vec(),
        }
    }
}
fn reaches_backend(step: Step) -> bool {
    step.context && step.shader.is_some()
}
fn compare(step: Step) {
    let original = run(false, step);
    let ported = run(true, step);
    assert_eq!(ported, original, "{step:?}");
    let function = [
        "gs_shader_get_viewproj_matrix",
        "gs_shader_get_world_matrix",
    ][step.query as usize];
    let mut calls = [0; 2];
    if reaches_backend(step) {
        calls[step.query as usize] = 1;
        assert_eq!(ported.result, param(step.results[step.query as usize]));
        assert_eq!(ported.shader, shader(step.shader));
        assert_eq!((ported.logs, ported.level), (0, 0));
        assert_eq!(ported.events, [b"VW"[step.query as usize]]);
    } else {
        let reason = if step.context {
            "Null 'shader' parameter"
        } else {
            "called while not in a graphics context"
        };
        assert!(ported.result.is_null());
        assert!(ported.shader.is_null());
        assert_eq!((ported.logs, ported.level), (1, 400));
        assert_eq!(ported.message, format!("{function}: {reason}").as_bytes());
        assert_eq!(ported.events, b"L");
    }
    assert_eq!(ported.calls, calls);
}
fn live(context: bool, callbacks: u32, query: u32, shader: Option<usize>) -> bool {
    !(context && shader.is_some()) || callbacks & (1 << query) != 0
}
#[test]
fn characterized_matrix_matches_c() {
    let slots = [None, Some(0), Some(1)];
    for context in [false, true] {
        for callbacks in 0..4 {
            for query in 0..2 {
                for shader in slots {
                    if !live(context, callbacks, query, shader) {
                        continue;
                    }
                    for viewproj in slots {
                        for world in slots {
                            compare(Step {
                                context,
                                callbacks,
                                query,
                                shader,
                                results: [viewproj, world],
                            });
                        }
                    }
                }
            }
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn random_queries_match_c(steps in prop::collection::vec(
        (any::<bool>(), 0u32..4, 0u32..2, prop::option::of(0usize..2), prop::option::of(0usize..2), prop::option::of(0usize..2)), 1..=16
    )) {
        for (context, callbacks, query, shader, viewproj, world) in steps {
            let callbacks = callbacks | if context && shader.is_some() { 1 << query } else { 0 };
            compare(Step { context, callbacks, query, shader, results: [viewproj, world] });
        }
    }
}
