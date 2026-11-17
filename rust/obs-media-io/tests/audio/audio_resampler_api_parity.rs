//! Tier 2 layout/NULL/output-slot contracts and Tier 3 differential wrapper
//! behavior. The original C and Rust share a deterministic FFmpeg dependency
//! double; real SIMD sample fidelity and channel-layout versions are Tier 2.
//! No intentional differences for defined inputs. NULL descriptions and
//! excessive frame counts are outside the original C contract and not generated.
use super::audio_resampler_api::KERNEL_LOCK;
use core::{
    ffi::c_void,
    mem::{align_of, offset_of, size_of},
    ptr,
};
use obs_media_io::ffi::{audio_io::resample_info, audio_resampler as r};
use proptest::prelude::*;
unsafe extern "C" {
    fn oracle_audio_resampler_create(
        dst: *const resample_info,
        src: *const resample_info,
    ) -> *mut c_void;
    fn oracle_audio_resampler_destroy(rs: *mut c_void);
    fn oracle_audio_resampler_set_compensation_error(rs: *mut c_void, error: i64);
    fn oracle_audio_resampler_resample(
        rs: *mut c_void,
        output: *mut *mut u8,
        frames: *mut u32,
        offset: *mut u64,
        input: *const *const u8,
        count: u32,
    ) -> bool;
    fn oracle_resample_info_layout(field: i32) -> usize;
    fn oracle_resampler_kernel_script(failure: i32);
    fn oracle_resampler_kernel_trace() -> u64;
    fn oracle_audio_resampler_snapshot(
        rs: *mut c_void,
    ) -> obs_media_io::audio_resampler_drift::Compensation;
    fn oracle_resampler_kernel_live() -> i32;
}
#[test]
fn resample_info_layout() {
    let actual = [
        size_of::<resample_info>(),
        align_of::<resample_info>(),
        offset_of!(resample_info, samples_per_sec),
        offset_of!(resample_info, format),
        offset_of!(resample_info, speakers),
    ];
    for (i, v) in actual.into_iter().enumerate() {
        // SAFETY: oracle returns measurements of the real public header.
        assert_eq!(v, unsafe {
            oracle_resample_info_layout(i32::try_from(i).unwrap())
        });
    }
}
#[test]
fn null_resampler_preserves_outputs() {
    let mut marker = 0;
    let mut output = [ptr::from_mut(&mut marker); 8];
    let before = output;
    let mut frames = 123;
    let mut offset = 456;
    // SAFETY: NULL handles explicitly accept no input, preserving all outputs.
    unsafe {
        r::audio_resampler_destroy(ptr::null_mut());
        r::audio_resampler_set_compensation_error(ptr::null_mut(), 1_000_000);
        assert!(!r::audio_resampler_resample(
            ptr::null_mut(),
            output.as_mut_ptr(),
            &mut frames,
            &mut offset,
            ptr::null(),
            0
        ));
    }
    assert_eq!(output, before);
    assert_eq!(frames, 123);
    assert_eq!(offset, 456);
}
#[derive(Debug, PartialEq)]
struct Packet {
    ok: bool,
    frames: u32,
    offset: u64,
    samples: Vec<Vec<u8>>,
    untouched: Vec<bool>,
    compensation: obs_media_io::audio_resampler_drift::Compensation,
    trace: u64,
}
fn width(format: i32) -> usize {
    match format {
        1 | 5 => 1,
        0 | 2 | 6 => 2,
        _ => 4,
    }
}
fn run(
    oracle: bool,
    src: resample_info,
    dst: resample_info,
    failure: i32,
    packets: &[(u32, i64)],
) -> Option<Vec<Packet>> {
    // SAFETY: lock is held by every caller across scripting, creating, and
    // destroying both resamplers. Every plane is bounded to 1024 input frames,
    // all pointers remain live through conversion, and results are copied before
    // the next call can resize them. Both handles are always destroyed.
    unsafe {
        oracle_resampler_kernel_script(failure);
        let handle = if oracle {
            oracle_audio_resampler_create(&dst, &src)
        } else {
            r::audio_resampler_create(&dst, &src).cast()
        };
        if handle.is_null() {
            assert_eq!(oracle_resampler_kernel_live(), 0);
            oracle_resampler_kernel_script(0);
            return None;
        }
        let channels = usize::try_from(obs_media_io::audio_resampler_drift::output_channels(
            src.speakers,
        ))
        .unwrap();
        let planes = if src.format >= 5 { channels } else { 1 };
        let stride = width(src.format) * if src.format >= 5 { 1 } else { channels };
        // Finite small positive values in every supported sample representation.
        let input: Vec<Vec<u8>> = (0..planes).map(|_| vec![0; 1024 * stride]).collect();
        let pointers: Vec<*const u8> = input.iter().map(|p| p.as_ptr()).collect();
        let out_channels = usize::try_from(obs_media_io::audio_resampler_drift::output_channels(
            dst.speakers,
        ))
        .unwrap();
        let out_planes = if dst.format >= 5 { out_channels } else { 1 };
        let out_stride = width(dst.format) * if dst.format >= 5 { 1 } else { out_channels };
        let mut result = Vec::new();
        for &(count, error) in packets {
            if oracle {
                oracle_audio_resampler_set_compensation_error(handle, error)
            } else {
                r::audio_resampler_set_compensation_error(handle.cast(), error)
            }
            let mut marker = 42;
            let sentinel = ptr::from_mut(&mut marker);
            let mut output = [sentinel; 8];
            let mut frames = 123;
            let mut offset = 456;
            let ok = if oracle {
                oracle_audio_resampler_resample(
                    handle,
                    output.as_mut_ptr(),
                    &mut frames,
                    &mut offset,
                    pointers.as_ptr(),
                    count,
                )
            } else {
                r::audio_resampler_resample(
                    handle.cast(),
                    output.as_mut_ptr(),
                    &mut frames,
                    &mut offset,
                    pointers.as_ptr(),
                    count,
                )
            };
            let samples = if ok {
                output[..out_planes]
                    .iter()
                    .map(|&p| {
                        if frames == 0 {
                            Vec::new()
                        } else {
                            core::slice::from_raw_parts(
                                p,
                                usize::try_from(frames).unwrap() * out_stride,
                            )
                            .to_vec()
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            };
            result.push(Packet {
                ok,
                frames,
                offset,
                samples,
                compensation: if oracle {
                    oracle_audio_resampler_snapshot(handle)
                } else {
                    r::snapshot(handle.cast())
                },
                trace: oracle_resampler_kernel_trace(),
                untouched: output.iter().map(|&p| p == sentinel).collect(),
            });
        }
        if oracle {
            oracle_audio_resampler_destroy(handle)
        } else {
            r::audio_resampler_destroy(handle.cast())
        }
        assert_eq!(oracle_resampler_kernel_live(), 0);
        oracle_resampler_kernel_script(0);
        Some(result)
    }
}
fn compare(src: resample_info, dst: resample_info, failure: i32, packets: &[(u32, i64)]) {
    let _lock = KERNEL_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(
        run(false, src, dst, failure, packets),
        run(true, src, dst, failure, packets)
    );
}
#[test]
fn errors_compensation_and_output_slots_match() {
    for failure in 0..=6 {
        for (rate, fmt, layout) in [(48000, 4, 2), (24000, 8, 2), (48000, 0, 1), (0, 4, 1)] {
            compare(
                resample_info {
                    samples_per_sec: 48000,
                    format: 4,
                    speakers: 1,
                },
                resample_info {
                    samples_per_sec: rate,
                    format: fmt,
                    speakers: layout,
                },
                failure,
                &[(16, 0), (1024, 20_000_000), (32, -20_000_000), (1024, 0)],
            );
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(128)))]
    #[test]
    fn wrapper_matches_original(src_rate in prop_oneof![Just(24000u32),Just(44100),Just(48000)], dst_rate in prop_oneof![Just(24000u32),Just(44100),Just(48000)], src_format in 0i32..=8,dst_format in 0i32..=8,src_layout in prop_oneof![Just(1i32),Just(2),Just(3),Just(4),Just(5),Just(6),Just(8)], dst_layout in prop_oneof![Just(1i32),Just(2),Just(3),Just(4),Just(5),Just(6),Just(8)], packets in prop::collection::vec((1u32..=1024,-20_000_000i64..=20_000_000),1..8)) {
        compare(resample_info {samples_per_sec:src_rate,format:src_format,speakers:src_layout},resample_info {samples_per_sec:dst_rate,format:dst_format,speakers:dst_layout},0,&packets);
    }
}
