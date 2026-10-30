//! #3003: one audio output tick against the C, for the `audio_mix`
//! category of #3009 (the mix add itself is `audio.rs`). The output clamp
//! of `libobs/media-io/audio-io.c.h` (`clamp_audio_output`, to [-1, 1] with
//! NaN to 0) is a step of the tick, not an entry point, so it is measured
//! here inside `audio_tick()`: both sides open without a worker thread,
//! with one output on each of the 6 mixes, and the input callback fills
//! every mix with the same stereo tick of 1024 frames, a tone with samples
//! above 1, below -1 and NaN. One call is one tick: the input callback, the
//! clamp of 6 mixes and the 6 output callbacks. Before timing, the clamped
//! samples of both sides are compared bit for bit.

use std::ffi::c_void;
use std::hint::black_box;
use std::ptr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use criterion::Criterion;
use obs_c_oracle::audio_io as c;
use obs_media_io::audio_io::AudioOutput;
use obs_media_io::ffi::audio_io as rs;

const FRAMES: usize = 1024;
const MIXES: usize = 6;
const CHANNELS: usize = 2;
/// `AUDIO_FORMAT_FLOAT_PLANAR`, the format libobs runs its audio output in.
const FORMAT_FLOAT_PLANAR: i32 = 8;
const SPEAKERS_STEREO: i32 = 2;
/// The tick's start; the end is one tick of 1024 frames at 48 kHz later.
const START: u64 = 1_000_000_000;
const END: u64 = START + 21_333_333;

/// One tick of one channel: a tone that clips, with a NaN every 97 frames.
static TICK: std::sync::LazyLock<Vec<f32>> = std::sync::LazyLock::new(|| {
    (0..FRAMES)
        .map(|i| {
            if i % 97 == 13 {
                f32::NAN
            } else {
                1.4 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 48_000.0).sin()
            }
        })
        .collect()
});

/// Set only for the fixture check: the output callbacks copy mix 0, ch 0.
static CAPTURE: AtomicBool = AtomicBool::new(false);
static CAPTURED: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// Fills every active mix's channels with [`TICK`].
///
/// # Safety
///
/// `data` holds a channel pointer of at least FRAMES floats per channel.
unsafe fn fill(active: u32, data: impl Fn(usize) -> [*mut f32; 8]) {
    for mix in 0..MIXES {
        if active & (1 << mix) == 0 {
            continue;
        }
        let channels = data(mix);
        for channel in channels
            .iter()
            .take(CHANNELS)
            .filter(|channel| !channel.is_null())
        {
            // SAFETY: the caller guarantees FRAMES floats per channel.
            unsafe { ptr::copy_nonoverlapping(TICK.as_ptr(), *channel, FRAMES) };
        }
    }
}

unsafe extern "C" fn input_rs(
    _param: *mut c_void,
    _start: u64,
    end: u64,
    new_ts: *mut u64,
    active: u32,
    mixes: *mut rs::audio_output_data,
) -> bool {
    // SAFETY: the tick passes MAX_AUDIO_MIXES mixes of tick-sized planes.
    unsafe {
        *new_ts = end;
        fill(active, |mix| (*mixes.add(mix)).data);
    }
    true
}

unsafe extern "C" fn input_c(
    _param: *mut c_void,
    _start: u64,
    end: u64,
    new_ts: *mut u64,
    active: u32,
    mixes: *mut c::OracleAudioOutputData,
) -> bool {
    // SAFETY: as in `input_rs`.
    unsafe {
        *new_ts = end;
        fill(active, |mix| (*mixes.add(mix)).data);
    }
    true
}

/// Copies mix 0, channel 0 while [`CAPTURE`] is set.
fn capture(mix_idx: usize, frames: u32, plane: *const u8) {
    if mix_idx == 0 && CAPTURE.load(Ordering::Relaxed) {
        // SAFETY: the output's plane 0 holds `frames` floats.
        let samples = unsafe { std::slice::from_raw_parts(plane.cast::<f32>(), frames as usize) };
        *CAPTURED.lock().unwrap() = samples.iter().map(|s| s.to_bits()).collect();
    }
}

unsafe extern "C" fn output_rs(_param: *mut c_void, mix_idx: usize, data: *mut rs::audio_data) {
    // SAFETY: the tick passes a valid `audio_data`.
    let data = unsafe { &*data };
    capture(mix_idx, data.frames, data.data[0]);
}

unsafe extern "C" fn output_c(_param: *mut c_void, mix_idx: usize, data: *mut c::OracleAudioData) {
    // SAFETY: the tick passes a valid `audio_data`.
    let data = unsafe { &*data };
    capture(mix_idx, data.frames, data.data[0]);
}

/// Clamped mix 0, channel 0 of one tick.
fn captured(tick: impl FnOnce()) -> Vec<u32> {
    CAPTURE.store(true, Ordering::Relaxed);
    tick();
    CAPTURE.store(false, Ordering::Relaxed);
    std::mem::take(&mut *CAPTURED.lock().unwrap())
}

pub fn benchmarks(criterion: &mut Criterion) {
    // SAFETY (all calls): both outputs are opened without a worker thread,
    // so this thread is the only one that ticks them; every pointer passed
    // is valid for the call.
    unsafe {
        let mut info_c = c::OracleAudioOutputInfo {
            name: c"bench".as_ptr(),
            samples_per_sec: 48_000,
            format: FORMAT_FLOAT_PLANAR,
            speakers: SPEAKERS_STEREO,
            input_callback: Some(input_c),
            input_param: ptr::null_mut(),
        };
        let mut co: *mut c::OracleAudioOutput = ptr::null_mut();
        assert_eq!(
            c::oracle_audio_output_open_no_thread(&mut co, &mut info_c),
            0
        );
        let info_rs = rs::audio_output_info {
            name: c"bench".as_ptr(),
            samples_per_sec: 48_000,
            format: FORMAT_FLOAT_PLANAR,
            speakers: SPEAKERS_STEREO,
            input_callback: Some(input_rs),
            input_param: ptr::null_mut(),
        };
        let ro =
            Box::into_raw(AudioOutput::open_no_worker(&info_rs).expect("open the Rust output"));
        for mix in 0..MIXES {
            assert!(c::oracle_audio_output_connect(
                co,
                mix,
                ptr::null(),
                Some(output_c),
                ptr::null_mut()
            ));
            assert!(rs::audio_output_connect(
                ro.cast(),
                mix,
                ptr::null(),
                Some(output_rs),
                ptr::null_mut()
            ));
        }

        let rust = captured(|| (*ro).tick_for_test(END, START));
        let original = captured(|| c::oracle_audio_tick(co, END, START));
        assert!(!rust.is_empty(), "the tick reached the outputs");
        assert_eq!(
            rust, original,
            "benchmark fixture differs: audio_mix/output_tick"
        );
        super::measure_pair(
            criterion,
            "audio_mix/output_tick/stereo_6_mixes",
            || (*ro).tick_for_test(black_box(END), START),
            || c::oracle_audio_tick(co, black_box(END), START),
        );

        for mix in 0..MIXES {
            c::oracle_audio_output_disconnect(co, mix, Some(output_c), ptr::null_mut());
            rs::audio_output_disconnect(ro.cast(), mix, Some(output_rs), ptr::null_mut());
        }
        c::oracle_audio_output_close_no_thread(co);
        rs::audio_output_close(ro.cast());
    }
}
