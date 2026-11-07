//! Tier 1 mirrors test_encoder_gpu_scaling_getters.c.
use obs_output::encoder_gpu_scaling_getters::{enabled, scale_type};

#[test]
fn characterize_null_defaults_and_diagnostics() {
    let mut logs = 0;
    assert!(!enabled(None, |level, format| {
        assert_eq!(level, 400);
        assert_eq!(
            format,
            c"obs_encoder_gpu_scaling_enabled: Null 'encoder' parameter"
        );
        logs += 1;
    }));
    assert_eq!(
        scale_type(None, |level, format| {
            assert_eq!(level, 400);
            assert_eq!(
                format,
                c"obs_encoder_get_scale_type: Null 'encoder' parameter"
            );
            logs += 1;
        }),
        0
    );
    assert_eq!(logs, 2);
}

#[test]
fn characterize_types_and_all_scale_types() {
    for scale in 0..=5 {
        for kind in 0..=1 {
            let mut logs = 0;
            assert_eq!(
                enabled(Some((kind, scale)), |level, format| {
                    assert_eq!(level, 200);
                    assert_eq!(
                        format,
                        c"obs_encoder_gpu_scaling_enabled: encoder '%s' is not a video encoder"
                    );
                    logs += 1;
                }),
                kind == 1 && scale != 0
            );
            assert_eq!(
                scale_type(Some((kind, scale)), |level, format| {
                    assert_eq!(level, 200);
                    assert_eq!(
                        format,
                        c"obs_encoder_get_scale_type: encoder '%s' is not a video encoder"
                    );
                    logs += 1;
                }),
                if kind == 1 { scale } else { 0 }
            );
            assert_eq!(logs, if kind == 1 { 0 } else { 2 });
        }
    }
}

#[test]
fn undeclared_values_keep_the_original_integer_behavior() {
    for kind in [i32::MIN, -1, 2, i32::MAX] {
        assert_eq!(scale_type(Some((kind, 5)), |_, _| {}), 0);
    }
    for scale in [i32::MIN, -1, 6, i32::MAX] {
        assert_eq!(
            scale_type(Some((1, scale)), |_, _| panic!("unexpected log")),
            scale
        );
        assert!(enabled(Some((1, scale)), |_, _| panic!("unexpected log")));
    }
}
