use obs_output::encoder_scaling_enabled::get;

#[test]
fn mirrors_c_null_default_and_diagnostic() {
    let mut logs = Vec::new();
    assert!(!get(None, |f, p| logs.push((f, p))));
    assert_eq!(logs, [(c"obs_encoder_scaling_enabled", c"encoder")]);
}

#[test]
fn mirrors_c_either_axis_without_mutation() {
    for width in [0, 1, 1920, 0x8000_0000, u32::MAX] {
        for height in [0, 1, 1920, 0x8000_0000, u32::MAX] {
            // Both C encoder kinds share the same safe input: type is not consulted.
            for _kind in [0, 1] {
                let fields = (width, height);
                assert_eq!(
                    get(Some(fields), |_, _| panic!("unexpected log")),
                    width != 0 || height != 0
                );
                assert_eq!(fields, (width, height));
            }
        }
    }
}
