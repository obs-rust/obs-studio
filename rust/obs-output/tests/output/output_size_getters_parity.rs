//! Tier 3: original C getters, real private layouts, exact diagnostics and state.
use core::ffi::c_void;
use obs_media_io::ffi::video_io::{video_output_close, video_output_info, video_output_open};
use obs_output::ffi::output_size_getters::{
    obs_output_get_height, obs_output_get_height2, obs_output_get_width, obs_output_get_width2,
};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;
type Indexed = unsafe extern "C" fn(*const c_void, usize) -> u32;
type DefaultGetter = unsafe extern "C" fn(*const c_void) -> u32;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_output_size_new(
        flags: u32,
        video: *mut c_void,
        media: bool,
        width: u32,
        height: u32,
        slot: usize,
        present: u32,
    ) -> *mut c_void;
    fn oracle_output_size_free(output: *mut c_void);
    fn oracle_output_size_probe(
        output: *mut c_void,
        idx: usize,
        height: bool,
        indexed: bool,
        getter: Indexed,
        wrapper: DefaultGetter,
        get: GetLog,
        set: SetLog,
    ) -> bool;
}
struct Video(*mut c_void);
impl Video {
    fn new() -> Self {
        let mut info = video_output_info {
            name: c"output-size".as_ptr(),
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
        // SAFETY: writable output/info; name is static.
        assert_eq!(unsafe { video_output_open(&mut video, &mut info) }, 0);
        Self(video)
    }
}
impl Drop for Video {
    fn drop(&mut self) {
        // SAFETY: uniquely owned video; close joins its worker before freeing.
        unsafe { video_output_close(self.0) }
    }
}
struct Output(*mut c_void);
impl Drop for Output {
    fn drop(&mut self) {
        // SAFETY: uniquely owned oracle fixture, which only borrows the video.
        unsafe { oracle_output_size_free(self.0) }
    }
}
fn probe(output: *mut c_void, idx: usize) {
    // SAFETY: live exclusive fixture or NULL; isolated child owns logging and the probe restores it.
    unsafe {
        for (height, getter, wrapper) in [
            (
                false,
                obs_output_get_width2 as Indexed,
                obs_output_get_width as DefaultGetter,
            ),
            (
                true,
                obs_output_get_height2 as Indexed,
                obs_output_get_height as DefaultGetter,
            ),
        ] {
            for indexed in [false, true] {
                assert!(
                    oracle_output_size_probe(
                        output,
                        idx,
                        height,
                        indexed,
                        getter,
                        wrapper,
                        base_get_log_handler,
                        base_set_log_handler
                    ),
                    "idx={idx}, height={height}, indexed={indexed}"
                );
            }
        }
    }
}
fn check(
    video: &Video,
    flags: u32,
    media: bool,
    dims: [u32; 2],
    slot: usize,
    present: u32,
    idx: usize,
) {
    // SAFETY: oracle allocates and initializes the real private output/encoder layouts.
    let output = Output(unsafe {
        oracle_output_size_new(flags, video.0, media, dims[0], dims[1], slot, present)
    });
    assert!(!output.0.is_null());
    probe(output.0, idx);
}
#[test]
fn characterized_edges() {
    if !super::isolated(concat!(module_path!(), "::characterized_edges")) {
        return;
    }
    let video = Video::new();
    for idx in [0, 9, 10, usize::MAX] {
        probe(core::ptr::null_mut(), idx);
    }
    for flags in [2, 6, 3, 7] {
        for media in [false, true] {
            for width in [0, 1, 1920, u32::MAX] {
                for height in [0, 1, 1920, u32::MAX] {
                    for slot in 0..10 {
                        for present in 0..4 {
                            check(&video, flags, media, [width, height], slot, present, slot);
                        }
                    }
                }
            }
        }
        for idx in [10, 11, usize::MAX / 2, usize::MAX] {
            check(&video, flags, true, [123, 456], 0, 1, idx);
        }
    }
}
#[test]
fn randomized_dimensions_match_original_c() {
    if !super::isolated(concat!(
        module_path!(),
        "::randomized_dimensions_match_original_c"
    )) {
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
                any::<u32>(),
                any::<bool>(),
                prop_oneof![Just(0), any::<u32>()],
                prop_oneof![Just(0), any::<u32>()],
                0usize..10,
                0u32..4,
                prop_oneof![0usize..12, any::<usize>()],
            ),
            |(flags, media, width, height, slot, present, idx)| {
                check(&video, flags, media, [width, height], slot, present, idx);
                Ok(())
            },
        )
        .unwrap();
}
