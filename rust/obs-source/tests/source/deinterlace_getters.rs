use obs_source::deinterlace_getters::{field_order, mode};

#[test]
fn mirrors_c_null_defaults() {
    let mut logs = Vec::new();
    assert_eq!(mode(None, |f, p| logs.push((f, p))), 0);
    assert_eq!(field_order(None, |f, p| logs.push((f, p))), 0);
    assert_eq!(
        logs,
        [
            (c"obs_source_set_deinterlace_mode", c"source"),
            (c"obs_source_set_deinterlace_field_order", c"source"),
        ]
    );
}

#[test]
fn mirrors_c_independent_fields() {
    for value in 0..=8 {
        for top in [false, true] {
            assert_eq!(mode(Some(value), |_, _| panic!("unexpected log")), value);
            assert_eq!(
                field_order(Some(top), |_, _| panic!("unexpected log")),
                u32::from(!top)
            );
        }
    }
}

#[test]
fn preserves_mode_bits() {
    for value in [0, 9, 0x7fff_ffff, 0x8000_0000, u32::MAX] {
        assert_eq!(mode(Some(value), |_, _| panic!("unexpected log")), value);
    }
}
