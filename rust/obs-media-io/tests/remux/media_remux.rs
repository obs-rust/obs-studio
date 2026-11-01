//! Tier 1: the media-remux core over a scripted [`Host`]. The cases mirror
//! `test/cmocka/test_media_remux.c` (refused jobs, layouts, progress) and
//! `test_remux_audio_timestamps.c` (sample-accurate audio), plus the error
//! paths FFmpeg rarely takes.

use std::ffi::CStr;

use obs_media_io::media_remux::{
    CODEC_ID_HEVC, ERROR_EINVAL, ERROR_EOF, ERROR_INVALIDDATA, FMT_NOFILE, Handle, Host, Job,
    MEDIA_TYPE_AUDIO, NOPTS_VALUE, Packet, Rational, StreamInfo, TAG_HVC1,
};

const INPUT: Handle = 0x100;
const OUTPUT: Handle = 0x200;
const VIDEO: i32 = 0;
const AAC: i32 = 86018;
const H264: i32 = 27;

/// `av_rescale_rnd` for the rounding modes the remux uses, on i128.
fn rescale_rnd(a: i64, b: i64, c: i64, rnd: i32) -> i64 {
    if rnd & 8192 != 0 && (a == i64::MIN || a == i64::MAX) {
        return a;
    }
    let rnd = rnd & !8192;
    if a < 0 {
        let flipped = rnd ^ ((rnd >> 1) & 1);
        return rescale_rnd(-(a.max(-i64::MAX)), b, c, flipped).wrapping_neg();
    }
    let r: i128 = match rnd {
        5 => i128::from(c / 2),
        1 | 3 => i128::from(c - 1),
        _ => 0,
    };
    let v = (i128::from(a) * i128::from(b) + r) / i128::from(c);
    i64::try_from(v).unwrap_or(i64::MIN)
}

fn rescale_q_rnd(a: i64, bq: Rational, cq: Rational, rnd: i32) -> i64 {
    rescale_rnd(
        a,
        i64::from(bq.num) * i64::from(cq.den),
        i64::from(cq.num) * i64::from(bq.den),
        rnd,
    )
}

#[derive(Debug, Clone)]
struct Script {
    exists: bool,
    size: i64,
    open_ret: i32,
    find_info_ret: i32,
    alloc_ok: bool,
    streams: Vec<StreamInfo>,
    out_time_bases: Vec<Rational>,
    new_stream_fail_at: Option<u32>,
    copy_ret: i32,
    flags: i32,
    has_tags: bool,
    tag_codec_id: i32,
    avio_ret: i32,
    header_ret: i32,
    trailer_ret: i32,
    packets: Vec<(Packet, i32)>,
    read_end_ret: i32,
    frame_duration: i32,
}

impl Default for Script {
    fn default() -> Self {
        Self {
            exists: true,
            size: 1000,
            open_ret: 0,
            find_info_ret: 0,
            alloc_ok: true,
            streams: vec![audio(48000, 1024, 2, ms())],
            out_time_bases: vec![Rational { num: 1, den: 48000 }],
            new_stream_fail_at: None,
            copy_ret: 0,
            flags: 0,
            has_tags: true,
            tag_codec_id: CODEC_ID_HEVC,
            avio_ret: 0,
            header_ret: 0,
            trailer_ret: 0,
            packets: Vec::new(),
            read_end_ret: ERROR_EOF,
            frame_duration: 1024,
        }
    }
}

fn ms() -> Rational {
    Rational { num: 1, den: 1000 }
}

fn audio(rate: i32, frame_size: i32, channels: i32, time_base: Rational) -> StreamInfo {
    StreamInfo {
        codec_type: MEDIA_TYPE_AUDIO,
        codec_id: AAC,
        sample_rate: rate,
        frame_size,
        channels,
        time_base,
    }
}

fn video(codec_id: i32) -> StreamInfo {
    StreamInfo {
        codec_type: VIDEO,
        codec_id,
        sample_rate: 0,
        frame_size: 0,
        channels: 0,
        time_base: ms(),
    }
}

fn packet(stream_index: i32, dts: i64, duration: i64, pos: i64) -> Packet {
    Packet {
        pts: dts,
        dts,
        duration,
        pos,
        size: 100,
        stream_index,
    }
}

/// A scripted FFmpeg that logs every call.
struct Fake {
    script: Script,
    log: Vec<String>,
    out_streams: u32,
    next: usize,
    written: Vec<Packet>,
    live: i32,
}

impl Fake {
    fn new(script: Script) -> Self {
        Self {
            script,
            log: Vec::new(),
            out_streams: 0,
            next: 0,
            written: Vec::new(),
            live: 0,
        }
    }

    fn has(&self, prefix: &str) -> bool {
        self.log.iter().any(|l| l.starts_with(prefix))
    }

    fn errors(&self) -> Vec<&str> {
        self.log
            .iter()
            .filter_map(|l| l.strip_prefix("log 100 "))
            .collect()
    }
}

impl Host for Fake {
    fn file_exists(&mut self, _: &CStr) -> bool {
        self.script.exists
    }
    fn file_size(&mut self, _: &CStr) -> i64 {
        self.script.size
    }
    fn open_input(&mut self, path: &CStr) -> (i32, Handle) {
        self.log
            .push(format!("open_input {}", path.to_string_lossy()));
        if self.script.open_ret < 0 {
            return (self.script.open_ret, 0);
        }
        self.live += 1;
        (self.script.open_ret, INPUT)
    }
    fn find_stream_info(&mut self, input: Handle) -> i32 {
        assert_eq!(input, INPUT);
        self.log.push("find_stream_info".into());
        self.script.find_info_ret
    }
    fn dump_format(&mut self, _: Handle, _: &CStr, _: bool) {}
    fn alloc_output(&mut self, path: &CStr) -> Handle {
        self.log
            .push(format!("alloc_output {}", path.to_string_lossy()));
        if self.script.alloc_ok {
            self.live += 1;
            OUTPUT
        } else {
            0
        }
    }
    fn nb_streams(&mut self, ctx: Handle) -> u32 {
        if ctx == INPUT {
            self.script.streams.len() as u32
        } else {
            self.out_streams
        }
    }
    fn stream(&mut self, ctx: Handle, index: u32) -> StreamInfo {
        let i = index as usize;
        if ctx == INPUT {
            self.script.streams[i]
        } else {
            StreamInfo {
                time_base: self
                    .script
                    .out_time_bases
                    .get(i)
                    .copied()
                    .unwrap_or_default(),
                ..self.script.streams[i]
            }
        }
    }
    fn new_stream(&mut self, output: Handle) -> bool {
        assert_eq!(output, OUTPUT);
        self.log.push(format!("new_stream {}", self.out_streams));
        if self.script.new_stream_fail_at == Some(self.out_streams) {
            return false;
        }
        self.out_streams += 1;
        true
    }
    fn copy_parameters(&mut self, _: Handle, _: Handle, index: u32) -> i32 {
        self.log.push(format!("copy_parameters {index}"));
        self.script.copy_ret
    }
    fn copy_metadata(&mut self, _: Handle, _: Handle, index: u32) {
        self.log.push(format!("copy_metadata {index}"));
    }
    fn has_codec_tags(&mut self, _: Handle) -> bool {
        self.log.push("has_codec_tags".into());
        self.script.has_tags
    }
    fn codec_id_for_tag(&mut self, _: Handle, tag: u32) -> i32 {
        self.log.push(format!("codec_id_for_tag {tag:08x}"));
        self.script.tag_codec_id
    }
    fn set_codec_tag(&mut self, _: Handle, index: u32, tag: u32) {
        self.log.push(format!("set_codec_tag {index} {tag:08x}"));
    }
    fn default_channel_layout(&mut self, _: Handle, index: u32, channels: i32) {
        self.log
            .push(format!("default_channel_layout {index} {channels}"));
    }
    fn set_layout_4point1(&mut self, _: Handle, index: u32) {
        self.log.push(format!("set_layout_4point1 {index}"));
    }
    fn output_flags(&mut self, _: Handle) -> i32 {
        self.script.flags
    }
    fn avio_open(&mut self, _: Handle, path: &CStr) -> i32 {
        self.log
            .push(format!("avio_open {}", path.to_string_lossy()));
        self.script.avio_ret
    }
    fn write_header(&mut self, _: Handle) -> i32 {
        self.log.push("write_header".into());
        self.script.header_ret
    }
    fn read_frame(&mut self, input: Handle) -> (i32, Packet) {
        assert_eq!(input, INPUT);
        match self.script.packets.get(self.next) {
            Some(&(packet, _)) => {
                self.next += 1;
                self.log.push(format!("read_frame {}", self.next - 1));
                (0, packet)
            }
            None => {
                self.log
                    .push(format!("read_frame {}", self.script.read_end_ret));
                (self.script.read_end_ret, Packet::default())
            }
        }
    }
    fn write_frame(&mut self, output: Handle, packet: &Packet) -> i32 {
        assert_eq!(output, OUTPUT);
        self.log.push(format!(
            "write_frame {} pts={} dts={} duration={} pos={}",
            packet.stream_index, packet.pts, packet.dts, packet.duration, packet.pos
        ));
        self.written.push(*packet);
        self.script.packets[self.next - 1].1
    }
    fn packet_unref(&mut self) {
        self.log.push("packet_unref".into());
    }
    fn write_trailer(&mut self, _: Handle) -> i32 {
        self.log.push("write_trailer".into());
        self.script.trailer_ret
    }
    fn close_input(&mut self, input: Handle) {
        self.log.push(format!("close_input {}", input != 0));
        if input != 0 {
            self.live -= 1;
        }
    }
    fn avio_close(&mut self, _: Handle) {
        self.log.push("avio_close".into());
    }
    fn free_context(&mut self, output: Handle) {
        self.log.push(format!("free_context {}", output != 0));
        if output != 0 {
            self.live -= 1;
        }
    }
    fn audio_frame_duration(&mut self, _: Handle, index: u32, size: i32) -> i32 {
        self.log
            .push(format!("audio_frame_duration {index} {size}"));
        self.script.frame_duration
    }
    fn rescale_q_rnd(&mut self, a: i64, bq: Rational, cq: Rational, rnd: i32) -> i64 {
        rescale_q_rnd(a, bq, cq, rnd)
    }
    fn rescale_q(&mut self, a: i64, bq: Rational, cq: Rational) -> i64 {
        rescale_q_rnd(a, bq, cq, 5)
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
        // libavutil/mathematics.c
        let simple = |last: &mut i64| {
            *last = rescale_q_rnd(in_ts, in_tb, fs_tb, 5) + i64::from(duration);
            rescale_q_rnd(in_ts, in_tb, out_tb, 5)
        };
        if *last == NOPTS_VALUE
            || duration == 0
            || i64::from(in_tb.num) * i64::from(out_tb.den)
                <= i64::from(out_tb.num) * i64::from(in_tb.den)
        {
            return simple(last);
        }
        let a = rescale_q_rnd(2 * in_ts - 1, in_tb, fs_tb, 2) >> 1;
        let b = (rescale_q_rnd(2 * in_ts + 1, in_tb, fs_tb, 3) + 1) >> 1;
        if *last < 2 * a - b || *last > 2 * b - a {
            return simple(last);
        }
        let this = (*last).clamp(a, b);
        *last = this + i64::from(duration);
        rescale_q_rnd(this, fs_tb, out_tb, 5)
    }
    fn error_string(&mut self, err: i32) -> String {
        format!("error {err}")
    }
    fn log(&mut self, level: i32, message: &str) {
        self.log.push(format!("log {level} {message}"));
    }
}

fn create(fake: &mut Fake, input: &CStr, output: Option<&CStr>) -> Option<Job> {
    Job::create(fake, input, output)
}

fn run(script: Script, progress: Option<&mut Vec<f32>>, stop_at: Option<usize>) -> (bool, Fake) {
    let mut fake = Fake::new(script);
    let mut job = create(&mut fake, c"in.mkv", Some(c"out.mp4")).expect("created");
    let ok = match progress {
        Some(reports) => {
            let mut callback = |percent: f32| {
                reports.push(percent);
                stop_at.is_none_or(|n| reports.len() < n)
            };
            job.process(&mut fake, Some(&mut callback))
        }
        None => job.process(&mut fake, None),
    };
    job.destroy(&mut fake);
    assert_eq!(fake.live, 0, "every context freed");
    (ok, fake)
}

// ---------------------------------------------------------------------------
// test_media_remux.c: refused jobs

#[test]
fn create_missing_input_fails() {
    let mut fake = Fake::new(Script {
        exists: false,
        ..Script::default()
    });
    assert!(create(&mut fake, c"in.wav", Some(c"out.mkv")).is_none());
    assert!(fake.log.is_empty(), "nothing opened: {:?}", fake.log);
}

#[test]
fn create_same_file_fails() {
    let mut fake = Fake::new(Script::default());
    assert!(create(&mut fake, c"in.wav", Some(c"in.wav")).is_none());
    assert!(fake.log.is_empty());
}

#[test]
fn create_without_output_name_fails() {
    // Intentional difference: the C calls strcmp(in, NULL).
    let mut fake = Fake::new(Script::default());
    assert!(create(&mut fake, c"in.wav", None).is_none());
    assert!(fake.log.is_empty());
}

#[test]
fn create_unreadable_input_fails() {
    let mut fake = Fake::new(Script {
        open_ret: -1,
        ..Script::default()
    });
    assert!(create(&mut fake, c"in.txt", Some(c"out.mkv")).is_none());
    assert_eq!(
        fake.errors(),
        ["media_remux: Could not open input file 'in.txt'"]
    );
    assert_eq!(
        fake.log.last().map(String::as_str),
        Some("free_context false"),
        "{:?}",
        fake.log
    );
    assert!(fake.has("close_input false"));
    assert_eq!(fake.live, 0);
}

#[test]
fn create_without_stream_info_fails() {
    let mut fake = Fake::new(Script {
        find_info_ret: -5,
        ..Script::default()
    });
    assert!(create(&mut fake, c"in.mkv", Some(c"out.mkv")).is_none());
    assert_eq!(
        fake.errors(),
        ["media_remux: Failed to retrieve input stream information"]
    );
    assert!(fake.has("close_input true"));
    assert!(!fake.has("alloc_output"));
    assert_eq!(fake.live, 0);
}

#[test]
fn create_unknown_output_format_fails() {
    let mut fake = Fake::new(Script {
        alloc_ok: false,
        ..Script::default()
    });
    assert!(create(&mut fake, c"in.wav", Some(c"out.nope")).is_none());
    assert_eq!(
        fake.errors(),
        ["media_remux: Could not create output context"]
    );
    assert!(!fake.has("avio_close"));
    assert_eq!(fake.live, 0);
}

#[test]
fn create_output_in_missing_directory_fails() {
    let mut fake = Fake::new(Script {
        avio_ret: -2,
        ..Script::default()
    });
    assert!(create(&mut fake, c"in.wav", Some(c"missing/out.mkv")).is_none());
    assert_eq!(
        fake.errors(),
        ["media_remux: Failed to open output file 'missing/out.mkv'"]
    );
    // The C closes the output's (NULL) pb too.
    assert!(fake.has("avio_close"));
    assert_eq!(fake.live, 0);
}

#[test]
fn create_fails_when_a_stream_cannot_be_added() {
    let mut fake = Fake::new(Script {
        streams: vec![video(H264), audio(48000, 1024, 2, ms())],
        new_stream_fail_at: Some(1),
        ..Script::default()
    });
    assert!(create(&mut fake, c"in.mkv", Some(c"out.mp4")).is_none());
    assert_eq!(
        fake.errors(),
        ["media_remux: Failed to allocate output stream"]
    );
    assert!(!fake.has("copy_parameters 1"));
    assert_eq!(fake.live, 0);
}

#[test]
fn create_fails_when_parameters_do_not_copy() {
    let mut fake = Fake::new(Script {
        copy_ret: -12,
        ..Script::default()
    });
    assert!(create(&mut fake, c"in.mkv", Some(c"out.mp4")).is_none());
    assert_eq!(fake.errors(), ["media_remux: Failed to copy parameters"]);
    assert!(!fake.has("copy_metadata"));
}

#[test]
fn create_without_streams_keeps_no_audio_clocks() {
    let mut fake = Fake::new(Script {
        streams: Vec::new(),
        ..Script::default()
    });
    let job = create(&mut fake, c"in.mkv", Some(c"out.mp4")).expect("created");
    assert!(job.next_audio_ts.is_none());
    job.destroy(&mut fake);
}

#[test]
fn create_records_the_input_size() {
    let mut fake = Fake::new(Script {
        size: 123_456,
        ..Script::default()
    });
    let job = create(&mut fake, c"in.mkv", Some(c"out.mp4")).expect("created");
    assert_eq!(job.in_size, 123_456);
    assert_eq!(job.next_audio_ts.as_deref(), Some(&[NOPTS_VALUE][..]));
    job.destroy(&mut fake);
}

// ---------------------------------------------------------------------------
// stream setup

#[test]
fn five_channels_become_4point1() {
    let (_, fake) = run(
        Script {
            streams: vec![audio(48000, 1024, 5, ms())],
            ..Script::default()
        },
        None,
        None,
    );
    let setup: Vec<&str> = fake
        .log
        .iter()
        .map(String::as_str)
        .skip_while(|l| !l.starts_with("new_stream"))
        .take(5)
        .collect();
    assert_eq!(
        setup,
        [
            "new_stream 0",
            "copy_parameters 0",
            "copy_metadata 0",
            "set_codec_tag 0 00000000",
            "default_channel_layout 0 5",
        ]
    );
    assert!(fake.has("set_layout_4point1 0"));
}

#[test]
fn six_channels_get_the_default_layout() {
    let (_, fake) = run(
        Script {
            streams: vec![audio(48000, 1024, 6, ms())],
            ..Script::default()
        },
        None,
        None,
    );
    assert!(fake.has("default_channel_layout 0 6"));
    assert!(!fake.has("set_layout_4point1"));
}

#[test]
fn video_gets_no_channel_layout() {
    let (_, fake) = run(
        Script {
            streams: vec![video(H264)],
            ..Script::default()
        },
        None,
        None,
    );
    assert!(!fake.has("default_channel_layout"));
    assert!(!fake.has("has_codec_tags"), "only HEVC asks for tags");
    assert!(fake.has("set_codec_tag 0 00000000"));
}

#[test]
fn hevc_is_tagged_hvc1_where_the_muxer_maps_it() {
    let (_, fake) = run(
        Script {
            streams: vec![video(CODEC_ID_HEVC)],
            ..Script::default()
        },
        None,
        None,
    );
    assert!(fake.has(&format!("codec_id_for_tag {TAG_HVC1:08x}")));
    assert!(fake.has(&format!("set_codec_tag 0 {TAG_HVC1:08x}")));
}

#[test]
fn hevc_gets_tag_0_without_hvc1() {
    for (has_tags, tag_codec_id) in [(false, CODEC_ID_HEVC), (true, 0)] {
        let (_, fake) = run(
            Script {
                streams: vec![video(CODEC_ID_HEVC)],
                has_tags,
                tag_codec_id,
                ..Script::default()
            },
            None,
            None,
        );
        assert_eq!(fake.has("codec_id_for_tag"), has_tags);
        assert!(fake.has("set_codec_tag 0 00000000"), "{:?}", fake.log);
    }
}

#[test]
fn nofile_muxers_open_and_close_no_file() {
    let (ok, fake) = run(
        Script {
            flags: FMT_NOFILE,
            ..Script::default()
        },
        None,
        None,
    );
    assert!(ok);
    assert!(!fake.has("avio_open"));
    assert!(!fake.has("avio_close"));
}

// ---------------------------------------------------------------------------
// test_media_remux.c: processing and progress

fn packets(n: usize) -> Vec<(Packet, i32)> {
    (0..n)
        .map(|i| (packet(0, i as i64 * 21, 21, 100 + i as i64 * 10), 0))
        .collect()
}

#[test]
fn progress_runs_from_0_to_100() {
    let mut reports = Vec::new();
    let (ok, fake) = run(
        Script {
            packets: packets(40),
            ..Script::default()
        },
        Some(&mut reports),
        None,
    );
    assert!(ok);
    assert_eq!(fake.written.len(), 40);
    // Once every 12 packets (the 12th, 24th, 36th), at its read position
    // over the input's size.
    assert_eq!(reports, [0.0, 21.0, 33.0, 45.0, 100.0]);
}

#[test]
fn stopping_from_the_callback_still_succeeds() {
    let mut reports = Vec::new();
    let (ok, fake) = run(
        Script {
            packets: packets(40),
            ..Script::default()
        },
        Some(&mut reports),
        Some(2),
    );
    assert!(ok, "characterized, not endorsed");
    assert_eq!(fake.written.len(), 11);
    assert_eq!(reports, [0.0, 21.0, 100.0]);
    assert!(fake.has("write_trailer"));
}

/// The packet read when the callback stops the job is never written, so the
/// job unrefs it before it stops reading (#3365).
#[test]
fn stopping_from_the_callback_unrefs_the_packet_it_read() {
    let mut reports = Vec::new();
    let (_, fake) = run(
        Script {
            packets: packets(40),
            ..Script::default()
        },
        Some(&mut reports),
        Some(2),
    );
    let read = fake
        .log
        .iter()
        .rposition(|l| l.starts_with("read_frame"))
        .expect("a read");
    assert_eq!(
        fake.log.get(read + 1).map(String::as_str),
        Some("packet_unref")
    );
    assert_eq!(fake.log.iter().filter(|l| *l == "packet_unref").count(), 1);
}

#[test]
fn progress_over_an_empty_file_is_not_finite() {
    let mut reports = Vec::new();
    run(
        Script {
            size: 0,
            packets: packets(12),
            ..Script::default()
        },
        Some(&mut reports),
        None,
    );
    assert_eq!(reports.len(), 3);
    assert!(reports[1].is_infinite());
}

#[test]
fn process_fails_when_the_muxer_rejects_the_stream() {
    let mut reports = Vec::new();
    let (ok, fake) = run(
        Script {
            header_ret: -22,
            packets: packets(3),
            ..Script::default()
        },
        Some(&mut reports),
        None,
    );
    assert!(!ok);
    assert!(reports.is_empty());
    assert_eq!(
        fake.errors(),
        ["media_remux: Error opening output file: error -22"]
    );
    assert!(!fake.has("read_frame"));
    assert!(!fake.has("write_trailer"));
}

#[test]
fn a_read_error_is_logged_and_fails() {
    let (ok, fake) = run(
        Script {
            read_end_ret: -5,
            packets: packets(2),
            ..Script::default()
        },
        None,
        None,
    );
    assert!(!ok);
    assert_eq!(
        fake.errors(),
        ["media_remux: Error reading packet: error -5"]
    );
    assert!(fake.has("write_trailer"));
}

#[test]
fn invalid_data_and_einval_writes_are_skipped() {
    let mut script = Script {
        packets: packets(4),
        ..Script::default()
    };
    script.packets[1].1 = ERROR_INVALIDDATA;
    script.packets[2].1 = ERROR_EINVAL;
    let (ok, fake) = run(script, None, None);
    assert!(ok);
    assert_eq!(fake.written.len(), 4);
    assert_eq!(
        fake.errors(),
        [
            format!("media_remux: Error muxing packet: error {ERROR_INVALIDDATA}"),
            format!("media_remux: Error muxing packet: error {ERROR_EINVAL}"),
        ]
    );
}

#[test]
fn another_write_error_stops_and_fails() {
    let mut script = Script {
        packets: packets(4),
        ..Script::default()
    };
    script.packets[1].1 = -28;
    let (ok, fake) = run(script, None, None);
    assert!(!ok);
    assert_eq!(fake.written.len(), 2);
    assert!(fake.has("write_trailer"));
}

#[test]
fn a_trailer_error_fails() {
    let mut reports = Vec::new();
    let (ok, fake) = run(
        Script {
            trailer_ret: -32,
            ..Script::default()
        },
        Some(&mut reports),
        None,
    );
    assert!(!ok);
    assert_eq!(fake.errors(), ["media_remux: av_write_trailer: error -32"]);
    assert_eq!(reports, [0.0, 100.0]);
}

// ---------------------------------------------------------------------------
// test_remux_audio_timestamps.c: audio frames stay sample accurate

/// AAC frames of 1024 samples at `rate`, stored in milliseconds as an MKV
/// does, `gap` frames left out after frame 100.
fn mkv_audio(rate: i32, frames: i64, gap: i64) -> Script {
    let mut packets = Vec::new();
    let mut n = 0;
    for i in 0..frames {
        if (100..100 + gap).contains(&i) {
            continue;
        }
        let start = rescale_rnd(i * 1024, 1000, i64::from(rate), 5);
        let end = rescale_rnd((i + 1) * 1024, 1000, i64::from(rate), 5);
        packets.push((packet(0, start, end - start, n * 300), 0));
        n += 1;
    }
    Script {
        streams: vec![audio(rate, 1024, 2, ms())],
        out_time_bases: vec![Rational { num: 1, den: rate }],
        packets,
        ..Script::default()
    }
}

#[test]
fn mkv_48k_to_mp4() {
    let (ok, fake) = run(mkv_audio(48000, 200, 0), None, None);
    assert!(ok);
    for (i, p) in fake.written.iter().enumerate() {
        assert_eq!(p.dts, i as i64 * 1024, "frame {i}");
        assert_eq!(p.pts, p.dts);
        assert_eq!(p.duration, 1024);
        assert_eq!(p.pos, -1);
    }
}

#[test]
fn mkv_44k_to_mp4() {
    let (ok, fake) = run(mkv_audio(44100, 200, 0), None, None);
    assert!(ok);
    for (i, p) in fake.written.iter().enumerate() {
        assert_eq!(p.dts, i as i64 * 1024, "frame {i}");
        assert_eq!(p.duration, 1024);
    }
}

#[test]
fn mkv_with_gap_to_mp4() {
    let (_, fake) = run(mkv_audio(48000, 200, 10), None, None);
    let dts: Vec<i64> = fake.written.iter().map(|p| p.dts).collect();
    assert_eq!(dts[99], 99 * 1024);
    // The gap stays: frame 110 lands where the input put it, to the
    // millisecond.
    assert!((dts[100] - 110 * 1024).abs() <= 48, "{}", dts[100]);
    assert_eq!(dts[101] - dts[100], 1024);
}

#[test]
fn a_short_last_frame_keeps_its_length() {
    let mut script = mkv_audio(48000, 10, 0);
    // 500 samples: 10.4 ms, stored as 10.
    script.packets.last_mut().unwrap().0.duration = 10;
    let (_, fake) = run(script, None, None);
    assert_eq!(fake.written.last().unwrap().duration, 480);
    assert_eq!(fake.written[0].duration, 1024);
}

#[test]
fn audio_without_a_frame_length_is_rescaled_plainly() {
    let mut script = mkv_audio(48000, 3, 0);
    script.frame_duration = 0;
    script.streams[0].frame_size = 0;
    let (_, fake) = run(script, None, None);
    let p = fake.written[1];
    assert_eq!(p.dts, 21 * 48);
    assert_eq!(p.duration, 22 * 48);
}

#[test]
fn audio_falls_back_to_the_frame_size() {
    let mut script = mkv_audio(48000, 3, 0);
    script.frame_duration = -1;
    let (_, fake) = run(script, None, None);
    assert_eq!(fake.written[1].dts, 1024);
}

#[test]
fn packets_without_dts_are_rescaled_plainly() {
    let mut script = mkv_audio(48000, 2, 0);
    script.packets[1].0.dts = NOPTS_VALUE;
    script.packets[1].0.pts = NOPTS_VALUE;
    let (_, fake) = run(script, None, None);
    // AV_ROUND_PASS_MINMAX keeps AV_NOPTS_VALUE.
    assert_eq!(fake.written[1].dts, NOPTS_VALUE);
    assert_eq!(fake.written[1].pts, NOPTS_VALUE);
}

#[test]
fn video_durations_pass_through_an_int() {
    let mut script = Script {
        streams: vec![video(H264)],
        out_time_bases: vec![Rational {
            num: 1,
            den: 1 << 30,
        }],
        packets: vec![(packet(0, 0, 4000, 0), 0)],
        ..Script::default()
    };
    script.streams[0].time_base = Rational { num: 1, den: 1 };
    let (_, fake) = run(script, None, None);
    // 4000 << 30 does not fit in an int; the C's (int) cast keeps the low
    // 32 bits.
    assert_eq!(fake.written[0].duration, i64::from((4000_i64 << 30) as i32));
}

#[test]
fn a_packet_of_an_unknown_stream_is_written_unchanged() {
    let p = packet(3, 7, 8, 9);
    let (_, fake) = run(
        Script {
            packets: vec![(p, 0)],
            ..Script::default()
        },
        None,
        None,
    );
    assert_eq!(fake.written, [p]);
}

#[test]
fn destroy_closes_input_then_output() {
    let (_, fake) = run(Script::default(), None, None);
    let tail: Vec<&str> = fake
        .log
        .iter()
        .rev()
        .take(3)
        .rev()
        .map(String::as_str)
        .collect();
    assert_eq!(
        tail,
        ["close_input true", "avio_close", "free_context true"]
    );
}
