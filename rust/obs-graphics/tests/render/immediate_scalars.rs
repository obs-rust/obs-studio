//! Tier 1: scalar construction used by test_graphics_immediate_render.c.
use obs_graphics::immediate_scalars::{normal3f, vertex2f};
use obs_graphics::vec3::Vec3;

fn bits(v: Vec3) -> [u32; 4] {
    [v.x.to_bits(), v.y.to_bits(), v.z.to_bits(), v.w.to_bits()]
}

#[test]
fn test_graphics_immediate_render_scalar_inputs() {
    // The unchanged C lifecycle test submits this quad and this normal.
    for (x, y) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
        assert_eq!(bits(vertex2f(x, y)), [x.to_bits(), y.to_bits(), 0, 0]);
    }
    assert_eq!(bits(normal3f(0.0, 0.0, 1.0)), [0, 0, 1.0f32.to_bits(), 0]);
}

#[test]
fn exact_components_including_signed_zero_and_nonfinite() {
    let edges = [
        0, 0x80000000, 1, 0x80000001, 0x007fffff, 0x00800000, 0x3f800000, 0xbf800000, 0x7f7fffff,
        0xff7fffff, 0x7f800000, 0xff800000, 0x7fc12345, 0xffc54321, 0x7f800001,
    ];
    for x in edges {
        for y in edges {
            assert_eq!(
                bits(vertex2f(f32::from_bits(x), f32::from_bits(y))),
                [x, y, 0, 0]
            );
            for z in edges {
                assert_eq!(
                    bits(normal3f(
                        f32::from_bits(x),
                        f32::from_bits(y),
                        f32::from_bits(z)
                    )),
                    [x, y, z, 0]
                );
            }
        }
    }
}
