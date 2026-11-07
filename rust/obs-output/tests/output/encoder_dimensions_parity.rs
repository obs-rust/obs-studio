//! Real-header layout, exact values/diagnostics and whole-encoder immutability.
use core::ffi::{c_int, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_media_io::ffi::video_io::{
    video_output_close, video_output_get_height, video_output_get_width, video_output_info,
    video_output_open,
};
use obs_output::ffi::encoder_dimensions::{
    DimensionFields, obs_encoder_get_height, obs_encoder_get_width,
};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const c_void) -> u32;
unsafe extern "C" {
    fn oracle_encoder_dimensions_layout(layout: *mut usize);
    fn oracle_encoder_dimensions_new(
        kind: c_int,
        media: *mut c_void,
        width: u32,
        height: u32,
    ) -> *mut c_void;
    fn oracle_encoder_dimensions_unchanged(encoder: *mut c_void) -> bool;
    fn oracle_encoder_dimensions_free(encoder: *mut c_void);
    fn oracle_encoder_dimensions_probe(
        encoder: *const c_void,
        width: Getter,
        height: Getter,
        media_width: Getter,
        media_height: Getter,
    ) -> bool;
}
#[test]
fn real_header_layout() {
    let mut layout = [0; 11];
    // SAFETY: the oracle writes exactly eleven size_t values.
    unsafe { oracle_encoder_dimensions_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<DimensionFields>(),
            align_of::<DimensionFields>(),
            offset_of!(DimensionFields, kind),
            offset_of!(DimensionFields, media),
            offset_of!(DimensionFields, width),
            offset_of!(DimensionFields, height),
            offset_of!(DimensionFields, name),
            size_of::<c_int>(),
            align_of::<c_int>(),
            0,
            1
        ]
    );
}
struct Encoder(*mut c_void);
impl Drop for Encoder {
    fn drop(&mut self) {
        // SAFETY: this wrapper owns the allocated C fixture, freed once.
        unsafe { oracle_encoder_dimensions_free(self.0) };
    }
}
struct Video(*mut c_void);
impl Video {
    fn new() -> Self {
        let mut info = video_output_info {
            name: c"dimensions".as_ptr(),
            format: 6,
            fps_num: 30,
            fps_den: 1,
            width: 320,
            height: 240,
            cache_size: 0,
            colorspace: 0,
            range: 0,
        };
        let mut video = core::ptr::null_mut();
        // SAFETY: output and info are writable; the static name outlives the video.
        assert_eq!(unsafe { video_output_open(&mut video, &mut info) }, 0);
        Self(video)
    }
}
impl Drop for Video {
    fn drop(&mut self) {
        // SAFETY: uniquely owned live output; close stops and joins its worker before freeing.
        unsafe { video_output_close(self.0) };
    }
}
fn check(kind: i32, media: *mut c_void, width: u32, height: u32) {
    // SAFETY: C initializes its own real private encoder layout and owns its name buffer.
    let encoder = Encoder(unsafe { oracle_encoder_dimensions_new(kind, media, width, height) });
    assert!(!encoder.0.is_null());
    // SAFETY: live exclusive encoder and media, and a probe that restores logging in an isolated child.
    unsafe {
        assert!(oracle_encoder_dimensions_probe(
            encoder.0,
            obs_encoder_get_width,
            obs_encoder_get_height,
            video_output_get_width,
            video_output_get_height
        ));
        assert!(oracle_encoder_dimensions_unchanged(encoder.0));
    }
}
#[test]
fn characterized_edges_and_null_logs() {
    if !super::isolated(concat!(
        module_path!(),
        "::characterized_edges_and_null_logs"
    )) {
        return;
    }
    let video = Video::new();
    // SAFETY: callbacks accept NULL; the isolated probe restores logging.
    unsafe {
        assert!(oracle_encoder_dimensions_probe(
            core::ptr::null(),
            obs_encoder_get_width,
            obs_encoder_get_height,
            video_output_get_width,
            video_output_get_height
        ));
    }
    for kind in [i32::MIN, -1, 0, 1, 2, i32::MAX] {
        for media in [core::ptr::null_mut(), video.0] {
            for width in [0, 1, 1920, 0x8000_0000, u32::MAX] {
                for height in [0, 1, 1920, 0x8000_0000, u32::MAX] {
                    check(kind, media, width, height);
                }
            }
        }
    }
}
#[test]
fn random_fields_match_c() {
    if !super::isolated(concat!(module_path!(), "::random_fields_match_c")) {
        return;
    }
    let video = Video::new();
    let config = ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(config);
    runner
        .run(
            &(
                prop_oneof![Just(1i32), any::<i32>()],
                any::<bool>(),
                prop_oneof![Just(0), any::<u32>()],
                prop_oneof![Just(0), any::<u32>()],
            ),
            |(kind, media, width, height)| {
                check(
                    kind,
                    if media {
                        video.0
                    } else {
                        core::ptr::null_mut()
                    },
                    width,
                    height,
                );
                Ok(())
            },
        )
        .unwrap();
}
