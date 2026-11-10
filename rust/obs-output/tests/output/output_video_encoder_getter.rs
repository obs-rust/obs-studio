//! Tier 1 mirrors test_output_interleave_e2e's indexed getter assertions.
use obs_output::output_slots::MAX_OUTPUT_VIDEO_ENCODERS;
use obs_output::output_video_encoder_getter::video_encoder;

#[test]
fn test_output_interleave_e2e_empty_zero_populated_one() {
    let slots = [None, Some(17_u32), None, None, None, None];
    assert_eq!(
        video_encoder(true, 0, |i| slots[i], |_| panic!("valid")),
        None
    );
    assert_eq!(
        video_encoder(true, 1, |i| slots[i], |_| panic!("valid")),
        Some(17)
    );
}

#[test]
fn every_slot_preserves_borrowed_identity() {
    for idx in 0..MAX_OUTPUT_VIDEO_ENCODERS {
        assert_eq!(
            video_encoder(true, idx, |i| Some(i + 1), |_| panic!("valid")),
            Some(idx + 1)
        );
    }
}

#[test]
fn bounds_and_null_order_never_read_fields() {
    for idx in [
        0,
        MAX_OUTPUT_VIDEO_ENCODERS - 1,
        MAX_OUTPUT_VIDEO_ENCODERS,
        usize::MAX - 1,
        usize::MAX,
    ] {
        let mut logs = Vec::new();
        assert_eq!(
            video_encoder::<Option<usize>>(
                false,
                idx,
                |_| panic!("null read"),
                |name| logs.push(name)
            ),
            None
        );
        assert_eq!(logs, [c"obs_output_get_video_encoder2"]);
        if idx >= MAX_OUTPUT_VIDEO_ENCODERS {
            assert_eq!(
                video_encoder::<Option<usize>>(
                    true,
                    idx,
                    |_| panic!("out of bounds"),
                    |_| panic!("valid")
                ),
                None
            );
        }
    }
}
