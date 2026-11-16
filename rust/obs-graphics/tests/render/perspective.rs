//! Tier 1: the safe `gs_perspective` core against a model context and a
//! recording device, mirroring `test/cmocka/test_gs_perspective.c`: without
//! a context one debug line under the function's name and no device call;
//! inside one, exactly one `device_frustum` on the current device per call,
//! its six arguments bit-exact to the C's float expression, and no log.
use obs_graphics::perspective::{self, Frustum, Host, NOT_IN_CONTEXT};
use std::ffi::CStr;

#[derive(Default)]
pub(super) struct Fixture {
    pub device: Option<usize>,
    pub events: Vec<(usize, [u32; 6])>,
    pub logs: Vec<String>,
}

impl Host for Fixture {
    fn has_context(&self) -> bool {
        self.device.is_some()
    }
    fn debug(&mut self, function: &'static CStr) {
        self.logs
            .push(format!("{}: {NOT_IN_CONTEXT}", function.to_str().unwrap()));
    }
    fn frustum(&mut self, frustum: Frustum) {
        self.events.push((
            self.device.expect("no device call without a context"),
            frustum.bits(),
        ));
    }
}

/// The C characterization's cases: angle, aspect, near, far.
pub(super) const CASES: [[f32; 4]; 10] = [
    [60.0, 16.0 / 9.0, 0.1, 1000.0],
    [45.0, 4.0 / 3.0, 1.0, 100.0],
    [93.125, 0.731_25, 0.031_25, 123.456],
    [0.001, 2.125, 0.0001, 100_000.0],
    [179.0, 1.0, 2.0, 3.0],
    // Characterized, not endorsed: no range checks, even on the clips.
    [-35.5, -1.25, -0.75, -20.0],
    [80.0, -0.0, 10.0, 1.0],
    [0.0, 1.0, 1.0, -0.0],
    [-0.0, 1.0, 1.0, 0.0],
    [72.0, 1.5, -0.0, 0.0],
];

/// The test's own copy of the C expression, in the C's float order:
/// `ymax = near * tanf(RAD(angle) * 0.5f)`, with `RAD`'s float constant.
#[allow(
    clippy::excessive_precision,
    reason = "math-defs.h's RAD literal, verbatim; it rounds to the same f32"
)]
fn expected([angle, aspect, near, far]: [f32; 4]) -> [u32; 6] {
    let ymax = near
        * ((std::hint::black_box(angle) * 0.017_453_292_519_943_295_769_236_907_684_8_f32) * 0.5)
            .tan();
    let ymin = -ymax;
    [ymin * aspect, ymax * aspect, ymin, ymax, near, far].map(f32::to_bits)
}

#[test]
fn test_gs_perspective_contract() {
    let missing = format!("gs_perspective: {NOT_IN_CONTEXT}");
    let mut host = Fixture::default();
    perspective::perspective(&mut host, 60.0, 1.5, 0.1, 100.0);
    assert!(host.events.is_empty());
    assert_eq!(host.logs, std::slice::from_ref(&missing));
    for device in 0..2 {
        host.device = Some(device);
        for case in CASES {
            let before = host.events.len();
            perspective::perspective(&mut host, case[0], case[1], case[2], case[3]);
            assert_eq!(host.events.len(), before + 1);
            assert_eq!(host.events.last(), Some(&(device, expected(case))));
            assert_eq!(host.logs.len(), 1);
        }
        host.device = None;
    }
    let before = host.events.len();
    perspective::perspective(&mut host, 45.0, 1.0, 1.0, 10.0);
    assert_eq!(host.events.len(), before);
    assert_eq!(host.logs, [missing.clone(), missing]);
}

#[test]
fn test_gs_perspective_signed_zeros() {
    // tanf(0) is +0: ymax = +0, ymin = -0; the far clip passes unchanged.
    let f = perspective::frustum(0.0, 1.0, 1.0, -0.0);
    assert_eq!(
        f.bits(),
        [-0.0f32, 0.0, -0.0, 0.0, 1.0, -0.0].map(f32::to_bits)
    );
    // A -0 near flips every sign: ymax = -0, ymin = +0.
    let f = perspective::frustum(72.0, 1.5, -0.0, 0.0);
    assert_eq!(
        f.bits(),
        [0.0f32, -0.0, 0.0, -0.0, -0.0, 0.0].map(f32::to_bits)
    );
    // A -0 aspect zeroes the x extent with the sign of the product.
    let f = perspective::frustum(80.0, -0.0, 10.0, 1.0);
    assert_eq!(f.left.to_bits(), 0.0f32.to_bits());
    assert_eq!(f.right.to_bits(), (-0.0f32).to_bits());
    assert_eq!((f.top, f.bottom), (-f.bottom, f.bottom));
}

#[test]
fn test_gs_perspective_frustum_matches_expression() {
    for case in CASES {
        assert_eq!(
            perspective::frustum(case[0], case[1], case[2], case[3]).bits(),
            expected(case)
        );
    }
}
