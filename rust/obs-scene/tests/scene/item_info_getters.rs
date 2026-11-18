//! Tier 1 port of test/cmocka/test_scene_item_info_getters.c.
use obs_scene::item_info_getters::{Conversion, TransformBits, read_info, read_vec2};

const EDGES: [u32; 10] = [
    0, 0x80000000, 0x3f800000, 0xbf800000, 0x7f800000, 0xff800000, 0x7fc01234, 0xffc05678, 1,
    0x7f7fffff,
];

fn edge(i: usize) -> u32 {
    EDGES[i % EDGES.len()]
}

#[test]
fn null_items_leave_output_untouched() {
    let sentinel = [0x7fc01234, 0x80000000];
    let mut out = sentinel;
    read_vec2(None, || panic!("NULL must not convert"), &mut out);
    assert_eq!(out, sentinel);
    assert_eq!(read_info(None, |_| panic!("NULL must not convert")), None);
}

#[test]
fn absolute_bits() {
    for i in 0..EDGES.len() {
        let stored = TransformBits {
            pos: [edge(i), edge(i + 1)],
            scale: [edge(i + 2), edge(i + 3)],
            bounds: [edge(i + 4), edge(i + 5)],
            rot: edge(i + 6),
            alignment: i as u32 * 0x01010101,
            bounds_type: i as u32 + 100,
            bounds_alignment: !(i as u32),
            crop_to_bounds: i & 1 == 0,
        };
        let mut out = [0; 2];
        read_vec2(
            Some((stored.pos, true)),
            || panic!("absolute must not convert"),
            &mut out,
        );
        assert_eq!(out, stored.pos);
        read_vec2(
            Some((stored.bounds, true)),
            || panic!("absolute must not convert"),
            &mut out,
        );
        assert_eq!(out, stored.bounds);
        assert_eq!(
            read_info(Some((stored, true)), |_| panic!(
                "absolute must not convert"
            )),
            Some(stored)
        );
    }
}

#[test]
fn relative_conversion_uses_the_c_conversions() {
    // The C conversion of the characterized plain item: pos (2^-10, 2^-11)
    // to (960.5, 540.5), bounds (0.5, 2^-10) to (270, 0.5), scale (-2, 3)
    // to (-4, 6). Every other field is copied as stored.
    let bits = |x: f32, y: f32| [x.to_bits(), y.to_bits()];
    let stored = TransformBits {
        pos: bits(0.0009765625, 0.00048828125),
        scale: bits(-2.0, 3.0),
        bounds: bits(0.5, 0.0009765625),
        rot: (-45.5f32).to_bits(),
        alignment: 5,
        bounds_type: 3,
        bounds_alignment: 10,
        crop_to_bounds: true,
    };
    let mut out = [0; 2];
    let mut calls = 0;
    read_vec2(
        Some((stored.pos, false)),
        || {
            calls += 1;
            bits(960.5, 540.5)
        },
        &mut out,
    );
    assert_eq!((calls, out), (1, bits(960.5, 540.5)));
    read_vec2(
        Some((stored.bounds, false)),
        || {
            calls += 1;
            bits(270.0, 0.5)
        },
        &mut out,
    );
    assert_eq!((calls, out), (2, bits(270.0, 0.5)));

    let mut order = Vec::new();
    let info = read_info(Some((stored, false)), |conversion| {
        order.push(conversion);
        match conversion {
            Conversion::Position => bits(960.5, 540.5),
            Conversion::Scale => bits(-4.0, 6.0),
            Conversion::Bounds => bits(270.0, 0.5),
        }
    });
    assert_eq!(
        order,
        [Conversion::Position, Conversion::Scale, Conversion::Bounds]
    );
    assert_eq!(
        info,
        Some(TransformBits {
            pos: bits(960.5, 540.5),
            scale: bits(-4.0, 6.0),
            bounds: bits(270.0, 0.5),
            ..stored
        })
    );
}
