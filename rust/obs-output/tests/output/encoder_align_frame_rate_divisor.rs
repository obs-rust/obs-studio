use obs_output::encoder_align_frame_rate_divisor::align;

#[test]
fn no_call_guards_mirrors_c_characterization() {
    for encoder in [
        None,
        Some((None, u32::MAX, (1, 7))),
        Some((Some(7), 0, (1, 7))),
        Some((Some(7), 1, (1, 7))),
    ] {
        let before = encoder;
        align(encoder, u64::MAX, |_, _, _| {
            panic!("guard must not forward")
        });
        assert_eq!(encoder, before);
    }
}

#[test]
fn registered_identity_and_forwarding_mirrors_c_characterization() {
    for media in [7, 19] {
        for divisor in [2, 3, u32::MAX] {
            for timestamp in [0, 1, 0x1234_5678_9abc_def0, u64::MAX] {
                let identity = (1, 7);
                let encoder = Some((Some(media), divisor, identity));
                let before = encoder;
                // Callback and encoder decoys each differ in just one component.
                let mut inputs = [((1, 8), 1, 99), ((2, 7), 1, 99), (identity, 1, 99)];
                let decoys = [inputs[0], inputs[1]];
                let mut calls = 0;
                let mut observed = None;
                align(
                    encoder,
                    timestamp,
                    |forwarded_media, forwarded_timestamp, forwarded_identity| {
                        calls += 1;
                        observed = Some((forwarded_media, forwarded_timestamp, forwarded_identity));
                        let input = inputs
                            .iter_mut()
                            .find(|input| input.0 == forwarded_identity)
                            .unwrap();
                        input.1 = 0;
                        input.2 = forwarded_timestamp;
                    },
                );
                assert_eq!(calls, 1);
                assert_eq!(observed, Some((media, timestamp, identity)));
                assert_eq!(inputs[2], (identity, 0, timestamp));
                assert_eq!([inputs[0], inputs[1]], decoys);
                assert_eq!(encoder, before);
            }
        }
    }
}
