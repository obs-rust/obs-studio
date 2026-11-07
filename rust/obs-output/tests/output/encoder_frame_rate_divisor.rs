//! Tier 1 mirrors test_encoder_frame_rate_divisor.c without FFI.
use obs_output::encoder_frame_rate_divisor::get;

#[test]
fn test_encoder_frame_rate_divisor_characterize() {
    let mut logs = Vec::new();
    assert_eq!(get(None, |level, text| logs.push((level, text))), 0);
    assert_eq!(logs, [(400, c"obs_encoder_set_frame_rate_divisor: Null 'encoder' parameter")]);
    for value in [0, 1, 2, 0x8000_0000, u32::MAX] {
        for kind in [0, 1] {
            let mut logs = Vec::new();
            assert_eq!(get(Some((kind, value)), |level, text| logs.push((level, text))), if kind == 1 { value } else { 0 });
            if kind == 1 {
                assert!(logs.is_empty());
            } else {
                assert_eq!(logs, [(200, c"obs_encoder_set_frame_rate_divisor: encoder '%s' is not a video encoder")]);
            }
        }
    }
}
