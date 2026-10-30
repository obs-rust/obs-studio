//! Tier 3 for `media-io/video-io.c` against the C oracle.
//!
//! Intentional differences, not compared here:
//! - Cache frames are zeroed. C's `bmalloc` leaves them uninitialized, so
//!   only bytes this test writes, plus linesize and timestamps, are compared.
//! - `close` on a divisor child stops the root and frees only the child.
//!   C also frees the parent's cache and mutexes. The child is freed with
//!   `video_output_free_frame_rate_divisor` on both sides.
//! - The stop flag is atomic. C uses a plain `bool`. Not observable here.
//! - `connect(NULL)` and `connect2(NULL)` return false. C crashes in
//!   `get_root`.
//! - `get_skipped_frames(NULL)` and `get_total_frames(NULL)` return 0.
//!   C dereferences the handle.
//! - `open` with a null info or out-pointer returns `VIDEO_OUTPUT_INVALIDPARAM`.
//!   C crashes.
//! - A null frame pointer to `lock_frame` returns true and writes nothing.
//!   C crashes in `memcpy`.
//!
//! `video_scaler_*` stays the FFmpeg C file in libobs. This cargo-test link
//! uses one stub for both sides, so a scaled frame's first byte matches.
//! That stub is not a port difference.

use std::ffi::c_void;
use std::ptr;
use std::sync::{Condvar, Mutex};

use obs_c_oracle::video_io::{
    OracleVideo, OracleVideoData, OracleVideoFrame, OracleVideoOutputInfo, OracleVideoScaleInfo,
    oracle_video_output_align_frame_rate_divisor, oracle_video_output_close,
    oracle_video_output_connect, oracle_video_output_connect2,
    oracle_video_output_create_with_frame_rate_divisor,
    oracle_video_output_free_frame_rate_divisor, oracle_video_output_get_format,
    oracle_video_output_get_frame_rate, oracle_video_output_get_frame_time,
    oracle_video_output_get_info, oracle_video_output_get_total_frames,
    oracle_video_output_get_width, oracle_video_output_lock_frame, oracle_video_output_open,
    oracle_video_output_stop, oracle_video_output_unlock_frame,
};
use obs_media_io::ffi::video_frame::video_frame;
use obs_media_io::ffi::video_io::{
    VIDEO_OUTPUT_INVALIDPARAM, VIDEO_OUTPUT_SUCCESS, video_data,
    video_output_align_frame_rate_divisor, video_output_close, video_output_connect,
    video_output_connect2, video_output_create_with_frame_rate_divisor,
    video_output_free_frame_rate_divisor, video_output_get_format, video_output_get_frame_rate,
    video_output_get_frame_time, video_output_get_info, video_output_get_total_frames,
    video_output_get_width, video_output_info, video_output_lock_frame, video_output_open,
    video_output_stop, video_output_unlock_frame, video_scale_info,
};
use obs_media_io::video_io::{VideoColorspace, VideoFormat, VideoRangeType};
use obs_util::util_uint64::mul_div64;
use proptest::prelude::*;

use obs_c_oracle as _;
use obs_util as _;

const RGBA: i32 = VideoFormat::Rgba as i32;
const CS_709: i32 = VideoColorspace::Cs709 as i32;
const RANGE_FULL: i32 = VideoRangeType::Full as i32;

struct Hits {
    n: usize,
    ts: [u64; 4],
    linesize0: u32,
    byte0: [u8; 4],
}

struct Gate {
    hits: Mutex<Hits>,
    cv: Condvar,
}

fn gate() -> Gate {
    Gate {
        hits: Mutex::new(Hits {
            n: 0,
            ts: [0; 4],
            linesize0: 0,
            byte0: [0; 4],
        }),
        cv: Condvar::new(),
    }
}

fn record(gate: &Gate, timestamp: u64, linesize0: u32, byte0: u8) {
    let mut hits = gate.hits.lock().unwrap();
    let i = hits.n;
    if i < 4 {
        hits.ts[i] = timestamp;
        hits.linesize0 = linesize0;
        hits.byte0[i] = byte0;
    }
    hits.n += 1;
    gate.cv.notify_all();
}

unsafe extern "C" fn on_rust(param: *mut c_void, frame: *mut video_data) {
    let frame = unsafe { &*frame };
    let byte = if frame.data[0].is_null() {
        0
    } else {
        unsafe { *frame.data[0] }
    };
    record(
        unsafe { &*(param as *const Gate) },
        frame.timestamp,
        frame.linesize[0],
        byte,
    );
}

unsafe extern "C" fn on_oracle(param: *mut c_void, frame: *mut OracleVideoData) {
    let frame = unsafe { &*frame };
    let byte = if frame.data[0].is_null() {
        0
    } else {
        unsafe { *frame.data[0] }
    };
    record(
        unsafe { &*(param as *const Gate) },
        frame.timestamp,
        frame.linesize[0],
        byte,
    );
}

fn wait_hits(gate: &Gate, n: usize) {
    let mut hits = gate.hits.lock().unwrap();
    while hits.n < n {
        hits = gate.cv.wait(hits).unwrap();
    }
}

/// One frame with the same byte on both sides, delivered in lockstep.
fn push_both(pair: &Pair, stamp: u64, byte: u8) {
    unsafe {
        let mut rust_frame = video_frame {
            data: [ptr::null_mut(); 8],
            linesize: [0; 8],
        };
        let mut oracle_frame = OracleVideoFrame {
            data: [ptr::null_mut(); 8],
            linesize: [0; 8],
        };
        assert!(video_output_lock_frame(
            pair.rust,
            &mut rust_frame,
            1,
            stamp
        ));
        assert!(oracle_video_output_lock_frame(
            pair.oracle,
            &mut oracle_frame,
            1,
            stamp
        ));
        *rust_frame.data[0] = byte;
        *oracle_frame.data[0] = byte;
        video_output_unlock_frame(pair.rust);
        oracle_video_output_unlock_frame(pair.oracle);
    }
}

fn rust_info(
    name: &'static core::ffi::CStr,
    w: u32,
    h: u32,
    cache: usize,
    fps: u32,
    den: u32,
) -> video_output_info {
    video_output_info {
        name: name.as_ptr(),
        format: RGBA,
        fps_num: fps,
        fps_den: den,
        width: w,
        height: h,
        cache_size: cache,
        colorspace: CS_709,
        range: RANGE_FULL,
    }
}

fn oracle_info(
    name: &'static core::ffi::CStr,
    w: u32,
    h: u32,
    cache: usize,
    fps: u32,
    den: u32,
) -> OracleVideoOutputInfo {
    OracleVideoOutputInfo {
        name: name.as_ptr(),
        format: RGBA,
        fps_num: fps,
        fps_den: den,
        width: w,
        height: h,
        cache_size: cache,
        colorspace: CS_709,
        range: RANGE_FULL,
    }
}

struct Pair {
    rust: *mut c_void,
    oracle: *mut OracleVideo,
}

impl Pair {
    fn open(w: u32, h: u32, cache: usize, fps: u32, den: u32) -> Self {
        let mut rust_info = rust_info(c"parity", w, h, cache, fps, den);
        let mut oracle_info = oracle_info(c"parity", w, h, cache, fps, den);
        let mut rust_video = ptr::null_mut();
        let mut oracle_video = ptr::null_mut();
        unsafe {
            assert_eq!(
                video_output_open(&mut rust_video, &mut rust_info),
                VIDEO_OUTPUT_SUCCESS
            );
            assert_eq!(
                oracle_video_output_open(&mut oracle_video, &mut oracle_info),
                VIDEO_OUTPUT_SUCCESS
            );
        }
        Self {
            rust: rust_video,
            oracle: oracle_video,
        }
    }
}

impl Drop for Pair {
    fn drop(&mut self) {
        unsafe {
            if !self.rust.is_null() {
                video_output_close(self.rust);
            }
            if !self.oracle.is_null() {
                oracle_video_output_close(self.oracle);
            }
        }
    }
}

#[test]
fn invalid_open_leaves_the_pointer() {
    let mut rust_info = rust_info(c"bad", 0, 4, 2, 30, 1);
    let mut oracle_info = oracle_info(c"bad", 0, 4, 2, 30, 1);
    let mut rust_video = 0xdeadbeef_usize as *mut c_void;
    let mut oracle_video = 0xdeadbeef_usize as *mut OracleVideo;
    unsafe {
        assert_eq!(
            video_output_open(&mut rust_video, &mut rust_info),
            VIDEO_OUTPUT_INVALIDPARAM
        );
        assert_eq!(
            oracle_video_output_open(&mut oracle_video, &mut oracle_info),
            VIDEO_OUTPUT_INVALIDPARAM
        );
    }
    assert_eq!(rust_video, 0xdeadbeef_usize as *mut c_void);
    assert_eq!(oracle_video, 0xdeadbeef_usize as *mut OracleVideo);
}

#[test]
fn getters_match() {
    let pair = Pair::open(8, 4, 100, 30, 1);
    let expect_time = mul_div64(1_000_000_000, 1, 30);
    unsafe {
        let rust_info = &*video_output_get_info(pair.rust);
        let oracle_info = &*oracle_video_output_get_info(pair.oracle);
        assert_eq!(rust_info.cache_size, oracle_info.cache_size);
        assert_eq!(rust_info.cache_size, 16);
        assert_eq!(rust_info.width, oracle_info.width);
        assert_eq!(rust_info.fps_num, oracle_info.fps_num);
        assert_eq!(rust_info.fps_den, oracle_info.fps_den);
        assert_eq!(rust_info.name, oracle_info.name);
        assert_eq!(
            video_output_get_frame_time(pair.rust),
            oracle_video_output_get_frame_time(pair.oracle)
        );
        assert_eq!(video_output_get_frame_time(pair.rust), expect_time);
        assert_eq!(
            video_output_get_frame_rate(pair.rust),
            oracle_video_output_get_frame_rate(pair.oracle)
        );
        assert_eq!(
            video_output_get_width(pair.rust),
            oracle_video_output_get_width(pair.oracle)
        );
        assert_eq!(
            video_output_get_format(pair.rust),
            oracle_video_output_get_format(pair.oracle)
        );
    }
}

#[test]
fn one_frame_matches() {
    let pair = Pair::open(4, 4, 2, 30, 1);
    let rust_gate = gate();
    let oracle_gate = gate();
    unsafe {
        assert!(video_output_connect(
            pair.rust,
            ptr::null(),
            Some(on_rust),
            &rust_gate as *const Gate as *mut c_void
        ));
        assert!(oracle_video_output_connect(
            pair.oracle,
            ptr::null(),
            Some(on_oracle),
            &oracle_gate as *const Gate as *mut c_void
        ));
        let mut rust_frame = video_frame {
            data: [ptr::null_mut(); 8],
            linesize: [0; 8],
        };
        let mut oracle_frame = OracleVideoFrame {
            data: [ptr::null_mut(); 8],
            linesize: [0; 8],
        };
        assert!(video_output_lock_frame(pair.rust, &mut rust_frame, 1, 1000));
        assert!(oracle_video_output_lock_frame(
            pair.oracle,
            &mut oracle_frame,
            1,
            1000
        ));
        assert_eq!(rust_frame.linesize[0], oracle_frame.linesize[0]);
        *rust_frame.data[0] = 0xAB;
        *oracle_frame.data[0] = 0xAB;
        video_output_unlock_frame(pair.rust);
        oracle_video_output_unlock_frame(pair.oracle);
    }
    wait_hits(&rust_gate, 1);
    wait_hits(&oracle_gate, 1);
    unsafe {
        video_output_stop(pair.rust);
        oracle_video_output_stop(pair.oracle);
    }
    let rust_hits = rust_gate.hits.lock().unwrap();
    let oracle_hits = oracle_gate.hits.lock().unwrap();
    assert_eq!(rust_hits.n, oracle_hits.n);
    assert_eq!(rust_hits.ts[0], oracle_hits.ts[0]);
    assert_eq!(rust_hits.byte0[0], oracle_hits.byte0[0]);
    assert_eq!(rust_hits.linesize0, oracle_hits.linesize0);
    assert_eq!(rust_hits.byte0[0], 0xAB);
    drop(rust_hits);
    unsafe {
        assert_eq!(
            video_output_get_total_frames(pair.rust),
            oracle_video_output_get_total_frames(pair.oracle)
        );
    }
}

#[test]
fn divisor_skip_matches() {
    let pair = Pair::open(4, 4, 4, 30, 1);
    let rust_gate = gate();
    let oracle_gate = gate();
    let stamps = [1000_u64, 2000, 3000];
    unsafe {
        assert!(video_output_connect2(
            pair.rust,
            ptr::null(),
            2,
            Some(on_rust),
            &rust_gate as *const Gate as *mut c_void
        ));
        assert!(oracle_video_output_connect2(
            pair.oracle,
            ptr::null(),
            2,
            Some(on_oracle),
            &oracle_gate as *const Gate as *mut c_void
        ));
        for (i, stamp) in stamps.iter().enumerate() {
            let mut rust_frame = video_frame {
                data: [ptr::null_mut(); 8],
                linesize: [0; 8],
            };
            let mut oracle_frame = OracleVideoFrame {
                data: [ptr::null_mut(); 8],
                linesize: [0; 8],
            };
            assert!(video_output_lock_frame(
                pair.rust,
                &mut rust_frame,
                1,
                *stamp
            ));
            assert!(oracle_video_output_lock_frame(
                pair.oracle,
                &mut oracle_frame,
                1,
                *stamp
            ));
            *rust_frame.data[0] = (i + 1) as u8;
            *oracle_frame.data[0] = (i + 1) as u8;
            video_output_unlock_frame(pair.rust);
            oracle_video_output_unlock_frame(pair.oracle);
        }
    }
    wait_hits(&rust_gate, 2);
    wait_hits(&oracle_gate, 2);
    unsafe {
        video_output_stop(pair.rust);
        oracle_video_output_stop(pair.oracle);
    }
    let rust_hits = rust_gate.hits.lock().unwrap();
    let oracle_hits = oracle_gate.hits.lock().unwrap();
    assert_eq!(rust_hits.n, 2);
    assert_eq!(rust_hits.ts, oracle_hits.ts);
    assert_eq!(rust_hits.byte0, oracle_hits.byte0);
    assert_eq!(rust_hits.byte0[0], 1);
    assert_eq!(rust_hits.byte0[1], 3);
}

/// After one delivered frame of a divisor-3 input both sides skip the next
/// one; aligning on a start timestamp on both sides drops the frames before it
/// and delivers the one at it as the first of the phase, and a callback or
/// param that is not connected stays false.
#[test]
fn divisor_align_matches() {
    let pair = Pair::open(4, 4, 4, 30, 1);
    let rust_gate = gate();
    let oracle_gate = gate();
    let rust_param = &rust_gate as *const Gate as *mut c_void;
    let oracle_param = &oracle_gate as *const Gate as *mut c_void;
    let stamps = [1000_u64, 2000, 3000, 4000];
    unsafe {
        assert!(video_output_connect2(
            pair.rust,
            ptr::null(),
            3,
            Some(on_rust),
            rust_param
        ));
        assert!(oracle_video_output_connect2(
            pair.oracle,
            ptr::null(),
            3,
            Some(on_oracle),
            oracle_param
        ));
    }
    push_both(&pair, stamps[0], 1);
    wait_hits(&rust_gate, 1);
    wait_hits(&oracle_gate, 1);
    unsafe {
        assert!(video_output_align_frame_rate_divisor(
            pair.rust,
            3000,
            Some(on_rust),
            rust_param
        ));
        assert!(oracle_video_output_align_frame_rate_divisor(
            pair.oracle,
            3000,
            Some(on_oracle),
            oracle_param
        ));
        assert!(!video_output_align_frame_rate_divisor(
            pair.rust,
            3000,
            Some(on_rust),
            ptr::null_mut()
        ));
        assert!(!oracle_video_output_align_frame_rate_divisor(
            pair.oracle,
            3000,
            Some(on_oracle),
            ptr::null_mut()
        ));
        assert!(!video_output_align_frame_rate_divisor(
            pair.rust, 3000, None, rust_param
        ));
        assert!(!oracle_video_output_align_frame_rate_divisor(
            pair.oracle,
            3000,
            None,
            oracle_param
        ));
    }
    for (i, stamp) in stamps[1..].iter().enumerate() {
        push_both(&pair, *stamp, (i + 2) as u8);
    }
    wait_hits(&rust_gate, 2);
    wait_hits(&oracle_gate, 2);
    unsafe {
        video_output_stop(pair.rust);
        oracle_video_output_stop(pair.oracle);
    }
    let rust_hits = rust_gate.hits.lock().unwrap();
    let oracle_hits = oracle_gate.hits.lock().unwrap();
    assert_eq!(rust_hits.n, 2);
    assert_eq!(rust_hits.ts, oracle_hits.ts);
    assert_eq!(rust_hits.byte0, oracle_hits.byte0);
    assert_eq!(rust_hits.ts[0], 1000);
    assert_eq!(rust_hits.ts[1], 3000);
}

#[test]
fn child_fps_den_matches() {
    let pair = Pair::open(4, 4, 2, 30, 1);
    unsafe {
        let rust_child = video_output_create_with_frame_rate_divisor(pair.rust, 2);
        let oracle_child = oracle_video_output_create_with_frame_rate_divisor(pair.oracle, 2);
        assert!(!rust_child.is_null());
        assert!(!oracle_child.is_null());
        assert_eq!(
            (*video_output_get_info(rust_child)).fps_den,
            (*oracle_video_output_get_info(oracle_child)).fps_den
        );
        assert_eq!((*video_output_get_info(rust_child)).fps_den, 2);
        assert_eq!(
            video_output_get_frame_time(rust_child),
            oracle_video_output_get_frame_time(oracle_child)
        );
        assert_eq!(
            video_output_get_frame_rate(rust_child),
            oracle_video_output_get_frame_rate(oracle_child)
        );
        video_output_free_frame_rate_divisor(rust_child);
        oracle_video_output_free_frame_rate_divisor(oracle_child);
    }
}

#[test]
fn scale_linesize_matches() {
    let pair = Pair::open(4, 4, 2, 30, 1);
    let rust_gate = gate();
    let oracle_gate = gate();
    let rust_conv = video_scale_info {
        format: RGBA,
        width: 2,
        height: 2,
        range: RANGE_FULL,
        colorspace: CS_709,
    };
    let oracle_conv = OracleVideoScaleInfo {
        format: RGBA,
        width: 2,
        height: 2,
        range: RANGE_FULL,
        colorspace: CS_709,
    };
    unsafe {
        assert!(video_output_connect(
            pair.rust,
            &rust_conv,
            Some(on_rust),
            &rust_gate as *const Gate as *mut c_void
        ));
        assert!(oracle_video_output_connect(
            pair.oracle,
            &oracle_conv,
            Some(on_oracle),
            &oracle_gate as *const Gate as *mut c_void
        ));
        let mut rust_frame = video_frame {
            data: [ptr::null_mut(); 8],
            linesize: [0; 8],
        };
        let mut oracle_frame = OracleVideoFrame {
            data: [ptr::null_mut(); 8],
            linesize: [0; 8],
        };
        assert!(video_output_lock_frame(pair.rust, &mut rust_frame, 1, 11));
        assert!(oracle_video_output_lock_frame(
            pair.oracle,
            &mut oracle_frame,
            1,
            11
        ));
        *rust_frame.data[0] = 0x33;
        *oracle_frame.data[0] = 0x33;
        video_output_unlock_frame(pair.rust);
        oracle_video_output_unlock_frame(pair.oracle);
    }
    wait_hits(&rust_gate, 1);
    wait_hits(&oracle_gate, 1);
    let rust_hits = rust_gate.hits.lock().unwrap();
    let oracle_hits = oracle_gate.hits.lock().unwrap();
    assert_eq!(rust_hits.linesize0, oracle_hits.linesize0);
    assert_eq!(rust_hits.linesize0, 8);
    assert_eq!(rust_hits.byte0[0], oracle_hits.byte0[0]);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(32)))]

    #[test]
    fn random_getters_match(
        fps in 1u32..500,
        den in 1u32..500,
        cache in 0usize..40,
        w in 1u32..32,
        h in 1u32..32,
    ) {
        let pair = Pair::open(w, h, cache, fps, den);
        let expect_time = mul_div64(1_000_000_000, u64::from(den), u64::from(fps));
        let expect_cache = cache.min(16);
        unsafe {
            assert_eq!((*video_output_get_info(pair.rust)).cache_size, expect_cache);
            assert_eq!(
                (*video_output_get_info(pair.rust)).cache_size,
                (*oracle_video_output_get_info(pair.oracle)).cache_size
            );
            assert_eq!(
                video_output_get_frame_time(pair.rust),
                oracle_video_output_get_frame_time(pair.oracle)
            );
            assert_eq!(video_output_get_frame_time(pair.rust), expect_time);
            assert_eq!(
                video_output_get_width(pair.rust),
                oracle_video_output_get_width(pair.oracle)
            );
            assert_eq!(video_output_get_width(pair.rust), w);
            assert_eq!(
                video_output_get_frame_rate(pair.rust).to_bits(),
                oracle_video_output_get_frame_rate(pair.oracle).to_bits()
            );
        }
    }
}
