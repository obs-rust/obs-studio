//! Safe core of `libobs/media-io/media-remux.c.h`: copy every stream of a
//! recording into another container without re-encoding (Remux Recordings
//! in the frontend), with audio frames kept sample accurate.
//!
//! Every libavformat call and every field of an FFmpeg struct goes through
//! [`Host`]; FFmpeg contexts are opaque [`Handle`]s (0 is NULL). The core
//! decides which calls to make, in the C's order, and does the C's packet
//! and progress arithmetic. In libobs, `ffi::media_remux` implements
//! [`Host`] over the private C adapter in `media-io/media-remux-ffmpeg.c.h`.
//!
//! Kept as the C has it, characterized and not endorsed:
//!
//! - a progress callback that returns false stops the remux, but the job
//!   still writes the trailer and reports success;
//! - a write that fails with `AVERROR_INVALIDDATA` or `EINVAL` drops the
//!   packet and continues, and is not a failure unless it is the last call;
//! - progress is the read position over the input's size at create time, so
//!   it can pass 100 and is infinite or NaN for an empty or unreadable file.
//!
//! Intentional differences (also in rust/README.md):
//!
//! - a failed `media_remux_job_create` sets `*job` to NULL (C: leaves it
//!   pointing at the job it freed);
//! - a NULL output name fails `media_remux_job_create` (C: `strcmp(NULL)`);
//! - a packet whose stream index has no stream is written unchanged (C:
//!   reads out of bounds; FFmpeg never returns one);
//! - the job and its per-stream audio clocks are Rust allocations, not
//!   `bzalloc`/`bmalloc`.

use std::ffi::CStr;

/// An opaque FFmpeg pointer (`AVFormatContext *`) as an address. 0 is NULL.
pub type Handle = usize;

/// `AVRational`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(C)]
pub struct Rational {
    pub num: i32,
    pub den: i32,
}

/// The fields of one stream's `AVStream` and `AVCodecParameters` the remux
/// reads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(C)]
pub struct StreamInfo {
    pub codec_type: i32,
    pub codec_id: i32,
    pub sample_rate: i32,
    pub frame_size: i32,
    pub channels: i32,
    pub time_base: Rational,
}

/// The fields of an `AVPacket` the remux reads and writes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(C)]
pub struct Packet {
    pub pts: i64,
    pub dts: i64,
    pub duration: i64,
    pub pos: i64,
    pub size: i32,
    pub stream_index: i32,
}

/// `AV_NOPTS_VALUE`.
pub const NOPTS_VALUE: i64 = i64::MIN;
/// `AVMEDIA_TYPE_AUDIO`.
pub const MEDIA_TYPE_AUDIO: i32 = 1;
/// `AV_CODEC_ID_HEVC`.
pub const CODEC_ID_HEVC: i32 = 173;
/// `AVFMT_NOFILE`.
pub const FMT_NOFILE: i32 = 0x0001;
/// `MKTAG('h', 'v', 'c', '1')`.
pub const TAG_HVC1: u32 = u32::from_le_bytes(*b"hvc1");
/// `AVERROR_EOF`: `-MKTAG('E', 'O', 'F', ' ')`.
pub const ERROR_EOF: i32 = -(u32::from_le_bytes(*b"EOF ") as i32);
/// `AVERROR_INVALIDDATA`: `-MKTAG('I', 'N', 'D', 'A')`.
pub const ERROR_INVALIDDATA: i32 = -(u32::from_le_bytes(*b"INDA") as i32);
/// `-EINVAL` (22 on every OS libobs builds for).
pub const ERROR_EINVAL: i32 = -22;
/// `AV_ROUND_DOWN`, `AV_ROUND_UP`, `AV_ROUND_NEAR_INF`, `AV_ROUND_PASS_MINMAX`.
pub const ROUND_UP: i32 = 3;
pub const ROUND_NEAR_INF: i32 = 5;
pub const ROUND_PASS_MINMAX: i32 = 8192;

/// `LOG_ERROR`.
pub const LOG_ERROR: i32 = 100;

/// What the remux needs from FFmpeg and libobs. Each method is one call of
/// the C, or one field it reads or writes.
pub trait Host {
    /// `os_file_exists`.
    fn file_exists(&mut self, path: &CStr) -> bool;
    /// `st_size` after `stat`/`_stat64` of a zeroed `struct stat`.
    fn file_size(&mut self, path: &CStr) -> i64;
    /// `avformat_open_input(&ctx, path, NULL, NULL)`: its result and `ctx`.
    fn open_input(&mut self, path: &CStr) -> (i32, Handle);
    /// `avformat_find_stream_info(ctx, NULL)`.
    fn find_stream_info(&mut self, input: Handle) -> i32;
    /// `av_dump_format(ctx, 0, path, is_output)` in a build without
    /// `NDEBUG`; nothing otherwise.
    fn dump_format(&mut self, ctx: Handle, path: &CStr, is_output: bool);
    /// `avformat_alloc_output_context2(&ctx, NULL, NULL, path)`: `ctx`.
    fn alloc_output(&mut self, path: &CStr) -> Handle;
    /// `ctx->nb_streams`.
    fn nb_streams(&mut self, ctx: Handle) -> u32;
    /// `ctx->streams[index]`'s fields.
    fn stream(&mut self, ctx: Handle, index: u32) -> StreamInfo;
    /// `avformat_new_stream(output, NULL) != NULL`.
    fn new_stream(&mut self, output: Handle) -> bool;
    /// `avcodec_parameters_copy` from input stream `index` to output stream
    /// `index`.
    fn copy_parameters(&mut self, input: Handle, output: Handle, index: u32) -> i32;
    /// `av_dict_copy(&out->metadata, in->metadata, 0)` for stream `index`.
    fn copy_metadata(&mut self, input: Handle, output: Handle, index: u32);
    /// `output->oformat->codec_tag != NULL`.
    fn has_codec_tags(&mut self, output: Handle) -> bool;
    /// `av_codec_get_id(output->oformat->codec_tag, tag)`.
    fn codec_id_for_tag(&mut self, output: Handle, tag: u32) -> i32;
    /// `codecpar->codec_tag = tag` of output stream `index`.
    fn set_codec_tag(&mut self, output: Handle, index: u32, tag: u32);
    /// `av_channel_layout_default(&codecpar->ch_layout, channels)` of
    /// output stream `index`.
    fn default_channel_layout(&mut self, output: Handle, index: u32, channels: i32);
    /// `codecpar->ch_layout = AV_CHANNEL_LAYOUT_4POINT1` of output stream
    /// `index`.
    fn set_layout_4point1(&mut self, output: Handle, index: u32);
    /// `output->oformat->flags`.
    fn output_flags(&mut self, output: Handle) -> i32;
    /// `avio_open(&output->pb, path, AVIO_FLAG_WRITE)`.
    fn avio_open(&mut self, output: Handle, path: &CStr) -> i32;
    /// `avformat_write_header(output, NULL)`.
    fn write_header(&mut self, output: Handle) -> i32;
    /// `av_read_frame(input, &pkt)`: its result and the packet.
    fn read_frame(&mut self, input: Handle) -> (i32, Packet);
    /// The packet's fields written back, `av_interleaved_write_frame`, then
    /// `av_packet_unref`; the write's result.
    fn write_frame(&mut self, output: Handle, packet: &Packet) -> i32;
    /// `av_packet_unref` of the packet just read, when the progress callback
    /// stops the job before the packet is written (#3365).
    fn packet_unref(&mut self);
    /// `av_write_trailer(output)`.
    fn write_trailer(&mut self, output: Handle) -> i32;
    /// `avformat_close_input(&input)` (NULL included).
    fn close_input(&mut self, input: Handle);
    /// `avio_close(output->pb)`.
    fn avio_close(&mut self, output: Handle);
    /// `avformat_free_context(output)` (NULL included).
    fn free_context(&mut self, output: Handle);
    /// `av_get_audio_frame_duration2(input stream index's codecpar, size)`.
    fn audio_frame_duration(&mut self, input: Handle, index: u32, size: i32) -> i32;
    /// `av_rescale_q_rnd`.
    fn rescale_q_rnd(&mut self, a: i64, bq: Rational, cq: Rational, rnd: i32) -> i64;
    /// `av_rescale_q`.
    fn rescale_q(&mut self, a: i64, bq: Rational, cq: Rational) -> i64;
    /// `av_rescale_delta`.
    fn rescale_delta(
        &mut self,
        in_tb: Rational,
        in_ts: i64,
        fs_tb: Rational,
        duration: i32,
        last: &mut i64,
        out_tb: Rational,
    ) -> i64;
    /// `av_err2str(err)`.
    fn error_string(&mut self, err: i32) -> String;
    /// `blog(level, "%s", message)`.
    fn log(&mut self, level: i32, message: &str);
}

/// `struct media_remux_job`.
#[derive(Debug, Default)]
pub struct Job {
    pub in_size: i64,
    pub input: Handle,
    pub output: Handle,
    /// Per input stream: where the next audio frame starts, in samples.
    /// `None` for an input without streams, as the C leaves it NULL.
    pub next_audio_ts: Option<Vec<i64>>,
}

impl Job {
    /// `media_remux_job_create`: `None` where the C returns false. A job
    /// that fails after it was allocated has already been destroyed.
    pub fn create<H: Host>(
        host: &mut H,
        in_filename: &CStr,
        out_filename: Option<&CStr>,
    ) -> Option<Job> {
        if !host.file_exists(in_filename) {
            return None;
        }
        let out_filename = out_filename?;
        if in_filename == out_filename {
            return None;
        }

        let mut job = Job {
            in_size: host.file_size(in_filename),
            ..Job::default()
        };
        if job.init_input(host, in_filename) && job.init_output(host, out_filename) {
            Some(job)
        } else {
            job.destroy(host);
            None
        }
    }

    fn init_input<H: Host>(&mut self, host: &mut H, in_filename: &CStr) -> bool {
        let (ret, input) = host.open_input(in_filename);
        self.input = input;
        if ret < 0 {
            let name = in_filename.to_string_lossy();
            host.log(
                LOG_ERROR,
                &format!("media_remux: Could not open input file '{name}'"),
            );
            return false;
        }

        if host.find_stream_info(self.input) < 0 {
            host.log(
                LOG_ERROR,
                "media_remux: Failed to retrieve input stream information",
            );
            return false;
        }

        host.dump_format(self.input, in_filename, false);
        true
    }

    fn init_output<H: Host>(&mut self, host: &mut H, out_filename: &CStr) -> bool {
        self.output = host.alloc_output(out_filename);
        if self.output == 0 {
            host.log(LOG_ERROR, "media_remux: Could not create output context");
            return false;
        }

        let nb_streams = host.nb_streams(self.input);
        if nb_streams != 0 {
            self.next_audio_ts = Some(vec![NOPTS_VALUE; nb_streams as usize]);
        }

        for i in 0..nb_streams {
            if !host.new_stream(self.output) {
                host.log(LOG_ERROR, "media_remux: Failed to allocate output stream");
                return false;
            }

            if host.copy_parameters(self.input, self.output, i) < 0 {
                host.log(LOG_ERROR, "media_remux: Failed to copy parameters");
                return false;
            }

            host.copy_metadata(self.input, self.output, i);

            let in_stream = host.stream(self.input, i);
            // Tag HEVC with the industry standard hvc1 when the muxer
            // supports it, for wider device compatibility; otherwise 0, so
            // FFmpeg picks the tag.
            let hvc1 = in_stream.codec_id == CODEC_ID_HEVC
                && host.has_codec_tags(self.output)
                && host.codec_id_for_tag(self.output, TAG_HVC1)
                    == host.stream(self.output, i).codec_id;
            host.set_codec_tag(self.output, i, if hvc1 { TAG_HVC1 } else { 0 });

            if in_stream.codec_type == MEDIA_TYPE_AUDIO {
                host.default_channel_layout(self.output, i, in_stream.channels);
                // The default layout for 5 channels is 5.0, which OBS does
                // not support: 5 channels are 4.1.
                if in_stream.channels == 5 {
                    host.set_layout_4point1(self.output, i);
                }
            }
        }

        host.dump_format(self.output, out_filename, true);

        if host.output_flags(self.output) & FMT_NOFILE == 0
            && host.avio_open(self.output, out_filename) < 0
        {
            let name = out_filename.to_string_lossy();
            host.log(
                LOG_ERROR,
                &format!("media_remux: Failed to open output file '{name}'"),
            );
            return false;
        }

        true
    }

    /// `process_audio_packet`: audio frames have a fixed number of
    /// samples, but a container with a coarse time base (Matroska stores
    /// milliseconds) cannot place them exactly. Like `ffmpeg -c copy`, each
    /// audio frame goes right after the previous one while that is within
    /// the input's rounding of its timestamp, and follows the input only
    /// when it really jumps.
    fn process_audio_packet<H: Host>(
        &mut self,
        host: &mut H,
        pkt: &mut Packet,
        index: u32,
        in_stream: &StreamInfo,
        out_tb: Rational,
    ) -> bool {
        let Some(next_audio_ts) = self.next_audio_ts.as_mut() else {
            return false;
        };
        if in_stream.codec_type != MEDIA_TYPE_AUDIO
            || in_stream.sample_rate <= 0
            || pkt.dts == NOPTS_VALUE
        {
            return false;
        }

        let mut samples = host.audio_frame_duration(self.input, index, pkt.size);
        if samples <= 0 {
            samples = in_stream.frame_size;
        }
        if samples <= 0 {
            return false;
        }

        let sample_tb = Rational {
            num: 1,
            den: in_stream.sample_rate,
        };
        let in_tb = in_stream.time_base;
        let dts = host.rescale_delta(
            in_tb,
            pkt.dts,
            sample_tb,
            samples,
            &mut next_audio_ts[index as usize],
            out_tb,
        );

        if pkt.pts != NOPTS_VALUE {
            pkt.pts =
                dts.wrapping_add(host.rescale_q(pkt.pts.wrapping_sub(pkt.dts), in_tb, out_tb));
        }
        pkt.dts = dts;

        // Keep a shorter last frame (trimmed padding) but drop the input's
        // rounding from full frames.
        let mut duration = i64::from(samples);
        if pkt.duration > 0 {
            let in_duration = host.rescale_q(pkt.duration, in_tb, sample_tb);
            let in_tick = host.rescale_q_rnd(1, in_tb, sample_tb, ROUND_UP);
            if in_duration < i64::from(samples).wrapping_sub(in_tick) {
                duration = in_duration;
            }
        }
        pkt.duration = host.rescale_q(duration, sample_tb, out_tb);
        pkt.pos = -1;
        true
    }

    fn process_packet<H: Host>(&mut self, host: &mut H, pkt: &mut Packet) {
        let Ok(index) = u32::try_from(pkt.stream_index) else {
            return;
        };
        if index >= host.nb_streams(self.input) || index >= host.nb_streams(self.output) {
            return;
        }
        let in_stream = host.stream(self.input, index);
        let out_tb = host.stream(self.output, index).time_base;

        if self.process_audio_packet(host, pkt, index, &in_stream, out_tb) {
            return;
        }

        let in_tb = in_stream.time_base;
        let rnd = ROUND_NEAR_INF | ROUND_PASS_MINMAX;
        pkt.pts = host.rescale_q_rnd(pkt.pts, in_tb, out_tb, rnd);
        pkt.dts = host.rescale_q_rnd(pkt.dts, in_tb, out_tb, rnd);
        // The C stores the result through an `(int)` cast.
        pkt.duration = i64::from(host.rescale_q(pkt.duration, in_tb, out_tb) as i32);
        pkt.pos = -1;
    }

    fn process_packets<H: Host>(
        &mut self,
        host: &mut H,
        callback: &mut Option<&mut dyn FnMut(f32) -> bool>,
    ) -> i32 {
        let mut throttle = 0;
        loop {
            let (ret, mut pkt) = host.read_frame(self.input);
            if ret < 0 {
                if ret != ERROR_EOF {
                    let err = host.error_string(ret);
                    host.log(
                        LOG_ERROR,
                        &format!("media_remux: Error reading packet: {err}"),
                    );
                }
                return ret;
            }

            if let Some(callback) = callback.as_mut() {
                let throttled = throttle > 10;
                throttle += 1;
                if throttled {
                    let progress = pkt.pos as f32 / self.in_size as f32 * 100.0;
                    if !callback(progress) {
                        // Never written: unref it here (#3365).
                        host.packet_unref();
                        return ret;
                    }
                    throttle = 0;
                }
            }

            self.process_packet(host, &mut pkt);

            let ret = host.write_frame(self.output, &pkt);
            if ret < 0 {
                let err = host.error_string(ret);
                host.log(
                    LOG_ERROR,
                    &format!("media_remux: Error muxing packet: {err}"),
                );

                // "Invalid data found when processing input" and "Invalid
                // argument" are not fatal.
                if ret == ERROR_INVALIDDATA || ret == ERROR_EINVAL {
                    continue;
                }
                return ret;
            }
        }
    }

    /// `media_remux_job_process`.
    pub fn process<H: Host>(
        &mut self,
        host: &mut H,
        mut callback: Option<&mut dyn FnMut(f32) -> bool>,
    ) -> bool {
        let ret = host.write_header(self.output);
        if ret < 0 {
            let err = host.error_string(ret);
            host.log(
                LOG_ERROR,
                &format!("media_remux: Error opening output file: {err}"),
            );
            return false;
        }

        if let Some(callback) = callback.as_mut() {
            callback(0.0);
        }

        let ret = self.process_packets(host, &mut callback);
        let mut success = ret >= 0 || ret == ERROR_EOF;

        let ret = host.write_trailer(self.output);
        if ret < 0 {
            let err = host.error_string(ret);
            host.log(LOG_ERROR, &format!("media_remux: av_write_trailer: {err}"));
            success = false;
        }

        if let Some(callback) = callback.as_mut() {
            callback(100.0);
        }

        success
    }

    /// `media_remux_job_destroy`.
    pub fn destroy<H: Host>(self, host: &mut H) {
        host.close_input(self.input);
        if self.output != 0 && host.output_flags(self.output) & FMT_NOFILE == 0 {
            host.avio_close(self.output);
        }
        host.free_context(self.output);
    }
}
