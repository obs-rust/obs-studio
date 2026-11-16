//! Tier 1: construction assertions from test_immediate_arguments.c.
//! Downstream context diagnostics, appends, routing and limits remain covered
//! through the unchanged public C test against both libraries (Tier 2).
use obs_graphics::{immediate_arguments as core, vec2::Vec2, vec3::Vec3};

const COMPONENTS: [[u32; 3]; 6] = [
    [0x00000000, 0x80000000, 0x3f800000],
    [0x80000000, 0x00000000, 0xbf800000],
    [0x7f800000, 0xff800000, 0x7fc12345],
    [0xffc54321, 0x7fc12345, 0x80000000],
    [0x00000001, 0x80000001, 0x7f7fffff],
    [0xff7fffff, 0x3fa00000, 0xc0200000],
];

fn bits(v: Vec3) -> [u32; 4] {
    [v.x.to_bits(), v.y.to_bits(), v.z.to_bits(), v.w.to_bits()]
}

#[test]
fn test_immediate_arguments_argument_forwarding_vertex3f() {
    for [x, y, z] in COMPONENTS {
        assert_eq!(
            bits(core::vertex3f(
                f32::from_bits(x),
                f32::from_bits(y),
                f32::from_bits(z)
            )),
            [x, y, z, 0]
        );
    }
}

#[test]
fn test_immediate_arguments_argument_forwarding_vertex2v() {
    for [x, y, _] in COMPONENTS {
        let input = Vec2::new(f32::from_bits(x), f32::from_bits(y));
        assert_eq!(bits(core::vertex2v(input)), [x, y, 0, 0]);
        assert_eq!([input.x.to_bits(), input.y.to_bits()], [x, y]);
    }
    // The immediate_limits case alternates these inputs up to the existing cap.
    assert_eq!(
        bits(core::vertex2v(Vec2::new(-0.0, 2.5))),
        [0x80000000, 0x40200000, 0, 0]
    );
    assert_eq!(
        bits(core::vertex3f(-0.0, 2.5, -0.0)),
        [0x80000000, 0x40200000, 0x80000000, 0]
    );
}

#[test]
fn test_immediate_arguments_exact_downstream_calls_texcoord() {
    let units = [i32::MIN, -65537, -1, 0, 1, 7, 15, 16, 65537, i32::MAX];
    for [x, y, _] in COMPONENTS {
        for unit in units.into_iter().chain((0..16).rev()) {
            let (value, forwarded_unit) =
                core::texcoord(f32::from_bits(x), f32::from_bits(y), unit);
            assert_eq!([value.x.to_bits(), value.y.to_bits()], [x, y]);
            assert_eq!(forwarded_unit, unit);
        }
    }
}
