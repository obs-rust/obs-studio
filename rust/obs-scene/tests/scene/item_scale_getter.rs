use obs_scene::item_scale_getter::read_scale;

#[test]
fn null_leaves_output_untouched() {
    let mut out = [0x7fc01234, 0x80000000];
    read_scale(None, || panic!("NULL must not convert"), &mut out);
    assert_eq!(out, [0x7fc01234, 0x80000000]);
}

#[test]
fn characterized_stored_scale_bits() {
    let edges = [0, 0x80000000, 0x3f800000, 0xbf800000, 0x7f800000,
        0xff800000, 0x7fc01234, 0xffc05678, 1, 0x7f7fffff];
    for (index, &x) in edges.iter().enumerate() {
        let bits = [x, edges[(index + 1) % edges.len()]];
        let mut out = [0; 2];
        read_scale(Some((bits, true)), || panic!("absolute must not convert"), &mut out);
        assert_eq!(out, bits);
        // Relative groups and nested scenes keep their stored scale in C.
        read_scale(Some(([0; 2], false)), || bits, &mut out);
        assert_eq!(out, bits);
    }
}

#[test]
fn relative_scale_uses_canvas_conversion_once() {
    let mut calls = 0;
    let mut out = [0; 2];
    read_scale(Some(([(-2.0f32).to_bits(), 3.0f32.to_bits()], false)), || {
        calls += 1;
        [(-4.0f32).to_bits(), 6.0f32.to_bits()]
    }, &mut out);
    assert_eq!(calls, 1);
    assert_eq!(out, [(-4.0f32).to_bits(), 6.0f32.to_bits()]);
}
