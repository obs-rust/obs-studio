//! Tier 3: original C and Rust ABI dispatch over the same real layout and
//! thread-local recording backend. No intentional differences or exclusions.
use obs_c_oracle as _;
use obs_graphics::ffi::device_load as rust;
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr;

unsafe extern "C" {
    fn oracle_gs_load_vertexbuffer(buffer: *mut c_void);
    fn oracle_gs_load_indexbuffer(buffer: *mut c_void);
    fn oracle_gs_load_texture(texture: *mut c_void, unit: c_int);
    fn oracle_gs_load_samplerstate(sampler: *mut c_void, unit: c_int);
    fn oracle_gs_load_default_samplerstate(b_3d: bool, unit: c_int);
    fn oracle_device_load_reset(context: bool, device: *mut c_void);
    fn oracle_device_load_calls() -> u32;
    fn oracle_device_load_logs() -> u32;
    fn oracle_device_load_kind() -> c_int;
    fn oracle_device_load_unit() -> c_int;
    fn oracle_device_load_level() -> c_int;
    fn oracle_device_load_device() -> *const c_void;
    fn oracle_device_load_object() -> *const c_void;
    fn oracle_device_load_default_3d() -> bool;
    fn oracle_device_load_log() -> *const c_char;
}

static DEVICES: [u8; 2] = [0; 2];
static OBJECTS: [u8; 2] = [0; 2];

#[derive(Clone, Copy, Debug)]
struct Step {
    context: bool,
    device: usize,
    object: Option<usize>,
    kind: u8,
    unit: i32,
    b_3d: bool,
}

#[derive(Debug, PartialEq)]
struct Observed {
    calls: u32,
    kind: i32,
    device: *const c_void,
    object: *const c_void,
    unit: i32,
    b_3d: bool,
    logs: u32,
    level: i32,
    log: Vec<u8>,
}

fn device(i: usize) -> *mut c_void {
    ptr::from_ref(&DEVICES[i]).cast_mut().cast()
}

fn object(i: Option<usize>) -> *mut c_void {
    i.map_or(ptr::null_mut(), |i| {
        ptr::from_ref(&OBJECTS[i]).cast_mut().cast()
    })
}

fn run(use_rust: bool, step: Step) -> Observed {
    // SAFETY: the recorder and context are thread-local. Static tokens are
    // recorded by identity only; neither implementation dereferences them.
    unsafe {
        oracle_device_load_reset(step.context, device(step.device));
        let p = object(step.object);
        match (use_rust, step.kind) {
            (true, 0) => rust::gs_load_vertexbuffer(p),
            (false, 0) => oracle_gs_load_vertexbuffer(p),
            (true, 1) => rust::gs_load_indexbuffer(p),
            (false, 1) => oracle_gs_load_indexbuffer(p),
            (true, 2) => rust::gs_load_texture(p, step.unit),
            (false, 2) => oracle_gs_load_texture(p, step.unit),
            (true, 3) => rust::gs_load_samplerstate(p, step.unit),
            (false, 3) => oracle_gs_load_samplerstate(p, step.unit),
            (true, 4) => rust::gs_load_default_samplerstate(step.b_3d, step.unit),
            (false, 4) => oracle_gs_load_default_samplerstate(step.b_3d, step.unit),
            _ => unreachable!(),
        }
        Observed {
            calls: oracle_device_load_calls(),
            kind: oracle_device_load_kind(),
            device: oracle_device_load_device(),
            object: oracle_device_load_object(),
            unit: oracle_device_load_unit(),
            b_3d: oracle_device_load_default_3d(),
            logs: oracle_device_load_logs(),
            level: oracle_device_load_level(),
            log: CStr::from_ptr(oracle_device_load_log()).to_bytes().to_vec(),
        }
    }
}

fn compare(step: Step) {
    let original = run(false, step);
    let ported = run(true, step);
    assert_eq!(ported, original, "{step:?}");
    if step.context {
        assert_eq!((ported.calls, ported.logs), (1, 0));
        assert_eq!(ported.kind, i32::from(step.kind));
        assert_eq!(ported.device, device(step.device).cast_const());
        assert_eq!(
            ported.object,
            if step.kind == 4 {
                ptr::null()
            } else {
                object(step.object).cast_const()
            }
        );
        assert_eq!(ported.unit, if step.kind < 2 { 0 } else { step.unit });
        assert_eq!(ported.b_3d, step.kind == 4 && step.b_3d);
    } else {
        assert_eq!((ported.calls, ported.logs, ported.level), (0, 1, 400));
        let name = [
            "gs_load_vertexbuffer",
            "gs_load_indexbuffer",
            "gs_load_texture",
            "gs_load_samplerstate",
            "gs_load_default_samplerstate",
        ][usize::from(step.kind)];
        assert_eq!(
            ported.log,
            format!("{name}: called while not in a graphics context").as_bytes()
        );
    }
}

#[test]
fn characterized_matrix_matches_c() {
    for kind in 0..5 {
        for context in [false, true] {
            for device in 0..2 {
                for object in [None, Some(0), Some(1)] {
                    for unit in [0, 3, -1, i32::MIN, i32::MAX] {
                        for b_3d in [false, true] {
                            compare(Step {
                                context,
                                device,
                                object,
                                kind,
                                unit,
                                b_3d,
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
        (any::<bool>(), 0usize..2, prop::option::of(0usize..2), 0u8..5, any::<i32>(), any::<bool>()),
        1..=16,
    )) {
        for (context, device, object, kind, unit, b_3d) in steps {
            compare(Step { context, device, object, kind, unit, b_3d });
        }
    }
}
