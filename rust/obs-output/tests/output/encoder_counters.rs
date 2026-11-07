use obs_output::encoder_counters::{encoded_frames, pause_offset};

#[test]
fn mirrors_c_null_defaults() {
    let mut logs = Vec::new();
    assert_eq!(encoded_frames(None, |f, p| logs.push((f, p))), 0);
    assert_eq!(logs, [(c"obs_output_get_encoded_frames", c"encoder")]);
    assert_eq!(pause_offset(None), 0);
}

#[test]
fn mirrors_c_exact_independent_counters() {
    for frames in [0, 1, 0x8000_0000, u32::MAX] {
        for offset in [0, 1, 0x8000_0000_0000_0000, u64::MAX] {
            assert_eq!(
                encoded_frames(Some(frames), |_, _| panic!("unexpected log")),
                frames
            );
            assert_eq!(pause_offset(Some(offset)), offset);
        }
    }
}
