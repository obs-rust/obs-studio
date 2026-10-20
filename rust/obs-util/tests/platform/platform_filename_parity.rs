//! Tier 3: the Rust `os_generate_formatted_filename` behaves exactly like the
//! original `libobs/util/platform-filename.c`, compiled as
//! `oracle_os_generate_formatted_filename`.
//!
//! The oracle's `time()` and `obs_get_video_info` are fixtures
//! (`oracle_platform_filename_set_time`, `_set_video_info`). The plain
//! `obs_get_video_info` the Rust shim calls reads the same fixture, so:
//! - the safe core, given a time and video info, is compared with the C given
//!   the same, for fixed and random formats, extensions, times and video
//!   infos;
//! - the C ABI shim, which reads the real clock, is compared with the C set
//!   to the second the shim read (retried if the second changes), with the
//!   same video fixture, and both must read it exactly once per call.
//!
//! Both sides call the host C library's `localtime` and `strftime` in the
//! process's time zone and the C locale, so names and offsets are compared
//! as the host makes them.
//!
//! Not compared, an intentional difference: a time `localtime` fails for
//! (the C passes NULL to `strftime`): times stay within 1970 to 2999, where
//! it does not fail on any host.
//!
//! The fixtures are statics shared by every test here, so each
//! set-and-call holds [`ORACLE`].
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use core::ffi::{CStr, c_char, c_int, c_void};
use std::ffi::CString;
use std::sync::Mutex;

use obs_c_oracle::platform_filename as c;
use obs_c_oracle::video_io_inlines as c_video;
use obs_util::ffi::bmem::bfree;
use obs_util::ffi::platform_filename::{self as rs, obs_video_info};
use obs_util::platform::crt;
use obs_util::platform_filename::{Now, VideoInfo, generate_formatted_filename, video_format_name};
use proptest::prelude::*;

static ORACLE: Mutex<()> = Mutex::new(());

/// The latest time used here: the Windows CRT's `_localtime64` stops at
/// 3000-12-31.
const MAX_TIME: i64 = 32_503_680_000 - 1;

/// Takes a `bmalloc`ed C string: its bytes up to the NUL, or `None`.
fn take(s: *mut c_char) -> Option<Vec<u8>> {
    if s.is_null() {
        return None;
    }
    // SAFETY: both sides return NUL-terminated `bmalloc`ed strings.
    let bytes = unsafe { CStr::from_ptr(s) }.to_bytes().to_vec();
    // SAFETY: `s` came from `bmalloc`.
    unsafe { bfree(s.cast()) };
    Some(bytes)
}

/// The C struct for `video`, with the fields the formatter does not read
/// set to noise.
fn c_video_info(video: &VideoInfo) -> obs_video_info {
    obs_video_info {
        fps_num: video.fps_num,
        fps_den: video.fps_den,
        base_width: video.base_width,
        base_height: video.base_height,
        output_width: video.output_width,
        output_height: video.output_height,
        output_format: video.output_format,
        adapter: 7,
        gpu_conversion: true,
        colorspace: 3,
        range: 2,
        scale_type: 4,
        ..obs_video_info::ZERO
    }
}

fn set_video(video: &VideoInfo, result: bool) {
    let ovi = c_video_info(video);
    // SAFETY: `ovi` is a `struct obs_video_info` (layout test), copied by
    // the call; ORACLE serializes the fixture.
    unsafe {
        c::oracle_platform_filename_set_video_info((&raw const ovi).cast::<c_void>(), result)
    };
}

fn video_calls() -> c_int {
    // SAFETY: reads a counter; ORACLE serializes the fixture.
    unsafe { c::oracle_platform_filename_video_info_calls() }
}

fn c_string(bytes: Option<&[u8]>) -> Option<CString> {
    bytes.map(|b| CString::new(b).expect("no NUL"))
}

fn ptr_of(s: &Option<CString>) -> *const c_char {
    s.as_ref().map_or(core::ptr::null(), |s| s.as_ptr())
}

/// The C, at `time` with `video`.
fn oracle(
    time: i64,
    video: &VideoInfo,
    result: bool,
    extension: Option<&[u8]>,
    space: bool,
    format: Option<&[u8]>,
) -> Option<Vec<u8>> {
    let (ext, fmt) = (c_string(extension), c_string(format));
    // SAFETY: plain values; ORACLE serializes the clock.
    unsafe { c::oracle_platform_filename_set_time(time) };
    set_video(video, result);
    // SAFETY: NULL or NUL-terminated strings that outlive the call.
    take(unsafe { c::oracle_os_generate_formatted_filename(ptr_of(&ext), space, ptr_of(&fmt)) })
}

/// The safe core and the C agree.
#[track_caller]
fn core_same(
    time: i64,
    video: &VideoInfo,
    extension: Option<&[u8]>,
    space: bool,
    format: Option<&[u8]>,
) {
    let _guard = ORACLE.lock().unwrap_or_else(|e| e.into_inner());
    let want = oracle(time, video, true, extension, space, format);
    let got = generate_formatted_filename(
        extension.unwrap_or_default(),
        space,
        format.unwrap_or_default(),
        &Now::at(time),
        video,
    );
    assert_eq!(
        got.as_deref().map(String::from_utf8_lossy),
        want.as_deref().map(String::from_utf8_lossy),
        "time {time}, {video:?}, extension {extension:?}, space {space}, format {:?}",
        format.map(String::from_utf8_lossy)
    );
    assert_eq!(got, want);
}

const TIME: i64 = 1_762_511_943;

fn video_30() -> VideoInfo {
    VideoInfo {
        fps_num: 30,
        fps_den: 1,
        ..VideoInfo::default()
    }
}

const EXTENSIONS: &[Option<&[u8]>] = &[None, Some(b""), Some(b"mkv"), Some(b"my ext")];

/// Every specifier and case of the cmocka characterization, through the C.
#[test]
fn characterization_formats() {
    let formats: &[&[u8]] = &[
        b"%CCYY",
        b"%YY",
        b"%MM",
        b"%DD",
        b"%hh",
        b"%mm",
        b"%ss",
        b"%a",
        b"%A",
        b"%b",
        b"%B",
        b"%d",
        b"%H",
        b"%I",
        b"%m",
        b"%M",
        b"%p",
        b"%S",
        b"%y",
        b"%Y",
        b"%z",
        b"%Z",
        b"%s",
        b"t%s.",
        b"%FPS",
        b"%CRES",
        b"%ORES",
        b"%VF",
        b"%YYY",
        b"%CCYYYY",
        b"%MMM",
        b"%mmm",
        b"%DDD",
        b"%hhh",
        b"%sss",
        b"%FPSS",
        b"%VFF",
        b"%CRESS",
        b"%hh%mm%ss",
        b"%CCYY-%MM-%DD %hh-%mm-%ss",
        b"x%Yy%yY",
        b"%%CCYY",
        b"%%FPS",
        b"%fps",
        b"%cres",
        b"%vf",
        b"%%",
        b"%%%",
        b"%%%%",
        b"100%%",
        b"100%",
        b"a%",
        b"%q",
        b"%Q%q",
        b"%e",
        b"%j",
        b"%c",
        b"%T",
        b"%C",
        b"%CCY",
        b"%F",
        b"%FP",
        b"%CRE",
        b"%ORE",
        b"%V",
        b"%1",
        b"% ",
        "%\u{E9}".as_bytes(),
        b" a b  c ",
        b"   ",
        b"a\tb",
        b"%A %B",
        b"",
        b"%",
        b"plain",
    ];
    let video = VideoInfo {
        fps_num: 60,
        fps_den: 1,
        base_width: 1920,
        base_height: 1080,
        output_width: 1280,
        output_height: 720,
        output_format: 2,
    };
    for &format in formats {
        for space in [true, false] {
            for &extension in EXTENSIONS {
                core_same(TIME, &video, extension, space, Some(format));
                core_same(TIME, &video_30(), extension, space, Some(format));
            }
        }
    }
    core_same(TIME, &video, None, true, None);
    core_same(TIME, &video, Some(b"mkv"), false, None);
}

#[test]
fn fps_and_format_names() {
    for (num, den) in [
        (30, 1),
        (30000, 1001),
        (60000, 1001),
        (24000, 1001),
        (25, 0),
        (0, 0),
        (0, 1),
        (u32::MAX, 1),
        (1, 3),
        (50, 2),
        (1, 8),
        (1, 200),
        (5, 1000),
        (u32::MAX, 2),
        (u32::MAX, u32::MAX),
        (1, u32::MAX),
    ] {
        let video = VideoInfo {
            fps_num: num,
            fps_den: den,
            ..VideoInfo::default()
        };
        core_same(TIME, &video, None, true, Some(b"%FPS"));
    }
    for format in -5..40 {
        let video = VideoInfo {
            output_format: format,
            ..video_30()
        };
        core_same(TIME, &video, None, true, Some(b"%VF"));
        // SAFETY: the oracle accepts any value and returns a static string.
        let c_name = unsafe { CStr::from_ptr(c_video::oracle_get_video_format_name(format)) };
        assert_eq!(
            video_format_name(format).as_bytes(),
            c_name.to_bytes(),
            "{format}"
        );
    }
}

#[test]
fn long_names_and_utf8_cut() {
    for n in 240..=260 {
        let tails: &[&[u8]] = &[
            b"",
            b"%CCYY",
            b"%%%%",
            "\u{E9}".as_bytes(),
            "\u{200B}".as_bytes(),
            "\u{1F600}tail".as_bytes(),
            b"\x80\x80\x80\x80\x80",
        ];
        for &tail in tails {
            let mut format = vec![b'x'; n];
            format.extend_from_slice(tail);
            for &extension in &EXTENSIONS[..3] {
                core_same(TIME, &video_30(), extension, true, Some(&format));
            }
        }
    }
}

#[test]
fn times_across_the_range() {
    for time in [
        0,
        1,
        86_399,
        951_782_400,
        TIME,
        2_147_483_647,
        2_147_483_648,
        4_102_444_800,
        MAX_TIME,
    ] {
        core_same(
            time,
            &video_30(),
            Some(b"mkv"),
            true,
            Some(b"%CCYY-%MM-%DD %hh-%mm-%ss %a %A %b %B %I %p %y %z %Z %s"),
        );
    }
}

/// The shim, with the real clock, against the C at the second it read. The
/// video info is read exactly once per call on both sides. Returns the name.
#[track_caller]
fn shim_same(
    video: &VideoInfo,
    result: bool,
    extension: Option<&[u8]>,
    space: bool,
    format: Option<&[u8]>,
) -> Option<Vec<u8>> {
    let (ext, fmt) = (c_string(extension), c_string(format));
    let _guard = ORACLE.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        let before = crt::time_now();
        set_video(video, result);
        let calls = video_calls();
        // SAFETY: NULL or NUL-terminated strings that outlive the call.
        let got =
            take(unsafe { rs::os_generate_formatted_filename(ptr_of(&ext), space, ptr_of(&fmt)) });
        assert_eq!(
            video_calls(),
            calls + 1,
            "the shim reads the video info once"
        );
        if crt::time_now() != before {
            continue;
        }
        let want = oracle(before, video, result, extension, space, format);
        assert_eq!(video_calls(), calls + 2, "the C reads the video info once");
        assert_eq!(
            got.as_deref().map(String::from_utf8_lossy),
            want.as_deref().map(String::from_utf8_lossy),
            "{video:?} result {result}, extension {extension:?}, space {space}, format {:?}",
            format.map(String::from_utf8_lossy)
        );
        return got;
    }
}

#[test]
fn shim_matches_c() {
    let video = VideoInfo {
        fps_num: 30000,
        fps_den: 1001,
        base_width: 2560,
        base_height: 1440,
        output_width: 1920,
        output_height: 1080,
        output_format: 18,
    };
    let formats: &[Option<&[u8]>] = &[
        Some(b"%CCYY-%MM-%DD %hh-%mm-%ss"),
        Some(b"%FPS %CRES %ORES %VF %s %a %p %z %Z"),
        Some(b"plain"),
        Some(b""),
        Some(b"%"),
        None,
    ];
    for &format in formats {
        for &extension in &EXTENSIONS[..3] {
            for space in [true, false] {
                shim_same(&video, true, extension, space, format);
            }
        }
    }
    // with no video the fields format as zero on both sides, whatever the
    // call left in the struct (the fixture fills it)
    for (space, zeroed) in [(true, b"0 0x0 0x0 None"), (false, b"0_0x0_0x0_None")] {
        assert_eq!(
            shim_same(&video, false, None, space, Some(b"%FPS %CRES %ORES %VF")).as_deref(),
            Some(&zeroed[..])
        );
        assert_eq!(
            shim_same(&video, false, Some(b"mkv"), space, Some(b"plain")).as_deref(),
            Some(&b"plain.mkv"[..])
        );
    }
}

/// Pieces a format is made of: every specifier, near-misses, `%` before
/// anything, spaces, multibyte characters and arbitrary bytes.
fn piece() -> impl Strategy<Value = Vec<u8>> {
    const TOKENS: &[&[u8]] = &[
        b"%CCYY",
        b"%YY",
        b"%MM",
        b"%DD",
        b"%hh",
        b"%mm",
        b"%ss",
        b"%%",
        b"%a",
        b"%A",
        b"%b",
        b"%B",
        b"%d",
        b"%H",
        b"%I",
        b"%m",
        b"%M",
        b"%p",
        b"%S",
        b"%y",
        b"%Y",
        b"%z",
        b"%Z",
        b"%FPS",
        b"%CRES",
        b"%ORES",
        b"%VF",
        b"%s",
        b"%",
        b"%C",
        b"%CC",
        b"%CCY",
        b"%F",
        b"%FP",
        b"%CR",
        b"%CRE",
        b"%O",
        b"%ORE",
        b"%V",
        b"%q",
        b"%e",
        b"%c",
        b"%T",
        b" ",
        b"  ",
        b"_",
        b"-",
        b"Y",
        b"s",
        b"S",
        "\u{E9}".as_bytes(),
        "\u{200B}".as_bytes(),
        "\u{1F600}".as_bytes(),
        b"\x80",
        b"\xC3",
    ];
    prop_oneof![
        4 => prop::sample::select(TOKENS).prop_map(<[u8]>::to_vec),
        1 => prop::collection::vec(1u8..=255, 1..4),
        1 => (1usize..120).prop_map(|n| vec![b'x'; n]),
    ]
}

fn format() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(piece(), 0..24).prop_map(|p| p.concat())
}

fn video() -> impl Strategy<Value = VideoInfo> {
    (
        prop_oneof![
            Just(30u32),
            Just(60),
            Just(30000),
            Just(60000),
            any::<u32>()
        ],
        prop_oneof![Just(0u32), Just(1), Just(2), Just(1001), any::<u32>()],
        any::<u32>(),
        any::<u32>(),
        any::<u32>(),
        any::<u32>(),
        -3i32..30,
    )
        .prop_map(
            |(fps_num, fps_den, bw, bh, ow, oh, output_format)| VideoInfo {
                fps_num,
                fps_den,
                base_width: bw,
                base_height: bh,
                output_width: ow,
                output_height: oh,
                output_format,
            },
        )
}

fn extension() -> impl Strategy<Value = Option<Vec<u8>>> {
    prop_oneof![
        Just(None),
        Just(Some(Vec::new())),
        Just(Some(b"mkv".to_vec())),
        prop::collection::vec(1u8..=255, 1..12).prop_map(Some),
        prop::collection::vec(piece(), 1..4).prop_map(|p| Some(p.concat())),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn random_formats_match(
        time in prop_oneof![Just(TIME), 0i64..=MAX_TIME],
        video in video(),
        extension in extension(),
        space in any::<bool>(),
        format in prop::option::weighted(0.95, format()),
    ) {
        core_same(time, &video, extension.as_deref(), space, format.as_deref());
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn random_formats_match_through_the_shim(
        video in video(),
        result in any::<bool>(),
        extension in extension(),
        space in any::<bool>(),
        format in prop::option::weighted(0.95, format()),
    ) {
        shim_same(&video, result, extension.as_deref(), space, format.as_deref());
    }
}
