//! Tier 1 mirrors test_encoder_dimensions.c; callbacks expose lazy media access.
use obs_output::encoder_dimensions::{height, width};

#[test]
fn characterize_null_defaults_and_diagnostics() {
    let mut count = 0;
    assert_eq!(
        width(
            None,
            || panic!("media read"),
            |level, format| {
                assert_eq!(level, 400);
                assert_eq!(format, c"obs_encoder_get_width: Null 'encoder' parameter");
                count += 1;
            }
        ),
        0
    );
    assert_eq!(
        height(
            None,
            || panic!("media read"),
            |level, format| {
                assert_eq!(level, 400);
                assert_eq!(format, c"obs_encoder_get_height: Null 'encoder' parameter");
                count += 1;
            }
        ),
        0
    );
    assert_eq!(count, 2);
}

#[test]
fn characterize_routes_axes_and_full_width_values() {
    for kind in [i32::MIN, -1, 0, 1, 2, i32::MAX] {
        for media in [false, true] {
            for w in [0, 1, 1920, 0x8000_0000, u32::MAX] {
                for h in [0, 1, 1920, 0x8000_0000, u32::MAX] {
                    for axis in [false, true] {
                        let scaled = if axis { h } else { w };
                        let fallback = if axis { 240 } else { 320 };
                        let mut logs = 0;
                        let mut reads = 0;
                        let read = || {
                            reads += 1;
                            fallback
                        };
                        let log = |level, format| {
                            assert_eq!(level, 200);
                            assert_eq!(
                                format,
                                if axis {
                                    c"obs_encoder_get_height: encoder '%s' is not a video encoder"
                                } else {
                                    c"obs_encoder_get_width: encoder '%s' is not a video encoder"
                                }
                            );
                            logs += 1;
                        };
                        let fields = Some((kind, media, scaled));
                        let result = if axis {
                            height(fields, read, log)
                        } else {
                            width(fields, read, log)
                        };
                        assert_eq!(
                            result,
                            if kind == 1 && media {
                                if scaled != 0 { scaled } else { fallback }
                            } else {
                                0
                            }
                        );
                        assert_eq!(logs, u32::from(kind != 1));
                        assert_eq!(reads, u32::from(kind == 1 && media && scaled == 0));
                    }
                }
            }
        }
    }
}
