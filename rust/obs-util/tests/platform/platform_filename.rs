//! Tier 1: 1:1 port of `test/cmocka/test_formatted_filename.c` against the
//! safe core. Each test names the cmocka case it mirrors.
//!
//! The cmocka test fixes the clock with its own `time()`, `TZ=OBS-5:30` and
//! the C locale. Here the time is a [`Now`] with a fixed `struct tm` for the
//! same instant (2025-11-07 10:39:03 UTC is 16:09:03 local at UTC+05:30,
//! zone `OBS`), so nothing depends on the process's time zone; the test
//! process runs in the C locale. Its `obs_get_video_info` fixture is the
//! [`VideoInfo`] passed in. How the shim reads the video info (once, before
//! the format, result unchecked) is checked against the oracle in
//! `platform_filename_parity.rs`.

use obs_c_oracle as _; // links the test allocator
use obs_util::platform::{TmFields, crt::LocalTime};
use obs_util::platform_filename::{
    MAX_LEN, Now, VideoInfo, generate_formatted_filename, video_format_name,
};

/// `FIXED_NOW`: 2025-11-07 10:39:03 UTC, a Friday.
const FIXED_NOW: i64 = 1_762_511_943;

/// `localtime(FIXED_NOW)` in `OBS-5:30`.
const FIELDS: TmFields = TmFields {
    sec: 3,
    min: 9,
    hour: 16,
    mday: 7,
    mon: 10,
    year: 125,
    wday: 5,
    yday: 310,
    isdst: 0,
    gmtoff: 19_800,
    zone: c"OBS",
};

fn now() -> Now {
    Now {
        time: FIXED_NOW,
        local: Some(LocalTime::from_fields(&FIELDS)),
    }
}

/// `reset_video`: 30/1 fps, every other field zero.
fn reset_video() -> VideoInfo {
    VideoInfo {
        fps_num: 30,
        fps_den: 1,
        ..VideoInfo::default()
    }
}

fn name_with(video: &VideoInfo, extension: &str, space: bool, format: &[u8]) -> Option<Vec<u8>> {
    generate_formatted_filename(extension.as_bytes(), space, format, &now(), video)
}

/// `expect_name` with the `reset_video` fixture. A NULL extension or format
/// is the empty one (the shim maps NULL to empty).
fn name(extension: &str, space: bool, format: &[u8]) -> Option<Vec<u8>> {
    name_with(&reset_video(), extension, space, format)
}

#[track_caller]
fn expect(extension: &str, space: bool, format: &[u8], expected: Option<&[u8]>) {
    let got = name(extension, space, format);
    assert_eq!(
        got.as_deref(),
        expected,
        "format {:?} extension {extension:?} space {space}",
        String::from_utf8_lossy(format)
    );
}

#[track_caller]
fn expect_names(cases: &[(&str, &str)]) {
    for &(format, expected) in cases {
        expect("", true, format.as_bytes(), Some(expected.as_bytes()));
    }
}

fn is_valid_utf8(s: &[u8]) -> bool {
    std::str::from_utf8(s).is_ok()
}

/// `zero_width_space_report_test`
#[test]
fn zero_width_space_report() {
    let mut fmt = b"%CCYY-%MM-%DD %hh-%mm-%ss foo".to_vec();
    for _ in 0..100 {
        fmt.extend_from_slice("\u{200B}".as_bytes());
    }
    fmt.extend_from_slice(b"bar");

    let name = name("mkv", true, &fmt).expect("a name");
    // "YYYY-MM-DD hh-mm-ss foo" is 23 bytes; 77 full U+200B fit in 254.
    assert_eq!(name.len(), 254);
    assert!(is_valid_utf8(&name));
}

/// `every_interior_cut_test`
#[test]
fn every_interior_cut() {
    for ch in ["\u{E9}", "\u{200B}", "\u{1F600}"] {
        let clen = ch.len();
        for fit in 0..=clen {
            let prefix = MAX_LEN - fit;
            let mut fmt = vec![b'a'; prefix];
            fmt.extend_from_slice(ch.as_bytes());
            fmt.extend_from_slice(b"tail");

            let name = name("", true, &fmt).expect("a name");
            let expected = if fit == clen { MAX_LEN } else { prefix };
            assert_eq!(
                name.len(),
                expected,
                "{clen}-byte char, {fit} before the limit"
            );
            assert!(is_valid_utf8(&name));
        }
    }
}

/// `ascii_truncation_unchanged_test`
#[test]
fn ascii_truncation_unchanged() {
    let fmt = vec![b'x'; 300];
    let name = name("mp4", false, &fmt).expect("a name");
    assert_eq!(name, &fmt[..MAX_LEN]);
}

/// `invalid_continuation_bytes_test`
#[test]
fn invalid_continuation_bytes() {
    let mut fmt = vec![b'a'];
    fmt.extend(std::iter::repeat_n(0x80u8, 300));
    let name = name("", true, &fmt).expect("a name");
    assert_eq!(name.len(), 252);
}

/// `short_name_unchanged_test`
#[test]
fn short_name_unchanged() {
    expect(
        "mkv",
        false,
        "caf\u{E9} %FPS".as_bytes(),
        Some("caf\u{E9}_30.mkv".as_bytes()),
    );
}

/// `date_time_specifiers_test`, without `%z` and `%Z` (next test).
#[test]
fn date_time_specifiers() {
    expect_names(&[
        ("%CCYY", "2025"),
        ("%YY", "25"),
        ("%MM", "11"),
        ("%DD", "07"),
        ("%hh", "16"),
        ("%mm", "09"),
        ("%ss", "03"),
        ("%a", "Fri"),
        ("%A", "Friday"),
        ("%b", "Nov"),
        ("%B", "November"),
        ("%d", "07"),
        ("%H", "16"),
        ("%I", "04"),
        ("%m", "11"),
        ("%M", "09"),
        ("%p", "PM"),
        ("%S", "03"),
        ("%y", "25"),
        ("%Y", "2025"),
    ]);
}

/// `date_time_specifiers_test` with the cmocka test's `TZ=OBS-5:30`, in a
/// child process that has it from its start, as the cmocka test's group
/// setup does before any call. Hosts differ in where `strftime` reads `%z`
/// and `%Z`: glibc from the `struct tm`, macOS's `%z` and the Windows CRT's
/// both from the C library's own time zone. So this goes through the real
/// path, `localtime` of `FIXED_NOW` ([`Now::at`], what the shim does), and
/// checks every date specifier against the zone, on every host.
#[test]
fn zone_specifiers() {
    if !crate::common::isolated_env(
        concat!(module_path!(), "::zone_specifiers"),
        &[("TZ", "OBS-5:30")],
    ) {
        return;
    }
    let video = reset_video();
    for (format, expected) in [
        ("%z", "+0530"),
        ("%Z", "OBS"),
        ("%CCYY-%MM-%DD %hh-%mm-%ss", "2025-11-07 16-09-03"),
        (
            "%a %A %b %B %d %H %I %p",
            "Fri Friday Nov November 07 16 04 PM",
        ),
        ("%y %Y %m %M %S", "25 2025 11 09 03"),
        ("%s", "1762511943"),
    ] {
        let got =
            generate_formatted_filename(b"", true, format.as_bytes(), &Now::at(FIXED_NOW), &video);
        assert_eq!(got.as_deref(), Some(expected.as_bytes()), "{format}");
    }
}

/// `unix_time_specifier_test`
#[test]
fn unix_time_specifier() {
    expect_names(&[("%s", "1762511943"), ("t%s.", "t1762511943.")]);
}

/// `video_specifiers_test`
#[test]
fn video_specifiers() {
    let video = VideoInfo {
        fps_num: 60,
        fps_den: 1,
        base_width: 1920,
        base_height: 1080,
        output_width: 1280,
        output_height: 720,
        output_format: 2, // VIDEO_FORMAT_NV12
    };
    for (format, expected) in [
        ("%FPS", "60"),
        ("%CRES", "1920x1080"),
        ("%ORES", "1280x720"),
        ("%VF", "NV12"),
        ("%CRES %ORES %FPS %VF", "1920x1080 1280x720 60 NV12"),
    ] {
        assert_eq!(
            name_with(&video, "", true, format.as_bytes()).as_deref(),
            Some(expected.as_bytes()),
            "{format}"
        );
    }
}

/// `fps_formatting_test`
#[test]
fn fps_formatting() {
    for (num, den, expected) in [
        (30, 1, "30"),
        (30000, 1001, "29.97"),
        (60000, 1001, "59.94"),
        (24000, 1001, "23.98"),
        (25, 0, "25"),
        (0, 0, "0"),
        (0, 1, "0"),
        (u32::MAX, 1, "4294967295"),
        (1, 3, "0.33"),
        // characterized, not endorsed: a whole rate with fps_den > 1 keeps
        // its decimals
        (50, 2, "25.00"),
    ] {
        let video = VideoInfo {
            fps_num: num,
            fps_den: den,
            ..reset_video()
        };
        assert_eq!(
            name_with(&video, "", true, b"%FPS").as_deref(),
            Some(expected.as_bytes()),
            "fps {num}/{den}"
        );
    }
}

/// `video_format_names_test`
#[test]
fn video_format_names() {
    // VIDEO_FORMAT_NONE, I420, NV12, I444, P010, BGRA
    for (format, expected) in [
        (0, "None"),
        (1, "I420"),
        (2, "NV12"),
        (10, "I444"),
        (18, "P010"),
        (7, "BGRA"),
    ] {
        let video = VideoInfo {
            output_format: format,
            ..reset_video()
        };
        assert_eq!(
            name_with(&video, "", true, b"%VF").as_deref(),
            Some(expected.as_bytes()),
            "format {format}"
        );
    }
}

/// `get_video_format_name` for every value, and unknown ones.
#[test]
fn video_format_name_table() {
    let names = [
        "None", "I420", "NV12", "YVYU", "YUY2", "UYVY", "RGBA", "BGRA", "BGRX", "Y800", "I444",
        "BGR3", "I422", "I40A", "I42A", "YUVA", "AYUV", "I010", "P010", "I210", "I412", "YA2L",
        "P216", "P416", "v210", "R10l",
    ];
    for (i, want) in names.iter().enumerate() {
        assert_eq!(video_format_name(i as i32), *want, "format {i}");
    }
    for unknown in [-1, 26, 1000, i32::MIN, i32::MAX] {
        assert_eq!(video_format_name(unknown), "None", "format {unknown}");
    }
}

/// `video_info_read_test`: the core uses the fields it is given; with no
/// video the shim gives it zeroed fields, whatever the call left in its
/// struct (checked against the oracle in the parity file).
#[test]
fn video_info_read() {
    expect("", true, b"plain", Some(b"plain"));
    expect("", true, b"%FPS %CRES %FPS", Some(b"30 0x0 30"));

    // no video: the zeroed fields
    assert_eq!(
        name_with(&VideoInfo::default(), "", true, b"%FPS %CRES %ORES %VF").as_deref(),
        Some(&b"0 0x0 0x0 None"[..])
    );
}

/// `specifier_matching_test`
#[test]
fn specifier_matching() {
    expect_names(&[
        ("%YYY", "25Y"),
        ("%CCYYYY", "2025YY"),
        ("%MMM", "11M"),
        ("%mmm", "09m"),
        ("%DDD", "07D"),
        ("%hhh", "16h"),
        ("%sss", "03s"),
        ("%FPSS", "30S"),
        ("%VFF", "NoneF"),
        ("%CRESS", "0x0S"),
        ("%hh%mm%ss", "160903"),
        ("%CCYY-%MM-%DD %hh-%mm-%ss", "2025-11-07 16-09-03"),
        ("x%Yy%yY", "x2025y25Y"),
        ("%%CCYY", "%CCYY"),
        ("%%FPS", "%FPS"),
        // specifiers are case sensitive
        ("%fps", "fps"),
        ("%cres", "cres"),
        ("%vf", "vf"),
    ]);
}

/// `percent_and_unknown_specifiers_test`
#[test]
fn percent_and_unknown_specifiers() {
    expect_names(&[
        ("%%", "%"),
        ("%%%", "%"),
        ("%%%%", "%%"),
        ("100%%", "100%"),
        ("100%", "100"),
        ("a%", "a"),
        ("%q", "q"),
        ("%Q%q", "Qq"),
        ("%e", "e"),
        ("%j", "j"),
        ("%c", "c"),
        ("%T", "T"),
        ("%C", "C"),
        ("%CCY", "CCY"),
        ("%F", "F"),
        ("%FP", "FP"),
        ("%CRE", "CRE"),
        ("%ORE", "ORE"),
        ("%V", "V"),
        ("%1", "1"),
        ("% ", " "),
        ("%\u{E9}", "\u{E9}"),
    ]);
}

/// `space_replacement_test`
#[test]
fn space_replacement() {
    expect("", true, b" a b  c ", Some(b" a b  c "));
    expect("", false, b" a b  c ", Some(b"_a_b__c_"));
    expect("", false, b"   ", Some(b"___"));
    expect("", false, b"a\tb", Some(b"a\tb"));
    expect("", false, b"%A %B", Some(b"Friday_November"));
    expect(
        "mkv",
        false,
        b"%CCYY-%MM-%DD %hh-%mm-%ss",
        Some(b"2025-11-07_16-09-03.mkv"),
    );
    expect(
        "mkv",
        true,
        b"%CCYY-%MM-%DD %hh-%mm-%ss",
        Some(b"2025-11-07 16-09-03.mkv"),
    );
    expect("my ext", false, b"a b", Some(b"a_b.my ext"));
    let video = VideoInfo {
        base_width: 1920,
        base_height: 1080,
        ..reset_video()
    };
    assert_eq!(
        name_with(&video, "", false, b"%CRES %FPS").as_deref(),
        Some(&b"1920x1080_30"[..])
    );
}

/// `extension_test`
#[test]
fn extension() {
    expect("mkv", true, b"name", Some(b"name.mkv"));
    expect("", true, b"name", Some(b"name"));
    expect(".mkv", true, b"name", Some(b"name..mkv"));
    expect("%CCYY", true, b"name", Some(b"name.%CCYY"));
    expect("tar.gz", true, b"name", Some(b"name.tar.gz"));
}

/// `empty_format_test`
#[test]
fn empty_format() {
    expect("", true, b"", None);
    expect("mkv", true, b"", Some(b".mkv"));
    expect("mkv", false, b"", Some(b".mkv"));
    // the name is empty after expansion
    expect("", true, b"%", None);
    expect("mkv", true, b"%", Some(b".mkv"));
}

/// `expect_long_name`: `n` bytes of `x` plus `tail` give `n` bytes of `x`
/// plus `expected_tail`.
#[track_caller]
fn expect_long_name(extension: &str, n: usize, tail: &str, expected_tail: &str) {
    let mut fmt = vec![b'x'; n];
    let mut expected = fmt.clone();
    fmt.extend_from_slice(tail.as_bytes());
    expected.extend_from_slice(expected_tail.as_bytes());
    expect(extension, true, &fmt, Some(&expected));
}

/// `limit_after_expansion_test`
#[test]
fn limit_after_expansion() {
    // 252 + 4 bytes of %CCYY: the year is cut to "202"
    expect_long_name("", 252, "%CCYY", "202");
    // 252 + ".mkv": the extension is cut to ".mk"
    expect_long_name("mkv", 252, "", ".mk");
    // 251 + ".mkv" is exactly 255 bytes and kept
    expect_long_name("mkv", 251, "", ".mkv");
    // "%%" counts once: 254 and two escapes make 256 bytes
    expect_long_name("", 254, "%%%%", "%");
}

/// New: a `strftime` conversion that comes out empty tries the next table
/// entries and then drops the `%`; with no `struct tm` (C: NULL, undefined)
/// every date specifier is empty.
#[test]
fn empty_conversion() {
    let no_tm = Now {
        time: FIXED_NOW,
        local: None,
    };
    let video = reset_video();
    let got = generate_formatted_filename(b"", true, b"a%CCYYb%s%FPS", &no_tm, &video);
    assert_eq!(got.as_deref(), Some(&b"aCCYYb176251194330"[..]));
}

/// New: a replacement is never scanned again, and the position moves past
/// it, so a specifier straight after one is still expanded.
#[test]
fn adjacent_specifiers() {
    expect_names(&[
        ("%FPS%FPS", "3030"),
        ("%VF%%%VF", "None%None"),
        ("%s%ss", "176251194303"),
        ("%%%%%%", "%%%"),
    ]);
}

/// New: the [`Now`] the shim builds is `time` and `localtime` of it.
#[test]
fn now_at_reads_localtime() {
    let at = Now::at(FIXED_NOW);
    assert_eq!(at.time, FIXED_NOW);
    let local = at.local.expect("localtime of 2025");
    assert_eq!(local.strftime(c"%Y"), b"2025");
}

/// New: a `strftime` result of only spaces is not put in, but the scan
/// still skips that many bytes, so the specifier stays (as in C). A zone
/// abbreviation is the only conversion a test can make blank, and the
/// Windows CRT's `%Z` does not read it.
#[cfg(not(windows))]
#[test]
fn blank_conversions() {
    let video = reset_video();
    let with_zone = |zone: &'static core::ffi::CStr, format: &[u8]| {
        let now = Now {
            time: FIXED_NOW,
            local: Some(LocalTime::from_fields(&TmFields { zone, ..FIELDS })),
        };
        generate_formatted_filename(b"", true, format, &now, &video)
    };
    assert_eq!(with_zone(c" ", b"a%Zb").as_deref(), Some(&b"a%Zb"[..]));
    assert_eq!(with_zone(c"   ", b"%Z%Zx").as_deref(), Some(&b"%Z%Zx"[..]));
    assert_eq!(with_zone(c" X", b"a%Zb").as_deref(), Some(&b"a Xb"[..]));
}
