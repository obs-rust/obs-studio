//! Tier 3 against the original extracted C; no exclusions or behavior differences.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::output_conversion_callbacks::{
    Audio, Callback, PacketCallback, Video, obs_output_add_packet_callback,
    obs_output_remove_packet_callback, obs_output_set_audio_conversion,
    obs_output_set_video_conversion,
};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;
type VideoFn = unsafe extern "C" fn(*mut c_void, *const Video);
type AudioFn = unsafe extern "C" fn(*mut c_void, *const Audio);
type PacketFn = unsafe extern "C" fn(*mut c_void, Callback, *mut c_void);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_output_conversion_callbacks_check(
        video: VideoFn,
        audio: AudioFn,
        add: PacketFn,
        remove: PacketFn,
        get: GetLog,
        set: SetLog,
        flags: u32,
        value: u32,
        operations: u32,
    ) -> bool;
    fn oracle_output_conversion_callbacks_layout(layout: *mut usize);
}
#[test]
fn real_header_layout() {
    let mut layout = [0; 17];
    // SAFETY: oracle writes precisely 17 sizes, alignments and member offsets.
    unsafe {
        oracle_output_conversion_callbacks_layout(layout.as_mut_ptr());
    }
    assert_eq!(
        layout,
        [
            size_of::<Video>(),
            align_of::<Video>(),
            offset_of!(Video, format),
            offset_of!(Video, width),
            offset_of!(Video, height),
            offset_of!(Video, range),
            offset_of!(Video, colorspace),
            size_of::<Audio>(),
            align_of::<Audio>(),
            offset_of!(Audio, samples_per_sec),
            offset_of!(Audio, format),
            offset_of!(Audio, speakers),
            offset_of!(Audio, allow_clipping),
            size_of::<PacketCallback>(),
            align_of::<PacketCallback>(),
            offset_of!(PacketCallback, callback),
            offset_of!(PacketCallback, param)
        ]
    );
}
fn check(flags: u32, value: u32, operations: u32) {
    // SAFETY: C probe owns two real outputs, mutexes, list allocations and
    // conversion structs, and restores logging. Its test runs in isolation.
    unsafe {
        assert!(oracle_output_conversion_callbacks_check(
            obs_output_set_video_conversion,
            obs_output_set_audio_conversion,
            obs_output_add_packet_callback,
            obs_output_remove_packet_callback,
            base_get_log_handler,
            base_set_log_handler,
            flags,
            value,
            operations
        ));
    }
}
#[test]
fn differential() {
    if !super::isolated(concat!(module_path!(), "::differential")) {
        return;
    }
    for flags in [0, 1, 2, 3, 4, 7, u32::MAX] {
        for value in [0, 1, 44100, u32::MAX] {
            for ops in [0, 0xffffffff, 0xf1f0a9a8, 0x33332222] {
                check(flags, value, ops);
            }
        }
    }
    let config = ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    };
    proptest::test_runner::TestRunner::new(config)
        .run(
            &(any::<u32>(), any::<u32>(), any::<u32>()),
            |(flags, value, operations)| {
                check(flags, value, operations);
                Ok(())
            },
        )
        .unwrap();
}
