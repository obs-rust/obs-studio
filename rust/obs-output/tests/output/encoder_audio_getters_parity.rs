//! Real-header layout and exact value, diagnostic, and immutability parity.
use core::ffi::{c_int, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_media_io::ffi::audio_io::{
    audio_output_close, audio_output_data, audio_output_get_sample_rate, audio_output_info,
    audio_output_open,
};
use obs_output::ffi::encoder_audio_getters::{
    AudioFields, obs_encoder_get_frame_size, obs_encoder_get_mixer_index,
    obs_encoder_get_sample_rate,
};
use proptest::prelude::*;
type Rate = unsafe extern "C" fn(*const c_void) -> u32;
type Size = unsafe extern "C" fn(*const c_void) -> usize;
unsafe extern "C" {
    fn oracle_encoder_audio_layout(layout: *mut usize);
    fn oracle_encoder_audio_new(
        kind: c_int,
        media: *mut c_void,
        rate: u32,
        framesize: usize,
        mixer: usize,
    ) -> *mut c_void;
    fn oracle_encoder_audio_unchanged(encoder: *mut c_void) -> bool;
    fn oracle_encoder_audio_free(encoder: *mut c_void);
    fn oracle_encoder_audio_probe(
        encoder: *const c_void,
        rate: Rate,
        framesize: Size,
        mixer: Size,
        media: Rate,
    ) -> bool;
}
#[test]
fn real_header_layout() {
    let mut layout = [0; 12];
    // SAFETY: the oracle writes exactly twelve size_t values.
    unsafe { oracle_encoder_audio_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<AudioFields>(),
            align_of::<AudioFields>(),
            offset_of!(AudioFields, kind),
            offset_of!(AudioFields, media),
            offset_of!(AudioFields, rate),
            offset_of!(AudioFields, frame_size),
            offset_of!(AudioFields, mixer_index),
            offset_of!(AudioFields, name),
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
        // SAFETY: the fixture is uniquely owned and freed once.
        unsafe { oracle_encoder_audio_free(self.0) };
    }
}
struct Audio(*mut c_void);
unsafe extern "C" fn input(
    _: *mut c_void,
    _: u64,
    _: u64,
    _: *mut u64,
    _: u32,
    _: *mut audio_output_data,
) -> bool {
    false
}
impl Audio {
    fn new(rate: u32) -> Self {
        let mut info = audio_output_info {
            name: c"audio-getters".as_ptr(),
            samples_per_sec: rate,
            format: 8,
            speakers: 2,
            input_callback: Some(input),
            input_param: core::ptr::null_mut(),
        };
        let mut audio = core::ptr::null_mut();
        // SAFETY: info and output are valid; static name and callback outlive the output.
        assert_eq!(unsafe { audio_output_open(&mut audio, &mut info) }, 0);
        Self(audio)
    }
}
impl Drop for Audio {
    fn drop(&mut self) {
        // SAFETY: uniquely owned live output; close stops and joins its worker.
        unsafe { audio_output_close(self.0) };
    }
}
fn probe(encoder: *const c_void) {
    // SAFETY: fixture fields are readable or encoder is null, media stays live, logging is isolated and restored.
    unsafe {
        assert!(oracle_encoder_audio_probe(
            encoder,
            obs_encoder_get_sample_rate,
            obs_encoder_get_frame_size,
            obs_encoder_get_mixer_index,
            audio_output_get_sample_rate
        ))
    };
}
fn check(kind: i32, media: *mut c_void, rate: u32, framesize: usize, mixer: usize) {
    // SAFETY: C initializes its real private layout and owns its name buffer.
    let encoder = Encoder(unsafe { oracle_encoder_audio_new(kind, media, rate, framesize, mixer) });
    assert!(!encoder.0.is_null());
    probe(encoder.0);
    // SAFETY: the exclusive fixture remains live for comparison.
    assert!(unsafe { oracle_encoder_audio_unchanged(encoder.0) });
}
#[test]
fn characterized_edges_and_null_logs() {
    if !super::isolated(concat!(
        module_path!(),
        "::characterized_edges_and_null_logs"
    )) {
        return;
    }
    probe(core::ptr::null());
    for actual_rate in [44100, 48000] {
        let audio = Audio::new(actual_rate);
        for kind in [i32::MIN, -1, 0, 1, 2, i32::MAX] {
            for media in [core::ptr::null_mut(), audio.0] {
                for rate in [0, 1, 1024, u32::MAX] {
                    for framesize in [0, 1, 1024, u32::MAX as usize, usize::MAX] {
                        for mixer in [0, 1, 1024, u32::MAX as usize, usize::MAX] {
                            check(kind, media, rate, framesize, mixer);
                        }
                    }
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
    let audio = Audio::new(48000);
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(
                prop_oneof![Just(0i32), any::<i32>()],
                any::<bool>(),
                prop_oneof![Just(0), any::<u32>()],
                any::<usize>(),
                any::<usize>(),
            ),
            |(kind, media, rate, framesize, mixer)| {
                check(
                    kind,
                    if media {
                        audio.0
                    } else {
                        core::ptr::null_mut()
                    },
                    rate,
                    framesize,
                    mixer,
                );
                Ok(())
            },
        )
        .unwrap();
}
