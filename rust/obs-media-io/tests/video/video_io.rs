//! Tier 1 for the `video-io.c` port. The assertions mirror
//! `test/cmocka/test_video_io.c` and `test/cmocka/test_video_io_hidden.c`.
//! The crash-only pin (`open_crashes_without_obs_core`) stays in cmocka.
//!
//! A few extra asserts cover the null cases the C test cannot call.
//! `connect(NULL)` and `connect2(NULL)` crash in `get_root` before the null
//! check. `get_skipped_frames(NULL)` and `get_total_frames(NULL)`
//! dereference the handle. `open` with a null info or out-pointer crashes.
//! The port returns false, 0, and `VIDEO_OUTPUT_INVALIDPARAM` for those.
//!
//! Sync is a condvar, same idea as the cmocka gate: the callback records,
//! then waits until the test releases it. No sleeps.

use std::ffi::{c_int, c_void};
use std::ptr;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use obs_media_io::ffi::video_frame::video_frame;
use obs_media_io::ffi::video_io::*;
use obs_media_io::video_io::{VideoColorspace, VideoFormat, VideoRangeType};
use obs_util::util_uint64::mul_div64;

use obs_c_oracle as _;
use obs_util as _;

const GATE_MAX: usize = 8;
const RGBA: c_int = VideoFormat::Rgba as c_int;
const AYUV: c_int = VideoFormat::Ayuv as c_int;
const CS_709: c_int = VideoColorspace::Cs709 as c_int;
const CS_SRGB: c_int = VideoColorspace::Srgb as c_int;
const RANGE_FULL: c_int = VideoRangeType::Full as c_int;

struct Hits {
    n: usize,
    release: bool,
    ts: [u64; GATE_MAX],
    linesize0: u32,
    byte0: [u8; GATE_MAX],
}

struct Gate {
    hits: Mutex<Hits>,
    cv: Condvar,
}

struct Session {
    video: *mut c_void,
    gate: Gate,
}

impl Session {
    fn open(info: &mut video_output_info, block: bool) -> Self {
        let mut video = ptr::null_mut();
        unsafe {
            assert_eq!(video_output_open(&mut video, info), VIDEO_OUTPUT_SUCCESS);
        }
        assert!(!video.is_null());
        Self {
            video,
            gate: Gate {
                hits: Mutex::new(Hits {
                    n: 0,
                    release: !block,
                    ts: [0; GATE_MAX],
                    linesize0: 0,
                    byte0: [0; GATE_MAX],
                }),
                cv: Condvar::new(),
            },
        }
    }

    fn param(&self) -> *mut c_void {
        &self.gate as *const Gate as *mut c_void
    }

    fn wait_hits(&self, n: usize) {
        let mut hits = self.gate.hits.lock().unwrap();
        while hits.n < n {
            hits = self.gate.cv.wait(hits).unwrap();
        }
    }

    /// Waits until the video thread has finished `total` frames, so a test can
    /// assert on the counters without racing `video_output_stop`: a frame still
    /// queued when the stop flag is set is dropped, as in C.
    fn settle(&self, total: u32) {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let seen = unsafe { video_output_get_total_frames(self.video) };
            if seen >= total {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "video output processed {seen} of {total} pushed frames"
            );
            std::thread::yield_now();
        }
    }

    fn release(&self) {
        let mut hits = self.gate.hits.lock().unwrap();
        hits.release = true;
        self.gate.cv.notify_all();
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.release();
        unsafe { video_output_close(self.video) };
    }
}

unsafe extern "C" fn on_frame(param: *mut c_void, frame: *mut video_data) {
    let gate = unsafe { &*(param as *const Gate) };
    let frame = unsafe { &*frame };
    let mut hits = gate.hits.lock().unwrap();
    let i = hits.n;
    if i < GATE_MAX {
        hits.ts[i] = frame.timestamp;
        hits.byte0[i] = if frame.data[0].is_null() {
            0
        } else {
            unsafe { *frame.data[0] }
        };
    }
    hits.linesize0 = frame.linesize[0];
    hits.n += 1;
    gate.cv.notify_all();
    while !hits.release {
        hits = gate.cv.wait(hits).unwrap();
    }
}

fn rgba_info(
    name: &'static core::ffi::CStr,
    width: u32,
    height: u32,
    cache: usize,
) -> video_output_info {
    video_output_info {
        name: name.as_ptr(),
        format: RGBA,
        fps_num: 30,
        fps_den: 1,
        width,
        height,
        cache_size: cache,
        colorspace: CS_709,
        range: RANGE_FULL,
    }
}

fn frame_time(info: &video_output_info) -> u64 {
    mul_div64(
        1_000_000_000,
        u64::from(info.fps_den),
        u64::from(info.fps_num),
    )
}

/// The four invalid-info cases, plus a null info and a null out-pointer.
/// Invalid params do not write the out-pointer. The null cases crash in C.
#[test]
fn open_invalid_params() {
    let base = rgba_info(c"bad", 4, 4, 2);
    let mut bads = [base, base, base, base];
    bads[0].width = 0;
    bads[1].height = 0;
    bads[2].fps_num = 0;
    bads[3].fps_den = 0;

    for bad in &mut bads {
        let mut video = 0xdeadbeef_usize as *mut c_void;
        unsafe {
            assert_eq!(
                video_output_open(&mut video, bad),
                VIDEO_OUTPUT_INVALIDPARAM
            );
        }
        assert_eq!(video, 0xdeadbeef_usize as *mut c_void);
    }

    let mut video = 0xdeadbeef_usize as *mut c_void;
    unsafe {
        assert_eq!(
            video_output_open(&mut video, ptr::null_mut()),
            VIDEO_OUTPUT_INVALIDPARAM
        );
        assert_eq!(
            video_output_open(ptr::null_mut(), &mut bads[0]),
            VIDEO_OUTPUT_INVALIDPARAM
        );
    }
    assert_eq!(video, 0xdeadbeef_usize as *mut c_void);
}

/// Null-handle results that are defined in C, plus the ones that crash there.
#[test]
fn null_handles() {
    let mut frame = video_frame {
        data: [ptr::null_mut(); 8],
        linesize: [0; 8],
    };
    unsafe {
        // connect and connect2 call get_root before the NULL check, so a
        // NULL video segfaults in C. The port returns false.
        assert!(!video_output_connect(
            ptr::null_mut(),
            ptr::null(),
            Some(on_frame),
            ptr::null_mut()
        ));
        assert!(!video_output_connect2(
            ptr::null_mut(),
            ptr::null(),
            1,
            Some(on_frame),
            ptr::null_mut()
        ));
        video_output_disconnect(ptr::null_mut(), Some(on_frame), ptr::null_mut());
        assert!(!video_output_disconnect2(
            ptr::null_mut(),
            Some(on_frame),
            ptr::null_mut()
        ));
        assert!(!video_output_active(ptr::null()));
        assert!(video_output_get_info(ptr::null()).is_null());
        assert!(!video_output_lock_frame(ptr::null_mut(), &mut frame, 1, 0));
        video_output_unlock_frame(ptr::null_mut());
        assert_eq!(video_output_get_frame_time(ptr::null()), 0);
        video_output_stop(ptr::null_mut());
        assert!(video_output_stopped(ptr::null_mut()));
        assert_eq!(
            video_output_get_format(ptr::null()),
            VideoFormat::None as c_int
        );
        assert_eq!(video_output_get_width(ptr::null()), 0);
        assert_eq!(video_output_get_height(ptr::null()), 0);
        assert!(video_output_get_frame_rate(ptr::null()) == 0.0);
        // These two dereference the handle in C. The port returns 0.
        assert_eq!(video_output_get_skipped_frames(ptr::null()), 0);
        assert_eq!(video_output_get_total_frames(ptr::null()), 0);
        video_output_close(ptr::null_mut());
    }
}

#[test]
fn open_getters() {
    let mut info = rgba_info(c"getters", 8, 4, 100);
    let expected_time = frame_time(&info);
    let session = Session::open(&mut info, false);
    let video = session.video;
    unsafe {
        let got = &*video_output_get_info(video);
        assert_eq!(got.name, info.name);
        assert_eq!(got.format, RGBA);
        assert_eq!(got.fps_num, 30);
        assert_eq!(got.fps_den, 1);
        assert_eq!(got.width, 8);
        assert_eq!(got.height, 4);
        assert_eq!(got.cache_size, 16);
        assert_eq!(got.colorspace, CS_709);
        assert_eq!(got.range, RANGE_FULL);

        assert_eq!(video_output_get_format(video), RGBA);
        assert_eq!(video_output_get_width(video), 8);
        assert_eq!(video_output_get_height(video), 4);
        assert!(video_output_get_frame_rate(video) == 30.0);
        assert!(video_output_get_frame_time(video) == expected_time);
        assert!(!video_output_stopped(video));
        assert!(!video_output_active(video));
        assert_eq!(video_output_get_skipped_frames(video), 0);
        assert_eq!(video_output_get_total_frames(video), 0);

        video_output_stop(video);
        assert!(video_output_stopped(video));
        video_output_stop(video);
        assert!(video_output_stopped(video));
    }
}

#[test]
fn cache_size_zero_lock_fails() {
    let mut info = rgba_info(c"empty-cache", 4, 4, 0);
    let session = Session::open(&mut info, false);
    let mut frame = video_frame {
        data: [ptr::null_mut(); 8],
        linesize: [0; 8],
    };
    unsafe {
        assert_eq!((*video_output_get_info(session.video)).cache_size, 0);
        assert!(!video_output_lock_frame(session.video, &mut frame, 1, 1));
    }
}

#[test]
fn connect_delivers_one_frame() {
    let mut info = rgba_info(c"one", 4, 4, 2);
    let session = Session::open(&mut info, false);
    let video = session.video;
    let param = session.param();
    unsafe {
        assert!(video_output_connect(
            video,
            ptr::null(),
            Some(on_frame),
            param
        ));
        assert!(!video_output_connect(
            video,
            ptr::null(),
            Some(on_frame),
            param
        ));
        assert!(video_output_active(video));

        let mut frame = video_frame {
            data: [ptr::null_mut(); 8],
            linesize: [0; 8],
        };
        assert!(video_output_lock_frame(video, &mut frame, 1, 1000));
        assert!(!frame.data[0].is_null());
        assert_eq!(frame.linesize[0], 16);
        *frame.data[0] = 0xAB;
        video_output_unlock_frame(video);
    }
    session.wait_hits(1);
    unsafe { video_output_stop(video) };
    let hits = session.gate.hits.lock().unwrap();
    assert_eq!(hits.n, 1);
    assert!(hits.ts[0] == 1000);
    assert_eq!(hits.linesize0, 16);
    assert_eq!(hits.byte0[0], 0xAB);
    drop(hits);
    unsafe {
        assert_eq!(video_output_get_total_frames(video), 1);
        assert_eq!(video_output_get_skipped_frames(video), 0);
        assert!(video_output_disconnect2(video, Some(on_frame), param));
        assert!(!video_output_disconnect2(video, Some(on_frame), param));
        assert!(!video_output_active(video));
    }
}

#[test]
fn connect_rejects() {
    let mut info = rgba_info(c"reject", 4, 4, 2);
    let session = Session::open(&mut info, false);
    let video = session.video;
    let param = session.param();
    let ayuv = video_scale_info {
        format: AYUV,
        width: 4,
        height: 4,
        range: RANGE_FULL,
        colorspace: CS_709,
    };
    unsafe {
        assert!(!video_output_connect(video, ptr::null(), None, param));
        assert!(!video_output_connect2(
            video,
            ptr::null(),
            0,
            Some(on_frame),
            param
        ));
        assert!(!video_output_disconnect2(video, None, param));
        assert!(!video_output_connect(video, &ayuv, Some(on_frame), param));
        assert!(!video_output_active(video));
    }
}

#[test]
fn zero_conversion_size_uses_output() {
    let mut info = rgba_info(c"zero-conv", 4, 4, 2);
    let session = Session::open(&mut info, false);
    let video = session.video;
    let conv = video_scale_info {
        format: RGBA,
        width: 0,
        height: 0,
        range: RANGE_FULL,
        colorspace: CS_709,
    };
    unsafe {
        assert!(video_output_connect(
            video,
            &conv,
            Some(on_frame),
            session.param()
        ));
        let mut frame = video_frame {
            data: [ptr::null_mut(); 8],
            linesize: [0; 8],
        };
        assert!(video_output_lock_frame(video, &mut frame, 1, 7));
        *frame.data[0] = 0x11;
        video_output_unlock_frame(video);
    }
    session.wait_hits(1);
    unsafe { video_output_stop(video) };
    let hits = session.gate.hits.lock().unwrap();
    assert_eq!(hits.linesize0, 16);
    assert_eq!(hits.byte0[0], 0x11);
}

#[test]
fn srgb_matches_709_without_scaler() {
    let mut info = rgba_info(c"srgb", 4, 4, 2);
    let session = Session::open(&mut info, false);
    let video = session.video;
    let conv = video_scale_info {
        format: RGBA,
        width: 4,
        height: 4,
        range: RANGE_FULL,
        colorspace: CS_SRGB,
    };
    unsafe {
        assert!(video_output_connect(
            video,
            &conv,
            Some(on_frame),
            session.param()
        ));
        let mut frame = video_frame {
            data: [ptr::null_mut(); 8],
            linesize: [0; 8],
        };
        assert!(video_output_lock_frame(video, &mut frame, 1, 9));
        *frame.data[0] = 0x22;
        video_output_unlock_frame(video);
    }
    session.wait_hits(1);
    unsafe { video_output_stop(video) };
    let hits = session.gate.hits.lock().unwrap();
    assert_eq!(hits.linesize0, 16);
    assert_eq!(hits.byte0[0], 0x22);
}

#[test]
fn scale_changes_linesize() {
    let mut info = rgba_info(c"scale", 4, 4, 2);
    let session = Session::open(&mut info, false);
    let video = session.video;
    let conv = video_scale_info {
        format: RGBA,
        width: 2,
        height: 2,
        range: RANGE_FULL,
        colorspace: CS_709,
    };
    unsafe {
        assert!(video_output_connect(
            video,
            &conv,
            Some(on_frame),
            session.param()
        ));
        let mut frame = video_frame {
            data: [ptr::null_mut(); 8],
            linesize: [0; 8],
        };
        assert!(video_output_lock_frame(video, &mut frame, 1, 11));
        *frame.data[0] = 0x33;
        video_output_unlock_frame(video);
    }
    session.wait_hits(1);
    unsafe { video_output_stop(video) };
    let hits = session.gate.hits.lock().unwrap();
    assert_eq!(hits.linesize0, 8);
}

#[test]
fn count_repeats_frame() {
    let mut info = rgba_info(c"repeat", 4, 4, 2);
    let expected = frame_time(&info);
    let session = Session::open(&mut info, false);
    let video = session.video;
    unsafe {
        assert!(video_output_get_frame_time(video) == expected);
        assert!(video_output_connect(
            video,
            ptr::null(),
            Some(on_frame),
            session.param()
        ));
        let mut frame = video_frame {
            data: [ptr::null_mut(); 8],
            linesize: [0; 8],
        };
        assert!(video_output_lock_frame(video, &mut frame, 2, 5000));
        *frame.data[0] = 0x44;
        video_output_unlock_frame(video);
    }
    session.wait_hits(2);
    unsafe { video_output_stop(video) };
    let hits = session.gate.hits.lock().unwrap();
    assert_eq!(hits.n, 2);
    assert!(hits.ts[0] == 5000);
    assert!(hits.ts[1] == 5000 + expected);
    assert_eq!(hits.byte0[0], 0x44);
    assert_eq!(hits.byte0[1], 0x44);
    drop(hits);
    unsafe {
        assert_eq!(video_output_get_total_frames(video), 2);
        assert_eq!(video_output_get_skipped_frames(video), 0);
    }
}

#[test]
fn frame_rate_divisor_skips_every_other() {
    let mut info = rgba_info(c"div2", 4, 4, 4);
    let session = Session::open(&mut info, false);
    let video = session.video;
    let stamps = [1000_u64, 2000, 3000];
    let bytes = [1_u8, 2, 3];
    unsafe {
        assert!(video_output_connect2(
            video,
            ptr::null(),
            2,
            Some(on_frame),
            session.param()
        ));
        for i in 0..3 {
            let mut frame = video_frame {
                data: [ptr::null_mut(); 8],
                linesize: [0; 8],
            };
            assert!(video_output_lock_frame(video, &mut frame, 1, stamps[i]));
            *frame.data[0] = bytes[i];
            video_output_unlock_frame(video);
        }
    }
    session.wait_hits(2);
    unsafe { video_output_stop(video) };
    let hits = session.gate.hits.lock().unwrap();
    assert_eq!(hits.n, 2);
    assert!(hits.ts[0] == 1000);
    assert!(hits.ts[1] == 3000);
    assert_eq!(hits.byte0[0], 1);
    assert_eq!(hits.byte0[1], 3);
    drop(hits);
    unsafe {
        assert_eq!(video_output_get_total_frames(video), 3);
    }
}

/// C's video_output_align_frame_rate_divisor: after one delivered frame the
/// counter is 1, so the next frame would be skipped. Aligning on a start
/// timestamp drops the frames before it without moving the counter, so the
/// first frame at or after it is delivered.
#[test]
fn align_divisor_restarts_the_phase() {
    let mut info = rgba_info(c"div3align", 4, 4, 4);
    let session = Session::open(&mut info, false);
    let video = session.video;
    let stamps = [1000_u64, 2000, 3000, 4000];
    unsafe {
        assert!(video_output_connect2(
            video,
            ptr::null(),
            3,
            Some(on_frame),
            session.param()
        ));
        let push = |stamp: u64| {
            let mut frame = video_frame {
                data: [ptr::null_mut(); 8],
                linesize: [0; 8],
            };
            assert!(video_output_lock_frame(video, &mut frame, 1, stamp));
            video_output_unlock_frame(video);
        };
        push(stamps[0]);
        session.wait_hits(1);
        // The counter is 1 here, so without the alignment frame 2000 would be
        // skipped; the phase starts at 3000 instead, so both 2000 (before the
        // phase) and 4000 (after it) behave as the start frame does.
        assert!(video_output_align_frame_rate_divisor(
            video,
            3000,
            Some(on_frame),
            session.param()
        ));
        assert!(!video_output_align_frame_rate_divisor(
            video,
            3000,
            Some(on_frame),
            ptr::null_mut()
        ));
        assert!(!video_output_align_frame_rate_divisor(
            video,
            3000,
            None,
            session.param()
        ));
        assert!(!video_output_align_frame_rate_divisor(
            ptr::null_mut(),
            3000,
            Some(on_frame),
            session.param()
        ));
        for stamp in stamps[1..].iter() {
            push(*stamp);
        }
    }
    session.wait_hits(2);
    // 4000 is pushed but skipped by the divisor; it still has to be processed
    // before stop, which drops whatever is still queued (C does the same).
    session.settle(4);
    unsafe { video_output_stop(video) };
    let hits = session.gate.hits.lock().unwrap();
    assert_eq!(hits.n, 2);
    assert!(hits.ts[0] == 1000);
    assert!(hits.ts[1] == 3000);
    drop(hits);
    unsafe { assert_eq!(video_output_get_total_frames(video), 4) };
}

#[test]
fn full_cache_counts_a_skip() {
    let mut info = rgba_info(c"full", 4, 4, 1);
    let expected = frame_time(&info);
    let session = Session::open(&mut info, true);
    let video = session.video;
    unsafe {
        assert!(video_output_get_frame_time(video) == expected);
        assert!(video_output_connect(
            video,
            ptr::null(),
            Some(on_frame),
            session.param()
        ));
        let mut frame = video_frame {
            data: [ptr::null_mut(); 8],
            linesize: [0; 8],
        };
        assert!(video_output_lock_frame(video, &mut frame, 1, 1000));
        *frame.data[0] = 0xAB;
        video_output_unlock_frame(video);
    }
    session.wait_hits(1);
    unsafe {
        let mut frame = video_frame {
            data: [ptr::null_mut(); 8],
            linesize: [0; 8],
        };
        assert!(!video_output_lock_frame(video, &mut frame, 1, 999));
    }
    session.release();
    session.wait_hits(2);
    unsafe { video_output_stop(video) };
    let hits = session.gate.hits.lock().unwrap();
    assert_eq!(hits.n, 2);
    assert!(hits.ts[0] == 1000);
    assert!(hits.ts[1] == 1000 + expected);
    assert_eq!(hits.byte0[0], 0xAB);
    drop(hits);
    unsafe {
        assert_eq!(video_output_get_skipped_frames(video), 1);
        assert_eq!(video_output_get_total_frames(video), 2);
    }
}

#[test]
fn connect_resets_frame_counters() {
    let mut info = rgba_info(c"reset", 4, 4, 2);
    let session = Session::open(&mut info, false);
    let video = session.video;
    let param = session.param();
    unsafe {
        assert!(video_output_connect(
            video,
            ptr::null(),
            Some(on_frame),
            param
        ));
        let mut frame = video_frame {
            data: [ptr::null_mut(); 8],
            linesize: [0; 8],
        };
        assert!(video_output_lock_frame(video, &mut frame, 1, 1000));
        *frame.data[0] = 1;
        video_output_unlock_frame(video);
    }
    session.wait_hits(1);
    unsafe {
        video_output_stop(video);
        assert_eq!(video_output_get_total_frames(video), 1);
        assert!(video_output_disconnect2(video, Some(on_frame), param));
        assert!(!video_output_active(video));
        assert_eq!(video_output_get_total_frames(video), 1);
        assert!(video_output_connect(
            video,
            ptr::null(),
            Some(on_frame),
            param
        ));
        assert_eq!(video_output_get_total_frames(video), 0);
        assert!(video_output_active(video));
    }
}

#[test]
fn null_divisor() {
    unsafe {
        assert!(video_output_create_with_frame_rate_divisor(ptr::null_mut(), 2).is_null());
        video_output_free_frame_rate_divisor(ptr::null_mut());
    }
}

#[test]
fn texture_encoder_refs_skip_the_reset() {
    let mut info = rgba_info(c"gpu", 4, 4, 2);
    let session = Session::open(&mut info, false);
    let video = session.video;
    let param = session.param();
    unsafe {
        video_output_inc_texture_frames(video);
        video_output_inc_texture_encoders(video);
        assert_eq!(video_output_get_total_frames(video), 0);
        video_output_inc_texture_frames(video);
        video_output_inc_texture_skipped_frames(video);
        assert_eq!(video_output_get_total_frames(video), 1);
        assert_eq!(video_output_get_skipped_frames(video), 1);

        assert!(video_output_connect(
            video,
            ptr::null(),
            Some(on_frame),
            param
        ));
        assert_eq!(video_output_get_total_frames(video), 1);
        assert!(video_output_disconnect2(video, Some(on_frame), param));
        assert_eq!(video_output_get_total_frames(video), 1);

        video_output_dec_texture_encoders(video);
        assert_eq!(video_output_get_skipped_frames(video), 1);
        assert_eq!(video_output_get_total_frames(video), 1);
    }
}

#[test]
fn frame_rate_divisor_view() {
    let mut info = rgba_info(c"child", 4, 4, 2);
    let session = Session::open(&mut info, false);
    let video = session.video;
    unsafe {
        let parent_time = video_output_get_frame_time(video);
        assert!(video_output_create_with_frame_rate_divisor(video, 0).is_null());
        assert!(video_output_create_with_frame_rate_divisor(video, 1).is_null());
        let child = video_output_create_with_frame_rate_divisor(video, 2);
        assert!(!child.is_null());

        let got = &*video_output_get_info(child);
        assert_eq!(got.fps_num, 30);
        assert_eq!(got.fps_den, 2);
        assert!(video_output_get_frame_rate(child) == 30.0);
        assert!(video_output_get_frame_time(child) == parent_time);
        assert_eq!(video_output_get_width(child), 4);
        assert_eq!(video_output_get_format(child), RGBA);

        assert!(video_output_connect(
            child,
            ptr::null(),
            Some(on_frame),
            session.param()
        ));
        assert!(video_output_active(child));
        let mut frame = video_frame {
            data: [ptr::null_mut(); 8],
            linesize: [0; 8],
        };
        assert!(video_output_lock_frame(child, &mut frame, 1, 77));
        *frame.data[0] = 0x55;
        video_output_unlock_frame(child);
        session.wait_hits(1);
        assert_eq!(session.gate.hits.lock().unwrap().byte0[0], 0x55);

        video_output_stop(child);
        assert!(video_output_stopped(video));
        video_output_free_frame_rate_divisor(video);
        video_output_free_frame_rate_divisor(child);
    }
}

/// `fps_den` multiplies on the node you pass, then wraps as `uint32_t`.
/// `get_frame_rate` still reads the root.
#[test]
fn grandchild_multiplies_fps_den() {
    let mut info = rgba_info(c"grand", 4, 4, 2);
    let session = Session::open(&mut info, false);
    let video = session.video;
    unsafe {
        let parent_time = video_output_get_frame_time(video);
        let child = video_output_create_with_frame_rate_divisor(video, 2);
        let grand = video_output_create_with_frame_rate_divisor(child, 3);
        assert_eq!((*video_output_get_info(grand)).fps_den, 6);
        assert!(video_output_get_frame_rate(grand) == 30.0);
        assert!(video_output_get_frame_time(grand) == parent_time);

        let wide = video_output_create_with_frame_rate_divisor(video, u32::MAX);
        let wrapped = video_output_create_with_frame_rate_divisor(wide, 2);
        assert_eq!(
            (*video_output_get_info(wrapped)).fps_den,
            u32::MAX.wrapping_mul(2)
        );

        video_output_free_frame_rate_divisor(wrapped);
        video_output_free_frame_rate_divisor(wide);
        video_output_free_frame_rate_divisor(grand);
        video_output_free_frame_rate_divisor(child);
    }
}
