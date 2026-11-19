//! Tier 3: original C query and real mutex walk, using actual private layouts.
//! NULL encoders/cores and dangling mixes are excluded as undefined in C.
use core::ffi::{c_int, c_void};
use obs_encoder::ffi::video_tex_active::obs_encoder_video_tex_active;
use obs_encoder::video_tex_active::{NV12, P010};
use proptest::prelude::*;

type Query = unsafe extern "C" fn(*const c_void, c_int) -> bool;
unsafe extern "C" {
    fn oracle_encoder_tex_format_values(values: *mut c_int);
    fn oracle_encoder_tex_check(
        candidate: Query,
        format: c_int,
        media: u8,
        mask: u8,
        videos: *const u8,
        flags: *const u8,
    ) -> bool;
}

#[test]
fn real_header_format_values() {
    let mut values = [0; 3];
    // SAFETY: C writes exactly three integers measured from the real enum.
    unsafe { oracle_encoder_tex_format_values(values.as_mut_ptr()) };
    assert_eq!(values, [NV12, P010, 25]);
}

fn compare(format: i32, media: u8, mask: u8, videos: [u8; 3], flags: [u8; 3]) {
    // SAFETY: C creates and snapshots live real-header encoder/core/mix owners,
    // initializes the real mutex, calls both implementations, then destroys it.
    // Both three-byte inputs remain live; pointers are compared, never dereferenced.
    assert!(
        unsafe {
            oracle_encoder_tex_check(
                obs_encoder_video_tex_active,
                format,
                media,
                mask,
                videos.as_ptr(),
                flags.as_ptr(),
            )
        },
        "format={format}, media={media}, mask={mask}, videos={videos:?}, flags={flags:?}"
    );
}

#[test]
fn characterized_edges_and_first_matching_mix() {
    for format in 0..=25 {
        for flags in 0..4 {
            for media in 0..4 {
                for mask in [0, 1, 2, 4, 7] {
                    compare(format, media, mask, [1, 2, 3], [flags; 3]);
                    // Duplicate NULL/non-NULL videos select the first present mix.
                    compare(
                        format,
                        media,
                        mask,
                        [media; 3],
                        [flags, flags ^ 3, flags ^ 1],
                    );
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(512), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn result_lookup_and_storage_match_original_c(
        format in prop_oneof![0i32..=25, any::<i32>()],
        media in 0u8..4, mask in 0u8..8,
        videos in prop::array::uniform3(0u8..4),
        flags in prop::array::uniform3(0u8..4),
    ) {
        compare(format, media, mask, videos, flags);
    }
}
