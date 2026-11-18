//! Tier 3: actual C context/vtable layout, original bodies and real adapters.
//! No intentional differences or excluded inputs; texture tokens are identity-only.
use obs_c_oracle as _;
use obs_graphics::ffi::texture_copy as rust;
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr;

unsafe extern "C" {
    fn oracle_gs_copy_texture(dst: *mut c_void, src: *mut c_void);
    fn oracle_gs_copy_texture_region(
        dst: *mut c_void,
        dst_x: u32,
        dst_y: u32,
        src: *mut c_void,
        src_x: u32,
        src_y: u32,
        src_w: u32,
        src_h: u32,
    );
    fn oracle_texture_copy_reset(context: bool);
    fn oracle_texture_copy_calls() -> u32;
    fn oracle_texture_copy_logs() -> u32;
    fn oracle_texture_copy_kind() -> c_int;
    fn oracle_texture_copy_level() -> c_int;
    fn oracle_texture_copy_device() -> *const c_void;
    fn oracle_texture_copy_expected_device() -> *const c_void;
    fn oracle_texture_copy_dst() -> *const c_void;
    fn oracle_texture_copy_src() -> *const c_void;
    fn oracle_texture_copy_value(index: u32) -> u32;
    fn oracle_texture_copy_log() -> *const c_char;
}

static TOKENS: [u8; 2] = [0; 2];

#[derive(Clone, Copy, Debug)]
struct Step {
    context: bool,
    dst: Option<usize>,
    src: Option<usize>,
    region: bool,
    values: [u32; 6],
}

#[derive(Debug, PartialEq)]
struct Observed {
    calls: u32,
    kind: i32,
    device: *const c_void,
    dst: *const c_void,
    src: *const c_void,
    values: [u32; 6],
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
    // SAFETY: context and recorder are thread-local; live static tokens are
    // compared only by identity. Both paths use the real recording vtable.
    unsafe {
        oracle_texture_copy_reset(step.context);
        let dst = token(step.dst);
        let src = token(step.src);
        let v = step.values;
        match (use_rust, step.region) {
            (true, false) => rust::gs_copy_texture(dst, src),
            (false, false) => oracle_gs_copy_texture(dst, src),
            (true, true) => {
                rust::gs_copy_texture_region(dst, v[0], v[1], src, v[2], v[3], v[4], v[5])
            }
            (false, true) => {
                oracle_gs_copy_texture_region(dst, v[0], v[1], src, v[2], v[3], v[4], v[5])
            }
        }
        Observed {
            calls: oracle_texture_copy_calls(),
            kind: oracle_texture_copy_kind(),
            device: oracle_texture_copy_device(),
            dst: oracle_texture_copy_dst(),
            src: oracle_texture_copy_src(),
            values: std::array::from_fn(|i| oracle_texture_copy_value(i as u32)),
            logs: oracle_texture_copy_logs(),
            level: oracle_texture_copy_level(),
            log: CStr::from_ptr(oracle_texture_copy_log())
                .to_bytes()
                .to_vec(),
        }
    }
}

fn compare(step: Step) {
    let original = run(false, step);
    let ported = run(true, step);
    assert_eq!(ported, original, "{step:?}");
    if step.context && step.dst.is_some() && (step.region || step.src.is_some()) {
        assert_eq!((ported.calls, ported.logs), (1, 0));
        assert_eq!(ported.kind, i32::from(step.region));
        // SAFETY: returns the address of the current thread's live device token.
        assert_eq!(ported.device, unsafe {
            oracle_texture_copy_expected_device()
        });
        assert_eq!(ported.dst, token(step.dst).cast_const());
        assert_eq!(ported.src, token(step.src).cast_const());
        assert_eq!(
            ported.values,
            if step.region { step.values } else { [0; 6] }
        );
    } else {
        assert_eq!((ported.calls, ported.logs, ported.level), (0, 1, 400));
        let reason = if !step.context {
            "called while not in a graphics context"
        } else if step.dst.is_none() {
            "Null 'dst' parameter"
        } else {
            "Null 'src' parameter"
        };
        let name = if step.region {
            "gs_copy_texture_region"
        } else {
            "gs_copy_texture"
        };
        assert_eq!(ported.log, format!("{name}: {reason}").as_bytes());
    }
}

#[test]
fn characterized_matrix_matches_c() {
    let mut values = vec![[11, 23, 37, 41, 53, 67], [0; 6], [u32::MAX; 6]];
    for i in 0..6 {
        let mut v = [1, 2, 3, 4, 5, 6];
        v[i] = u32::MAX;
        values.push(v);
    }
    for region in [false, true] {
        for context in [false, true] {
            for dst in [None, Some(0), Some(1)] {
                for src in [None, Some(0), Some(1)] {
                    for &v in &values {
                        compare(Step {
                            context,
                            dst,
                            src,
                            region,
                            values: v,
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
        (any::<bool>(), prop::option::of(0usize..2), prop::option::of(0usize..2), any::<bool>(), any::<[u32; 6]>()),
        1..=16,
    )) {
        for (context, dst, src, region, values) in steps {
            compare(Step { context, dst, src, region, values });
        }
    }
}
