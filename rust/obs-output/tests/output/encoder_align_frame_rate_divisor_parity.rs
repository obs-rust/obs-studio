//! Exact observable call and encoder-byte parity against the immutable C oracle.
use core::ffi::c_void;
use obs_output::ffi::encoder_align_frame_rate_divisor::obs_encoder_align_frame_rate_divisor_with_callback;
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_encoder_align_probe(
        present: bool,
        media: bool,
        divisor: u32,
        timestamp: u64,
        rust: unsafe extern "C" fn(
            *mut c_void,
            u64,
            Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
        ),
    ) -> bool;
}

fn check(present: bool, media: bool, divisor: u32, timestamp: u64) {
    // SAFETY: the synchronous C probe owns the real-layout encoder and media
    // marker; its adapter records rather than dereferences that media marker.
    assert!(
        unsafe {
            oracle_encoder_align_probe(
                present,
                media,
                divisor,
                timestamp,
                obs_encoder_align_frame_rate_divisor_with_callback,
            )
        },
        "present={present}, media={media}, divisor={divisor}, timestamp={timestamp}"
    );
}

#[test]
fn characterized_guards_identity_and_timestamp_edges() {
    for present in [false, true] {
        for media in [false, true] {
            for divisor in [0, 1, 2, 3, u32::MAX] {
                for timestamp in [0, 1, 0x1234_5678_9abc_def0, u64::MAX] {
                    check(present, media, divisor, timestamp);
                }
            }
        }
    }
}

#[test]
fn randomized_calls_and_unchanged_encoder_bytes() {
    let config = ProptestConfig {
        cases: obs_c_oracle::parity::cases(512),
        failure_persistence: None,
        ..ProptestConfig::default()
    };
    proptest::test_runner::TestRunner::new(config)
        .run(
            &(any::<bool>(), any::<bool>(), any::<u32>(), any::<u64>()),
            |(present, media, divisor, timestamp)| {
                check(present, media, divisor, timestamp);
                Ok(())
            },
        )
        .unwrap();
}
