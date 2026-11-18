//! Tier 3: original C bodies, actual graphics layouts and real private adapters.
//! No intentional differences or input exclusions; opaque tokens are never dereferenced.
use obs_c_oracle as _;
use obs_graphics::ffi::render_target_setters as rust;
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr;

unsafe extern "C" {
    fn oracle_gs_set_render_target(texture: *mut c_void, depth: *mut c_void);
    fn oracle_gs_set_cube_render_target(texture: *mut c_void, side: c_int, depth: *mut c_void);
    fn oracle_render_target_setters_reset(context: bool, device: *mut c_void);
    fn oracle_render_target_setters_calls() -> u32;
    fn oracle_render_target_setters_logs() -> u32;
    fn oracle_render_target_setters_kind() -> c_int;
    fn oracle_render_target_setters_level() -> c_int;
    fn oracle_render_target_setters_side() -> c_int;
    fn oracle_render_target_setters_device() -> *const c_void;
    fn oracle_render_target_setters_texture() -> *const c_void;
    fn oracle_render_target_setters_depth() -> *const c_void;
    fn oracle_render_target_setters_log() -> *const c_char;
}

static TOKENS: [u8; 3] = [0; 3];

#[derive(Clone, Copy, Debug)]
struct Step {
    context: bool,
    device: usize,
    texture: Option<usize>,
    depth: Option<usize>,
    cube: bool,
    side: i32,
}

#[derive(Debug, PartialEq)]
struct Observed {
    calls: u32,
    kind: i32,
    device: *const c_void,
    texture: *const c_void,
    depth: *const c_void,
    side: i32,
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
    // SAFETY: recorder and context are thread-local. Static live tokens are
    // compared only by identity, through actual recording vtable callbacks.
    unsafe {
        oracle_render_target_setters_reset(step.context, token(Some(step.device)));
        let texture = token(step.texture);
        let depth = token(step.depth);
        match (use_rust, step.cube) {
            (true, false) => rust::gs_set_render_target(texture, depth),
            (false, false) => oracle_gs_set_render_target(texture, depth),
            (true, true) => rust::gs_set_cube_render_target(texture, step.side, depth),
            (false, true) => oracle_gs_set_cube_render_target(texture, step.side, depth),
        }
        Observed {
            calls: oracle_render_target_setters_calls(),
            kind: oracle_render_target_setters_kind(),
            device: oracle_render_target_setters_device(),
            texture: oracle_render_target_setters_texture(),
            depth: oracle_render_target_setters_depth(),
            side: oracle_render_target_setters_side(),
            logs: oracle_render_target_setters_logs(),
            level: oracle_render_target_setters_level(),
            log: CStr::from_ptr(oracle_render_target_setters_log())
                .to_bytes()
                .to_vec(),
        }
    }
}

fn compare(step: Step) {
    let original = run(false, step);
    let ported = run(true, step);
    assert_eq!(ported, original, "{step:?}");
    if step.context {
        assert_eq!((ported.calls, ported.logs), (1, 0));
        assert_eq!(ported.kind, i32::from(step.cube));
        assert_eq!(ported.device, token(Some(step.device)).cast_const());
        assert_eq!(ported.texture, token(step.texture).cast_const());
        assert_eq!(ported.depth, token(step.depth).cast_const());
        assert_eq!(ported.side, if step.cube { step.side } else { 0 });
        assert!(ported.log.is_empty());
    } else {
        assert_eq!((ported.calls, ported.logs, ported.level), (0, 1, 400));
        let name = if step.cube {
            "gs_set_cube_render_target"
        } else {
            "gs_set_render_target"
        };
        assert_eq!(
            ported.log,
            format!("{name}: called while not in a graphics context").as_bytes()
        );
    }
}

#[test]
fn characterized_matrix_matches_c() {
    for context in [false, true] {
        for device in 0..3 {
            for texture in [None, Some(0), Some(1), Some(2)] {
                for depth in [None, Some(0), Some(1), Some(2)] {
                    for cube in [false, true] {
                        for side in [0, 1, 2, 3, 4, 5, -1, i32::MIN, i32::MAX] {
                            compare(Step {
                                context,
                                device,
                                texture,
                                depth,
                                cube,
                                side,
                            });
                        }
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
        (any::<bool>(), 0usize..3, prop::option::of(0usize..3), prop::option::of(0usize..3), any::<bool>(), any::<i32>()),
        1..=16,
    )) {
        for (context, device, texture, depth, cube, side) in steps {
            compare(Step { context, device, texture, depth, cube, side });
        }
    }
}
