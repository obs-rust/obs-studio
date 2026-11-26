//! Tier 1 for the `audio-io.c` port: the open/connect/disconnect/getter
//! surface and a live audio-thread tick, mirroring the pins in
//! `test/cmocka/test_audio_io.c`. The ported thread runs for real here:
//! `os_*` timing resolves to the host clock in obs-util and the oracle's
//! threading build.
//!
//! The C test's crash-only pins (NULL-handle getter faults, thread running
//! without an obs core) are exercised through cmocka under ENABLE_RUST_LIBOBS
//! ON/OFF; they are not reproducible inside a Rust test binary.
//!
use std::ffi::{c_char, c_void};
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use obs_media_io::ffi::audio_io::*;

// Links the oracle staticlib (real os_*/pthread implementations plus the
// profiler/resampler stubs the ported thread calls) and the obs-util port
// (real bmalloc/bfree/bmemdup/base_get_log_handler they reference).
use obs_c_oracle as _;
use obs_util as _;

const AUDIO_FORMAT_FLOAT: i32 = 4;
const AUDIO_FORMAT_FLOAT_PLANAR: i32 = 8;

const SPEAKERS_UNKNOWN: i32 = 0;
const SPEAKERS_STEREO: i32 = 2;
const SPEAKERS_5POINT1: i32 = 6;

unsafe extern "C" fn counting_input(
    param: *mut c_void,
    _start: u64,
    _end: u64,
    _new_ts: *mut u64,
    _active: u32,
    _mixes: *mut audio_output_data,
) -> bool {
    let count = param as *mut AtomicUsize;
    if !count.is_null() {
        unsafe { (*count).fetch_add(1, Ordering::SeqCst) };
    }
    true
}

fn stereo_info(name: &'static core::ffi::CStr, param: *mut c_void) -> audio_output_info {
    audio_output_info {
        name: name.as_ptr() as *const c_char,
        samples_per_sec: 48000,
        format: AUDIO_FORMAT_FLOAT,
        speakers: SPEAKERS_STEREO,
        input_callback: Some(counting_input),
        input_param: param,
    }
}

/// The four invalid-info cases the cmocka test pins, including not touching
/// *audio on failure.
#[test]
fn open_invalid_params() {
    let bads = [
        audio_output_info {
            name: c"x".as_ptr(),
            samples_per_sec: 48000,
            format: 0, // AUDIO_FORMAT_UNKNOWN
            speakers: SPEAKERS_STEREO,
            input_callback: Some(counting_input),
            input_param: ptr::null_mut(),
        },
        audio_output_info {
            name: ptr::null(),
            samples_per_sec: 48000,
            format: AUDIO_FORMAT_FLOAT,
            speakers: SPEAKERS_STEREO,
            input_callback: Some(counting_input),
            input_param: ptr::null_mut(),
        },
        audio_output_info {
            name: c"x".as_ptr(),
            samples_per_sec: 0,
            format: AUDIO_FORMAT_FLOAT,
            speakers: SPEAKERS_STEREO,
            input_callback: Some(counting_input),
            input_param: ptr::null_mut(),
        },
        audio_output_info {
            name: c"x".as_ptr(),
            samples_per_sec: 48000,
            format: AUDIO_FORMAT_FLOAT,
            speakers: SPEAKERS_UNKNOWN,
            input_callback: Some(counting_input),
            input_param: ptr::null_mut(),
        },
    ];

    for bad in bads {
        let mut audio = 0xdeadbeef_usize as *mut c_void;
        let mut info = bad;
        unsafe {
            assert_eq!(
                audio_output_open(&mut audio, &mut info),
                AUDIO_OUTPUT_INVALIDPARAM
            );
            assert_eq!(audio, 0xdeadbeef_usize as *mut c_void);
        }
    }
}

#[test]
fn open_getters_stereo() {
    let counter = AtomicUsize::new(0);
    let mut info = stereo_info(c"t4-audio", &counter as *const _ as *mut c_void);
    let mut audio = ptr::null_mut();
    unsafe {
        assert_eq!(
            audio_output_open(&mut audio, &mut info),
            AUDIO_OUTPUT_SUCCESS
        );
        assert!(!audio.is_null());

        let got = audio_output_get_info(audio);
        assert!(!got.is_null());
        assert_eq!((*got).name, info.name);
        assert_eq!((*got).samples_per_sec, 48000);
        assert_eq!((*got).format, AUDIO_FORMAT_FLOAT);
        assert_eq!((*got).speakers, SPEAKERS_STEREO);

        assert_eq!(audio_output_get_sample_rate(audio), 48000);
        assert_eq!(audio_output_get_channels(audio), 2);
        assert_eq!(audio_output_get_planes(audio), 1);
        assert_eq!(audio_output_get_block_size(audio), 8);
        assert!(!audio_output_active(audio));

        audio_output_close(audio);
    }
}

#[test]
fn open_getters_planar() {
    let counter = AtomicUsize::new(0);
    let mut info = stereo_info(c"t4-planar", &counter as *const _ as *mut c_void);
    info.format = AUDIO_FORMAT_FLOAT_PLANAR;
    info.speakers = SPEAKERS_5POINT1;
    let mut audio = ptr::null_mut();
    unsafe {
        assert_eq!(
            audio_output_open(&mut audio, &mut info),
            AUDIO_OUTPUT_SUCCESS
        );
        assert_eq!(audio_output_get_channels(audio), 6);
        assert_eq!(audio_output_get_planes(audio), 6);
        assert_eq!(audio_output_get_block_size(audio), 4);
        audio_output_close(audio);
    }
}

unsafe extern "C" fn silent_output(param: *mut c_void, _mix_idx: usize, data: *mut audio_data) {
    let frames = param as *mut u32;
    if !frames.is_null() {
        unsafe { *frames += (*data).frames };
    }
}

unsafe extern "C" fn frames_sink(_param: *mut c_void, _mix_idx: usize, _data: *mut audio_data) {}

#[test]
fn connect_disconnect() {
    let counter = AtomicUsize::new(0);
    let mut info = stereo_info(c"t4-conn", &counter as *const _ as *mut c_void);
    let mut audio = ptr::null_mut();
    unsafe {
        assert_eq!(
            audio_output_open(&mut audio, &mut info),
            AUDIO_OUTPUT_SUCCESS
        );

        // NULL param + NULL conversion: accepted, defaults to output info.
        assert!(audio_output_connect(
            audio,
            0,
            ptr::null(),
            Some(silent_output),
            ptr::null_mut()
        ));
        assert!(audio_output_active(audio));

        // duplicate (callback, param) on the same mix is refused.
        assert!(!audio_output_connect(
            audio,
            0,
            ptr::null(),
            Some(silent_output),
            ptr::null_mut()
        ));

        // a different param is a different input.
        let mut frames: u32 = 0;
        assert!(audio_output_connect(
            audio,
            0,
            ptr::null(),
            Some(silent_output),
            &mut frames as *mut u32 as *mut c_void
        ));

        // a different callback is a different input.
        assert!(audio_output_connect(
            audio,
            0,
            ptr::null(),
            Some(frames_sink),
            ptr::null_mut()
        ));

        // mix index out of range and NULL audio are refused.
        assert!(!audio_output_connect(
            audio,
            6,
            ptr::null(),
            Some(frames_sink),
            ptr::null_mut()
        ));
        assert!(!audio_output_connect(
            ptr::null_mut(),
            0,
            ptr::null(),
            Some(frames_sink),
            ptr::null_mut()
        ));

        // zeroed conversion fields fall back to the output info (no resampler).
        let partial = audio_convert_info {
            samples_per_sec: 0,
            format: 0,
            speakers: 0,
            allow_clipping: false,
        };
        assert!(audio_output_connect(
            audio,
            1,
            &partial,
            Some(frames_sink),
            ptr::null_mut()
        ));

        // a differing conversion needs a resampler; the oracle's
        // deterministic resampler satisfies it, so the connect succeeds.
        let other = audio_convert_info {
            samples_per_sec: 44100,
            format: AUDIO_FORMAT_FLOAT,
            speakers: SPEAKERS_STEREO,
            allow_clipping: false,
        };
        assert!(audio_output_connect(
            audio,
            2,
            &other,
            Some(frames_sink),
            ptr::null_mut()
        ));

        audio_output_disconnect(audio, 0, Some(silent_output), ptr::null_mut());
        audio_output_disconnect(audio, 0, Some(frames_sink), ptr::null_mut());
        audio_output_disconnect(audio, 1, Some(frames_sink), ptr::null_mut());
        audio_output_disconnect(audio, 2, Some(frames_sink), ptr::null_mut());
        audio_output_disconnect(
            audio,
            0,
            Some(silent_output),
            &mut frames as *mut u32 as *mut c_void,
        );
        assert!(!audio_output_active(audio));

        audio_output_close(audio);
    }
}

/// The audio thread really runs: with no resampler the input callback is
/// invoked on every tick.
#[test]
fn input_thread_runs() {
    let counter = AtomicUsize::new(0);
    let mut info = stereo_info(c"t4-thread", &counter as *const _ as *mut c_void);
    let mut audio = ptr::null_mut();
    unsafe {
        assert_eq!(
            audio_output_open(&mut audio, &mut info),
            AUDIO_OUTPUT_SUCCESS
        );

        let deadline = Instant::now() + Duration::from_secs(3);
        while counter.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(counter.load(Ordering::SeqCst) > 0);

        audio_output_close(audio);
    }
}

/// `audio_output_close` must let an in-flight callback finish — including
/// its re-entrant `audio_output_disconnect` — before the owning `Box`
/// (and its `&mut`) exists. Forced ordering, no sleeps: (a) the callback
/// enters and parks; (b) a second thread runs close, which sets stop and
/// reaches the join point — observed through the debug-only
/// `stop_and_join_begins` seam; (c) only then does the callback call
/// `disconnect` and return; (d) close's join completes. If the seam is
/// never observed, the callback gives up with `timed_out` and the test
/// fails — it cannot pass without the overlap.
#[cfg(debug_assertions)]
mod close_overlap {
    use super::*;
    use obs_media_io::audio_io::AudioOutput;
    use std::sync::atomic::{AtomicBool, AtomicPtr};

    struct OvlCtx {
        audio: AtomicPtr<c_void>,
        entered: AtomicUsize,
        timed_out: AtomicBool,
        disconnected: AtomicUsize,
        closed: AtomicBool,
        join_begins_at_disconnect: AtomicUsize,
    }

    unsafe extern "C" fn ovl_input(
        _p: *mut c_void,
        _start: u64,
        end: u64,
        new_ts: *mut u64,
        _active: u32,
        mixes: *mut audio_output_data,
    ) -> bool {
        unsafe {
            *new_ts = end;
            *(*mixes).data[0] = 0.5f32;
        }
        true
    }

    unsafe extern "C" fn ovl_output(p: *mut c_void, mix: usize, _d: *mut audio_data) {
        let ctx = unsafe { &*(p as *const OvlCtx) };
        if ctx.entered.fetch_add(1, Ordering::SeqCst) != 0 {
            return;
        }
        // (c) is allowed only after close reached its join point (b).
        let deadline = Instant::now() + Duration::from_secs(30);
        while AudioOutput::stop_and_join_begins() == 0 {
            if Instant::now() >= deadline {
                ctx.timed_out.store(true, Ordering::SeqCst);
                return;
            }
            std::hint::spin_loop();
        }
        // Re-entrant disconnect on the same handle while close is blocked
        // in join — exactly the `&`-during-close aliasing path. Recording
        // the seam here (not only in the barrier above) is what makes the
        // test fail if the barrier is ever removed: without it the
        // callback reaches disconnect before close begins its join.
        ctx.join_begins_at_disconnect
            .store(AudioOutput::stop_and_join_begins(), Ordering::SeqCst);
        unsafe {
            audio_output_disconnect(ctx.audio.load(Ordering::SeqCst), mix, Some(ovl_output), p)
        };
        ctx.disconnected.fetch_add(1, Ordering::SeqCst);
    }

    #[test]
    fn close_join_overlaps_inflight_disconnect() {
        let ctx = Box::new(OvlCtx {
            audio: AtomicPtr::new(ptr::null_mut()),
            entered: AtomicUsize::new(0),
            timed_out: AtomicBool::new(false),
            disconnected: AtomicUsize::new(0),
            closed: AtomicBool::new(false),
            join_begins_at_disconnect: AtomicUsize::new(0),
        });
        let mut info = stereo_info(c"t4-close-ovl", &*ctx as *const OvlCtx as *mut c_void);
        info.input_callback = Some(ovl_input);
        let mut audio = ptr::null_mut();
        unsafe {
            assert_eq!(
                audio_output_open(&mut audio, &mut info),
                AUDIO_OUTPUT_SUCCESS
            );
            ctx.audio.store(audio, Ordering::SeqCst);
            assert!(audio_output_connect(
                audio,
                0,
                ptr::null(),
                Some(ovl_output),
                &*ctx as *const OvlCtx as *mut c_void
            ));

            // (a) the callback is in flight and parked on the seam.
            let deadline = Instant::now() + Duration::from_secs(30);
            while ctx.entered.load(Ordering::SeqCst) == 0 {
                std::hint::spin_loop();
                assert!(Instant::now() < deadline, "callback never ran");
            }

            // (b)+(d) close on another thread; flag, since join has no
            // timeout — spin on `closed` instead.
            let ctx_addr = &*ctx as *const OvlCtx as usize;
            let audio_addr = audio as usize;
            let close_t = std::thread::spawn(move || {
                audio_output_close(audio_addr as *mut c_void);
                (*(ctx_addr as *const OvlCtx))
                    .closed
                    .store(true, Ordering::SeqCst);
            });
            while !ctx.closed.load(Ordering::SeqCst) {
                std::hint::spin_loop();
                assert!(Instant::now() < deadline, "close never returned");
            }
            close_t.join().unwrap();
        }

        assert!(
            !ctx.timed_out.load(Ordering::SeqCst),
            "close never reached its join point — the callback could not overlap the join"
        );
        assert_eq!(
            ctx.disconnected.load(Ordering::SeqCst),
            1,
            "the in-flight callback's re-entrant disconnect did not run during close's join"
        );
        assert!(
            ctx.join_begins_at_disconnect.load(Ordering::SeqCst) >= 1,
            "disconnect ran before close reached its join — the overlap was never exercised"
        );
    }
}

/// Runs `test` (its `module_path!()` path) alone in a child process of this
/// binary, for checks of process-wide state that a test running in
/// parallel would disturb. In the child this returns `true` and the caller
/// measures there; in the parent it runs the child, asserts that exactly
/// that one test passed, and returns `false` (as obs-util's `isolated`).
fn run_alone(test: &str) -> bool {
    const MARKER: &str = "OBS_MEDIA_IO_ISOLATED_TEST";
    let test = test
        .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
        .unwrap_or(test);
    if std::env::var(MARKER).as_deref() == Ok(test) {
        return true;
    }
    let exe = std::env::current_exe().expect("path of the test binary");
    let out = std::process::Command::new(exe)
        .args([test, "--exact", "--test-threads=1", "--nocapture"])
        .env(MARKER, test)
        .output()
        .expect("start the isolated test process");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "isolated {test} failed:\n{stdout}\n{stderr}"
    );
    assert!(
        stdout.contains("1 passed"),
        "isolated {test} did not run exactly one test:\n{stdout}"
    );
    false
}

/// #5769: the audio thread registers itself as a profiler root with the
/// tick period as its expected time between calls, as the graphics thread
/// does, so the profiler output at quit shows how regularly it runs. The
/// oracle's name store is NULL in this binary, so every audio thread
/// registers the same (NULL-named) root: the test runs in its own process,
/// or an audio thread of a parallel test overwrites the expected time.
#[test]
fn audio_thread_registers_profiler_root_with_tick_period() {
    use obs_media_io::audio_io::{AUDIO_OUTPUT_FRAMES, audio_frames_to_ns};
    use obs_util::profiler as prof;

    if !run_alone(concat!(
        module_path!(),
        "::audio_thread_registers_profiler_root_with_tick_period"
    )) {
        return;
    }
    prof::profiler_start();
    for rate in [48_000u32, 44_100] {
        let counter = AtomicUsize::new(0);
        let mut info = stereo_info(c"t-profiler-root", &counter as *const _ as *mut c_void);
        info.samples_per_sec = rate;
        let mut audio = ptr::null_mut();
        unsafe {
            assert_eq!(
                audio_output_open(&mut audio, &mut info),
                AUDIO_OUTPUT_SUCCESS
            );
            let deadline = Instant::now() + Duration::from_secs(3);
            while counter.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
                std::thread::yield_now();
            }
            audio_output_close(audio);
        }
        let period_ns = audio_frames_to_ns(rate as usize, AUDIO_OUTPUT_FRAMES as u64);
        let expected_us = period_ns.wrapping_add(500) / 1000;
        let roots: Vec<u64> = prof::profile_snapshot_create()
            .roots
            .iter()
            .map(|root| root.expected_time_between_calls)
            .collect();
        assert!(
            roots.contains(&expected_us),
            "{rate} Hz: no profiler root expects {expected_us} us between calls (roots: {roots:?})"
        );
    }
    prof::profiler_free();
}
