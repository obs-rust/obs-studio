use obs_output::encoder_align_frame_rate_divisor::align;

#[test]
fn no_call_guards_mirrors_c_characterization() {
    for encoder in [
        None,
        Some((None, u32::MAX)),
        Some((Some(7), 0)),
        Some((Some(7), 1)),
    ] {
        align(encoder, u64::MAX, |_, _| panic!("guard must not forward"));
    }
}

#[test]
fn registered_identity_and_forwarding_mirrors_c_characterization() {
    for media in [7, 19] {
        for divisor in [2, 3, u32::MAX] {
            for timestamp in [0, 1, 0x1234_5678_9abc_def0, u64::MAX] {
                let encoder = Some((Some(media), divisor));
                let before = encoder;
                let mut calls = Vec::new();
                align(encoder, timestamp, |media, timestamp| {
                    calls.push((media, timestamp))
                });
                assert_eq!(calls, [(media, timestamp)]);
                assert_eq!(encoder, before);
            }
        }
    }
}
