//! C ABI for `libobs/media-io/media-remux.c.h` (`media-io/media-remux.h`).
//!
//! `media_remux_job_t` is a Rust [`Job`] behind a `Box`. Every FFmpeg call
//! and struct field goes through the private C adapter
//! `libobs/media-io/media-remux-ffmpeg.c.h` (`obs_media_remux_*`, hidden),
//! which libobs builds and links with FFmpeg; cargo tests get the same
//! adapter over a recording FFmpeg stand-in from the oracle crate.

use core::ffi::{CStr, c_char, c_int, c_void};
use core::ptr;

use crate::media_remux::{Handle, Host, Job, Packet, Rational, StreamInfo};

unsafe extern "C" {
    fn os_file_exists(path: *const c_char) -> bool;
    fn os_get_file_size(path: *const c_char) -> i64;

    fn obs_media_remux_open_input(input: *mut *mut c_void, path: *const c_char) -> c_int;
    fn obs_media_remux_find_stream_info(input: *mut c_void) -> c_int;
    fn obs_media_remux_dump_format(ctx: *mut c_void, path: *const c_char, is_output: bool);
    fn obs_media_remux_alloc_output(path: *const c_char) -> *mut c_void;
    fn obs_media_remux_nb_streams(ctx: *const c_void) -> u32;
    fn obs_media_remux_stream(ctx: *const c_void, index: u32, out: *mut StreamInfo);
    fn obs_media_remux_new_stream(output: *mut c_void) -> bool;
    fn obs_media_remux_copy_parameters(
        input: *const c_void,
        output: *mut c_void,
        index: u32,
    ) -> c_int;
    fn obs_media_remux_copy_metadata(input: *const c_void, output: *mut c_void, index: u32);
    fn obs_media_remux_has_codec_tags(output: *const c_void) -> bool;
    fn obs_media_remux_codec_id_for_tag(output: *const c_void, tag: u32) -> i32;
    fn obs_media_remux_set_codec_tag(output: *mut c_void, index: u32, tag: u32);
    fn obs_media_remux_default_channel_layout(output: *mut c_void, index: u32, channels: c_int);
    fn obs_media_remux_set_layout_4point1(output: *mut c_void, index: u32);
    fn obs_media_remux_output_flags(output: *const c_void) -> c_int;
    fn obs_media_remux_avio_open(output: *mut c_void, path: *const c_char) -> c_int;
    fn obs_media_remux_write_header(output: *mut c_void) -> c_int;
    fn obs_media_remux_packet_new() -> *mut c_void;
    fn obs_media_remux_packet_free(pkt: *mut c_void);
    fn obs_media_remux_packet_unref(pkt: *mut c_void);
    fn obs_media_remux_read_frame(input: *mut c_void, pkt: *mut c_void, out: *mut Packet) -> c_int;
    fn obs_media_remux_write_frame(
        output: *mut c_void,
        pkt: *mut c_void,
        packet: *const Packet,
    ) -> c_int;
    fn obs_media_remux_write_trailer(output: *mut c_void) -> c_int;
    fn obs_media_remux_close_input(input: *mut c_void);
    fn obs_media_remux_avio_close(output: *mut c_void);
    fn obs_media_remux_free_context(output: *mut c_void);
    fn obs_media_remux_audio_frame_duration(input: *const c_void, index: u32, size: c_int)
    -> c_int;
    fn obs_media_remux_rescale_q_rnd(a: i64, bq: Rational, cq: Rational, rnd: c_int) -> i64;
    fn obs_media_remux_rescale_q(a: i64, bq: Rational, cq: Rational) -> i64;
    fn obs_media_remux_rescale_delta(
        in_tb: Rational,
        in_ts: i64,
        fs_tb: Rational,
        duration: c_int,
        last: *mut i64,
        out_tb: Rational,
    ) -> i64;
    fn obs_media_remux_error_string(err: c_int, buf: *mut c_char, size: usize);
    fn obs_media_remux_log(level: c_int, line: *const c_char);
}

/// `AV_ERROR_MAX_STRING_SIZE`.
const ERROR_MAX_STRING_SIZE: usize = 64;

fn ctx(handle: Handle) -> *mut c_void {
    handle as *mut c_void
}

/// [`Host`] over the C adapter. `packet` is the `AVPacket` of one
/// [`Job::process`], null outside it.
struct Adapter {
    packet: *mut c_void,
}

impl Host for Adapter {
    fn file_exists(&mut self, path: &CStr) -> bool {
        // SAFETY: a valid string.
        unsafe { os_file_exists(path.as_ptr()) }
    }

    fn file_size(&mut self, path: &CStr) -> i64 {
        // The C reads `st_size` of a zeroed `struct stat`: 0 when `stat`
        // fails. `os_get_file_size` returns -1 then, and opens the path as
        // UTF-8 on Windows, where the C's `_stat64` took the ANSI code page.
        // SAFETY: a valid string.
        unsafe { os_get_file_size(path.as_ptr()) }.max(0)
    }

    fn open_input(&mut self, path: &CStr) -> (i32, Handle) {
        let mut input = ptr::null_mut();
        // SAFETY: a valid out-pointer and string.
        let ret = unsafe { obs_media_remux_open_input(&mut input, path.as_ptr()) };
        (ret, input as Handle)
    }

    fn find_stream_info(&mut self, input: Handle) -> i32 {
        // SAFETY: an open input context.
        unsafe { obs_media_remux_find_stream_info(ctx(input)) }
    }

    fn dump_format(&mut self, handle: Handle, path: &CStr, is_output: bool) {
        // SAFETY: a context and a valid string.
        unsafe { obs_media_remux_dump_format(ctx(handle), path.as_ptr(), is_output) }
    }

    fn alloc_output(&mut self, path: &CStr) -> Handle {
        // SAFETY: a valid string.
        unsafe { obs_media_remux_alloc_output(path.as_ptr()) as Handle }
    }

    fn nb_streams(&mut self, handle: Handle) -> u32 {
        // SAFETY: a context or NULL.
        unsafe { obs_media_remux_nb_streams(ctx(handle)) }
    }

    fn stream(&mut self, handle: Handle, index: u32) -> StreamInfo {
        let mut info = StreamInfo::default();
        // SAFETY: the core passes only indexes below `nb_streams`.
        unsafe { obs_media_remux_stream(ctx(handle), index, &mut info) };
        info
    }

    fn new_stream(&mut self, output: Handle) -> bool {
        // SAFETY: an output context.
        unsafe { obs_media_remux_new_stream(ctx(output)) }
    }

    fn copy_parameters(&mut self, input: Handle, output: Handle, index: u32) -> i32 {
        // SAFETY: stream `index` exists in both.
        unsafe { obs_media_remux_copy_parameters(ctx(input), ctx(output), index) }
    }

    fn copy_metadata(&mut self, input: Handle, output: Handle, index: u32) {
        // SAFETY: stream `index` exists in both.
        unsafe { obs_media_remux_copy_metadata(ctx(input), ctx(output), index) }
    }

    fn has_codec_tags(&mut self, output: Handle) -> bool {
        // SAFETY: an output context.
        unsafe { obs_media_remux_has_codec_tags(ctx(output)) }
    }

    fn codec_id_for_tag(&mut self, output: Handle, tag: u32) -> i32 {
        // SAFETY: an output context.
        unsafe { obs_media_remux_codec_id_for_tag(ctx(output), tag) }
    }

    fn set_codec_tag(&mut self, output: Handle, index: u32, tag: u32) {
        // SAFETY: output stream `index` exists.
        unsafe { obs_media_remux_set_codec_tag(ctx(output), index, tag) }
    }

    fn default_channel_layout(&mut self, output: Handle, index: u32, channels: i32) {
        // SAFETY: output stream `index` exists.
        unsafe { obs_media_remux_default_channel_layout(ctx(output), index, channels) }
    }

    fn set_layout_4point1(&mut self, output: Handle, index: u32) {
        // SAFETY: output stream `index` exists.
        unsafe { obs_media_remux_set_layout_4point1(ctx(output), index) }
    }

    fn output_flags(&mut self, output: Handle) -> i32 {
        // SAFETY: an output context.
        unsafe { obs_media_remux_output_flags(ctx(output)) }
    }

    fn avio_open(&mut self, output: Handle, path: &CStr) -> i32 {
        // SAFETY: an output context and a valid string.
        unsafe { obs_media_remux_avio_open(ctx(output), path.as_ptr()) }
    }

    fn write_header(&mut self, output: Handle) -> i32 {
        // SAFETY: an output context.
        unsafe { obs_media_remux_write_header(ctx(output)) }
    }

    fn read_frame(&mut self, input: Handle) -> (i32, Packet) {
        let mut packet = Packet::default();
        // SAFETY: an input context and the packet of this process call.
        let ret = unsafe { obs_media_remux_read_frame(ctx(input), self.packet, &mut packet) };
        (ret, packet)
    }

    fn write_frame(&mut self, output: Handle, packet: &Packet) -> i32 {
        // SAFETY: an output context and the packet just read into.
        unsafe { obs_media_remux_write_frame(ctx(output), self.packet, packet) }
    }

    fn packet_unref(&mut self) {
        // SAFETY: the packet of this process call.
        unsafe { obs_media_remux_packet_unref(self.packet) }
    }

    fn write_trailer(&mut self, output: Handle) -> i32 {
        // SAFETY: an output context.
        unsafe { obs_media_remux_write_trailer(ctx(output)) }
    }

    fn close_input(&mut self, input: Handle) {
        // SAFETY: an input context or NULL.
        unsafe { obs_media_remux_close_input(ctx(input)) }
    }

    fn avio_close(&mut self, output: Handle) {
        // SAFETY: an output context.
        unsafe { obs_media_remux_avio_close(ctx(output)) }
    }

    fn free_context(&mut self, output: Handle) {
        // SAFETY: an output context or NULL.
        unsafe { obs_media_remux_free_context(ctx(output)) }
    }

    fn audio_frame_duration(&mut self, input: Handle, index: u32, size: i32) -> i32 {
        // SAFETY: input stream `index` exists.
        unsafe { obs_media_remux_audio_frame_duration(ctx(input), index, size) }
    }

    fn rescale_q_rnd(&mut self, a: i64, bq: Rational, cq: Rational, rnd: i32) -> i64 {
        // SAFETY: plain values.
        unsafe { obs_media_remux_rescale_q_rnd(a, bq, cq, rnd) }
    }

    fn rescale_q(&mut self, a: i64, bq: Rational, cq: Rational) -> i64 {
        // SAFETY: plain values.
        unsafe { obs_media_remux_rescale_q(a, bq, cq) }
    }

    fn rescale_delta(
        &mut self,
        in_tb: Rational,
        in_ts: i64,
        fs_tb: Rational,
        duration: i32,
        last: &mut i64,
        out_tb: Rational,
    ) -> i64 {
        // SAFETY: plain values and a valid pointer.
        unsafe { obs_media_remux_rescale_delta(in_tb, in_ts, fs_tb, duration, last, out_tb) }
    }

    fn error_string(&mut self, err: i32) -> String {
        let mut buf = [0 as c_char; ERROR_MAX_STRING_SIZE];
        // SAFETY: a buffer of the size passed.
        unsafe { obs_media_remux_error_string(err, buf.as_mut_ptr(), buf.len()) };
        buf[ERROR_MAX_STRING_SIZE - 1] = 0;
        // SAFETY: terminated above.
        unsafe { CStr::from_ptr(buf.as_ptr()) }
            .to_string_lossy()
            .into_owned()
    }

    fn log(&mut self, level: i32, message: &str) {
        let line: Vec<u8> = message.bytes().filter(|&b| b != 0).chain([0]).collect();
        // SAFETY: a NUL-terminated string.
        unsafe { obs_media_remux_log(level, line.as_ptr().cast()) }
    }
}

/// `media_remux_job_t`.
#[allow(non_camel_case_types)]
pub type media_remux_job_t = *mut Job;

/// `media_remux_progress_callback`.
#[allow(non_camel_case_types)]
pub type media_remux_progress_callback =
    unsafe extern "C" fn(data: *mut c_void, percent: f32) -> bool;

/// # Safety
///
/// `job` is NULL or writable; the names are NULL or valid strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn media_remux_job_create(
    job: *mut media_remux_job_t,
    in_filename: *const c_char,
    out_filename: *const c_char,
) -> bool {
    if job.is_null() {
        return false;
    }
    // SAFETY: checked above.
    unsafe { *job = ptr::null_mut() };
    if in_filename.is_null() {
        return false;
    }
    // SAFETY: the caller's strings.
    let in_filename = unsafe { CStr::from_ptr(in_filename) };
    let out_filename = (!out_filename.is_null()).then(|| unsafe { CStr::from_ptr(out_filename) });

    let mut adapter = Adapter {
        packet: ptr::null_mut(),
    };
    match Job::create(&mut adapter, in_filename, out_filename) {
        Some(created) => {
            // SAFETY: checked above.
            unsafe { *job = Box::into_raw(Box::new(created)) };
            true
        }
        None => false,
    }
}

/// # Safety
///
/// `job` is NULL or from `media_remux_job_create`; `callback` is called
/// with `data`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn media_remux_job_process(
    job: media_remux_job_t,
    callback: Option<media_remux_progress_callback>,
    data: *mut c_void,
) -> bool {
    // SAFETY: the caller's job.
    let Some(job) = (unsafe { job.as_mut() }) else {
        return false;
    };
    // SAFETY: allocates a zeroed AVPacket.
    let packet = unsafe { obs_media_remux_packet_new() };
    let mut adapter = Adapter { packet };
    let mut progress = |percent: f32| -> bool {
        // SAFETY: the caller's callback and data.
        callback.is_some_and(|callback| unsafe { callback(data, percent) })
    };
    let ok = job.process(
        &mut adapter,
        callback
            .is_some()
            .then_some(&mut progress as &mut dyn FnMut(f32) -> bool),
    );
    // SAFETY: from obs_media_remux_packet_new.
    unsafe { obs_media_remux_packet_free(packet) };
    ok
}

/// # Safety
///
/// `job` is NULL or from `media_remux_job_create`, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn media_remux_job_destroy(job: media_remux_job_t) {
    if job.is_null() {
        return;
    }
    // SAFETY: from Box::into_raw in media_remux_job_create.
    let job = unsafe { Box::from_raw(job) };
    let mut adapter = Adapter {
        packet: ptr::null_mut(),
    };
    job.destroy(&mut adapter);
}
